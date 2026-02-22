{
  description = "Nix WebAssembly plugin example";

  inputs = {
    nixpkgs.follows = "nix/nixpkgs";
    flake-schemas.url = "https://flakehub.com/f/DeterminateSystems/flake-schemas/*.tar.gz";
    nix.url = "github:DeterminateSystems/nix-src/wasm";
  };

  outputs = { self, ... }@inputs:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./nix-wasm-rust/Cargo.toml);
      supportedSystems = [ "aarch64-darwin" "x86_64-linux" ];
      forAllSystems = f: inputs.nixpkgs.lib.genAttrs supportedSystems (system: f {
        pkgs = import inputs.nixpkgs { inherit system; };
        inherit system;
      });
    in
    {
      packages = forAllSystems ({ pkgs, system }: rec {
        default = nix-wasm-plugins;

        nix-wasm-plugins = with pkgs;
          let
            rustPackages = pkgs.rustPackages_1_89;
            rustPlatform = rustPackages.rustPlatform;
            rustSysroot = runCommand "rust-sysroot" { } ''
              mkdir -p $out/lib/rustlib
              cp -r ${rustPackages."rustc-unwrapped"}/lib/rustlib/* $out/lib/rustlib/
              mkdir -p $out/lib/rustlib/src
              ln -s ${rustPlatform.rustcSrc} $out/lib/rustlib/src/rust
            '';
            rustcWithSysroot = runCommand "rustc-with-sysroot" { } ''
              mkdir -p $out/bin
              cat > $out/bin/rustc <<'EOF'
              #!/bin/sh
              exec ${rustPackages.rustc}/bin/rustc --sysroot ${rustSysroot} "$@"
              EOF
              chmod +x $out/bin/rustc
            '';
            wasiSdkSuffix = if system == "aarch64-darwin" then "arm64-macos" else "x86_64-linux";
            wasiSdkHash =
              if system == "aarch64-darwin" then
                "sha256-Hbpw5ai4R5n3o6qtklS45QsFbe3p7gtUmEmR+94mHeQ="
              else
                "sha256-/cyLxhFsfBBQxn4NrhLdbgHjU3YUjYhPnvquWJodcO8=";
            wasiSdkSource = fetchzip {
              url = "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-24/wasi-sdk-24.0-${wasiSdkSuffix}.tar.gz";
              hash = wasiSdkHash;
              stripRoot = true;
            };
            wasiSdk = runCommand "wasi-sdk" { } ''
              cp -R -L ${wasiSdkSource} $out
              if [ -d $out/lib/clang ]; then
                chmod -R u+w $out/lib/clang
              fi
              if [ -d $out/lib/clang/18 ] && [ ! -d $out/lib/clang/19 ]; then
                ln -s $out/lib/clang/18 $out/lib/clang/19
              fi
            '';
            workspaceVendor = rustPlatform.fetchCargoVendor {
              src = self;
              hash = "sha256-JImJqCSQKc41F+qOBDBrYMOOIOXgLIG9lEtrY585sGY=";
            };
            stdlibVendor = rustPlatform.fetchCargoVendor {
              src = rustPlatform.rustcSrc;
              cargoRoot = "library";
              hash = "sha256-XD+1wJ7GfnJG4qyulIdZum7VV4rtIoQRM+L0xXUHjXA=";
            };
            cargoVendor = runCommand "cargo-vendor-merged" { } ''
              mkdir -p $out
              cp -R ${workspaceVendor}/* $out/
              mkdir -p $out/.cargo
              cp -R ${workspaceVendor}/.cargo/* $out/.cargo/
              chmod -R u+w $out
              cp -R ${stdlibVendor}/* $out/
              cp ${workspaceVendor}/Cargo.lock $out/Cargo.lock
              cp ${workspaceVendor}/.cargo/config.toml $out/.cargo/config.toml
            '';
          in rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;

          cargoLock.lockFile = ./Cargo.lock;
          cargoDeps = cargoVendor;

          src = self;

          CARGO_BUILD_TARGET = "wasm32-wasip1";
          buildPhase = "cargo build --release --workspace -Z build-std=std,panic_abort";

          checkPhase = ''
            # for i in nix-wasm-plugin-*/tests/*.nix; do
            #   echo "running test $i..."
            #   base="$(dirname $i)/$(basename $i .nix)"
            #   nix eval --store dummy:// --offline --json --show-trace -I plugins=target/wasm32-wasip1/release --impure --eval-cores 0 --file "$i" > "$base.out"
            #   cmp "$base.exp" "$base.out"
            # done
          '';
          dontCheck = true;

          installPhase = ''
            mkdir -p $out
            for i in target/wasm32-wasip1/release/*.wasm; do
              wasm-opt -O3 --enable-bulk-memory --enable-exception-handling --enable-nontrapping-float-to-int --enable-simd -o "$out/$(basename "$i")" "$i"
            done
          '';

          nativeBuildInputs = [
            rustPackages.rustc.llvmPackages.lld
            wasm-bindgen-cli
            wasm-pack
            binaryen
            pkgs.llvmPackages.clang
            pkgs.llvmPackages.libclang
            inputs.nix.packages.${system}.nix-cli
          ];

          WASI_SDK = "${wasiSdk}";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          RUSTC = "${rustcWithSysroot}/bin/rustc";
          CC_wasm32_wasip1 = "${wasiSdk}/bin/clang";
          CXX_wasm32_wasip1 = "${wasiSdk}/bin/clang++";
          AR_wasm32_wasip1 = "${wasiSdk}/bin/ar";
          CFLAGS_wasm32_wasip1 = "--sysroot=${wasiSdk}/share/wasi-sysroot -isystem ${wasiSdk}/lib/clang/19/include -mexception-handling -mllvm -wasm-enable-sjlj";
          CXXFLAGS_wasm32_wasip1 = "--sysroot=${wasiSdk}/share/wasi-sysroot -isystem ${wasiSdk}/lib/clang/19/include -mexception-handling -mllvm -wasm-enable-sjlj";
          BINDGEN_EXTRA_CLANG_ARGS_wasm32_wasip1 = "-fvisibility=default --sysroot=${wasiSdk}/share/wasi-sysroot -isystem ${wasiSdk}/lib/clang/19/include -resource-dir ${wasiSdk}/lib/clang/19 -mexception-handling";
          RUSTFLAGS = "-L ${wasiSdk}/share/wasi-sysroot/lib/wasm32-wasip1 -C target-feature=+exception-handling -C llvm-args=-wasm-enable-sjlj";
          CARGO_TARGET_WASM32_WASIP1_LINKER = "${wasiSdk}/bin/ld.lld";
          RUSTC_BOOTSTRAP = "1";
          NIX_CONFIG = "extra-experimental-features = wasm-builtin";
        };

        nix = inputs.nix.packages.${system}.nix-cli;
      });

      devShells = forAllSystems ({ pkgs, system }: rec {
        default = with pkgs;
          let
            rustPackages = pkgs.rustPackages_1_89;
          in self.packages.${system}.default.overrideAttrs (attrs: {
            nativeBuildInputs = attrs.nativeBuildInputs ++ [
              wabt
              rust-analyzer
              rustPackages.rustfmt
              rustPackages.clippy
            ];
          });
      });

      checks = forAllSystems ({ pkgs, system }: rec {
        build = self.packages.${system}.default;
      });
    };
}
