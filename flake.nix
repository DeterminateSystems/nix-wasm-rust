{
  description = "Nix WebAssembly plugin example";

  inputs = {
    nixpkgs.follows = "nix/nixpkgs";
    flake-schemas.url = "https://flakehub.com/f/DeterminateSystems/flake-schemas/*.tar.gz";
    nix.url = "github:DeterminateSystems/nix-src/main";
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

        nix-wasm-plugin-quickjs = with pkgs;
          let
            quickjsCargoToml = builtins.fromTOML (builtins.readFile ./nix-wasm-plugin-quickjs/Cargo.toml);
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
            wasiCc = pkgs.pkgsCross.wasi32.stdenv.cc;
            wasiLibc = pkgs.pkgsCross.wasi32.wasilibc;
            wasiLibcDev = wasiLibc.dev;
            wasiSysroot = runCommand "wasi-sysroot" { } ''
              mkdir -p $out/include $out/lib/wasm32-wasip1
              cp -R ${wasiLibcDev}/include/* $out/include/
              cp -R ${wasiLibc}/lib/* $out/lib/
              cp -R ${wasiLibc}/lib/* $out/lib/wasm32-wasip1/
            '';
            wasiSdk = runCommand "wasi-sdk-compat" { } ''
              mkdir -p $out/bin $out/lib/clang/19 $out/share

              ln -s ${wasiCc}/bin/wasm32-unknown-wasi-clang $out/bin/clang
              ln -s ${wasiCc}/bin/wasm32-unknown-wasi-clang++ $out/bin/clang++
              ln -s ${wasiCc}/bin/wasm32-unknown-wasi-ar $out/bin/ar
              ln -s ${wasiCc}/bin/wasm32-unknown-wasi-ld.lld $out/bin/ld.lld

              ln -s ${wasiCc}/resource-root/include $out/lib/clang/19/include
              ln -s ${wasiSysroot} $out/share/wasi-sysroot
            '';
            workspaceVendor = rustPlatform.fetchCargoVendor {
              src = self;
              hash = "sha256-c2jpj5YfRKIiIAwry0dOoNzAqW6YUdnHWsCe61t/New=";
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
            pname = quickjsCargoToml.package.name;
            version = quickjsCargoToml.package.version;

            cargoLock.lockFile = ./Cargo.lock;
            cargoDeps = cargoVendor;

            src = self;

            buildPhase = ''
              RUSTFLAGS="-L ${wasiSdk}/share/wasi-sysroot/lib/wasm32-wasip1" \
                cargo build --release -p nix-wasm-plugin-quickjs \
                --target wasm32-wasip1 -Z build-std=std,panic_abort
            '';

            installPhase = ''
              mkdir -p $out
              for i in target/wasm32-wasip1/release/*.wasm; do
                wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-simd -o "$out/$(basename "$i")" "$i"
              done
            '';

            nativeBuildInputs = [
              rustPackages.rustc.llvmPackages.lld
              binaryen
              llvmPackages.clang
              llvmPackages.libclang
            ];

            WASI_SDK = "${wasiSdk}";
            LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";
            RUSTC = "${rustcWithSysroot}/bin/rustc";
            CC_wasm32_wasip1 = "${wasiSdk}/bin/clang";
            AR_wasm32_wasip1 = "${wasiSdk}/bin/ar";
            CFLAGS_wasm32_wasip1 = "--sysroot=${wasiSdk}/share/wasi-sysroot -isystem ${wasiSdk}/lib/clang/19/include";
            BINDGEN_EXTRA_CLANG_ARGS_wasm32_wasip1 = "-fvisibility=default --sysroot=${wasiSdk}/share/wasi-sysroot -isystem ${wasiSdk}/lib/clang/19/include -resource-dir ${wasiSdk}/lib/clang/19";
            CARGO_TARGET_WASM32_WASIP1_LINKER = "${wasiSdk}/bin/ld.lld";
            RUSTC_BOOTSTRAP = "1";
            doCheck = false;
          };

        nix-wasm-plugins = with pkgs; rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;

          cargoLock.lockFile = ./Cargo.lock;

          src = self;

          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          buildPhase = "cargo build --release --workspace --exclude nix-wasm-plugin-quickjs";

          checkPhase = ''
            mkdir -p plugins
            cp target/wasm32-unknown-unknown/release/*.wasm plugins/
            cp ${nix-wasm-plugin-quickjs}/*.wasm plugins/

            for i in nix-wasm-plugin-*/tests/*.nix; do
              echo "running test $i..."
              base="$(dirname $i)/$(basename $i .nix)"
              nix eval --store dummy:// --offline --json --show-trace -I plugins=plugins --impure --eval-cores 0 --file "$i" > "$base.out"
              cmp "$base.exp" "$base.out"
            done
          '';

          installPhase = ''
            mkdir -p $out
            for i in target/wasm32-unknown-unknown/release/*.wasm; do
              wasm-opt -O3 -o "$out/$(basename "$i")" "$i"
            done
            cp ${nix-wasm-plugin-quickjs}/*.wasm $out/
          '';

          nativeBuildInputs = [
            rustc.llvmPackages.lld
            wasm-bindgen-cli
            wasm-pack
            binaryen
            inputs.nix.packages.${system}.nix-cli
          ];

          NIX_CONFIG = "extra-experimental-features = wasm-builtin";
        };

        nix = inputs.nix.packages.${system}.nix-cli;
      });

      devShells = forAllSystems ({ pkgs, system }: rec {
        default = with pkgs; self.packages.${system}.default.overrideAttrs (attrs: {
          nativeBuildInputs = attrs.nativeBuildInputs ++ [
            wabt
            rust-analyzer
            rustfmt
            clippy
          ];
        });
      });

      checks = forAllSystems ({ pkgs, system }: rec {
        build = self.packages.${system}.default;
      });
    };
}
