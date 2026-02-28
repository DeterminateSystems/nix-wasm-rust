{
  description = "Nix WebAssembly plugin example";

  inputs = {
    nixpkgs.follows = "nix/nixpkgs";
    flake-schemas.url = "https://flakehub.com/f/DeterminateSystems/flake-schemas/*.tar.gz";
    nix.url = "github:DeterminateSystems/nix-src";
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

        nix-wasi-plugins = with pkgs;
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
              hash = "sha256-vkTdv3StxslmBOKy8mFfz5afOiMjBujFd4IU6pkgqGc=";
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
            pname = "nix-wasi-plugins";
            version = cargoToml.package.version;

            cargoLock.lockFile = ./Cargo.lock;
            cargoDeps = cargoVendor;

            # FIXME: filter out the non-wasi plugins from the workspace.
            src = self;

            buildPhase = ''
              RUSTFLAGS="-L ${wasiSdk}/share/wasi-sysroot/lib/wasm32-wasip1" \
                cargo build --release -p nix-wasm-plugin-quickjs -p nix-wasm-plugin-fib-wasi \
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

          nix_wasi_plugins = nix-wasi-plugins;

          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          buildPhase = "cargo build --release --workspace --exclude nix-wasm-plugin-quickjs --exclude nix-wasm-plugin-fib-wasi";

          checkPhase = ''
            mkdir -p plugins
            cp target/wasm32-unknown-unknown/release/*.wasm plugins/
            if [[ -n $nix_wasi_plugins ]]; then
              cp $nix_wasi_plugins/*.wasm plugins/
            fi

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
            if [[ -n $nix_wasi_plugins ]]; then
              cp $nix_wasi_plugins/*.wasm $out/
            fi
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
          nix_wasi_plugins = null;
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
