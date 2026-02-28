{
  description = "Nix WebAssembly plugin example";

  inputs = {
    nixpkgs.follows = "nix/nixpkgs";
    flake-schemas.url = "https://flakehub.com/f/DeterminateSystems/flake-schemas/*.tar.gz";
    nix = {
      type = "github";
      owner = "DeterminateSystems";
      repo = "nix-src";
    };
    fenix = {
      url = "https://flakehub.com/f/nix-community/fenix/0.1.*.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "https://flakehub.com/f/nix-community/naersk/0.1.*.tar.gz";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, ... }@inputs:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      supportedSystems = [ "aarch64-darwin" "x86_64-linux" ];
      forAllSystems = f: inputs.nixpkgs.lib.genAttrs supportedSystems (system: f {
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [
            (final: prev: {
              rustToolchain = with inputs.fenix.packages.${system};
                combine [
                  latest.rustc
                  latest.cargo
                  targets.wasm32-wasip1.latest.rust-std
                ];
            })
          ];
        };
        inherit system;
      });
    in
    {
      packages = forAllSystems ({ pkgs, system }: rec {
        default = nix-wasi-plugins;

        nix-wasi-plugins = with pkgs;
          (pkgs.callPackage inputs.naersk {
            cargo = pkgs.rustToolchain;
            rustc = pkgs.rustToolchain;
          }).buildPackage {
            pname = "nix-wasi-plugins";
            version = "0.0.1";

            src = ./..;

            preConfigure = "cd wasi";

            CARGO_BUILD_TARGET = "wasm32-wasip1";

            postInstall =
              ''
                for i in $out/bin/*.wasm; do
                  wasm-opt -O3 -o "$i" "$i"
                done
              '';

            buildInputs = [
              binaryen
            ];
          };
      });

      devShells = forAllSystems ({ pkgs, system }: rec {
        default = with pkgs; self.packages.${system}.default.overrideAttrs (attrs: {
          nativeBuildInputs = attrs.nativeBuildInputs ++ [
            rust-analyzer
            rustfmt
            clippy
          ];
        });
      });

      checks = forAllSystems ({ pkgs, system }: rec {
        build = self.packages.${system}.default;
        run =
          pkgs.runCommand "nix-wasi-plugins-test-${system}"
          {
            buildInputs = [ inputs.nix.packages.${system}.nix ];
            NIX_CONFIG = "extra-experimental-features = wasm-builtin";
          }
          ''
            [[ $(nix eval --store dummy:// --offline --json --show-trace -I plugins=${self.packages.${system}.nix-wasi-plugins}/bin --impure --eval-cores 0 \
              --expr 'builtins.wasm { path = <plugins/nix-wasi-plugin-fib.wasm>; } 33') = 5702887 ]]
            mkdir -p $out
          '';
      });
    };
}
