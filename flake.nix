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

        nix-wasm-plugins = with pkgs; rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;

          cargoLock.lockFile = ./Cargo.lock;

          src = self;

          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          buildPhase = "cargo build --release";

          checkPhase = ''
            for i in nix-wasm-plugin-*/tests/*.nix; do
              echo "running test $i..."
              base="$(dirname $i)/$(basename $i .nix)"
              nix eval --store dummy:// --offline --json --show-trace -I plugins=target/wasm32-unknown-unknown/release --impure --eval-cores 0 --file "$i" > "$base.out"
              cmp "$base.exp" "$base.out"
            done
          '';

          installPhase = ''
            mkdir -p $out
            for i in target/wasm32-unknown-unknown/release/*.wasm; do
              wasm-opt -O3 -o "$out/$(basename "$i")" "$i"
            done
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
