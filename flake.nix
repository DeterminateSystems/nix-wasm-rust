{
  description = "Nix WebAssembly plugin example";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.2505.*.tar.gz";
    flake-schemas.url = "https://flakehub.com/f/DeterminateSystems/flake-schemas/*.tar.gz";
  };

  outputs = { self, ... }@inputs:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      supportedSystems = [ "aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux" ];
      forAllSystems = f: inputs.nixpkgs.lib.genAttrs supportedSystems (system: f {
        pkgs = import inputs.nixpkgs { inherit system; };
        inherit system;
      });
    in
    {
      packages = forAllSystems ({ pkgs, system }: rec {
        default = hello-wasm;
        hello-wasm = with pkgs; rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;
          cargoLock.lockFile = ./Cargo.lock;
          src = self;
          #cargoBuildFlags = "--target wasm32-unknown-unknown";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          buildPhase = "cargo build --release";
          checkPhase = "true";
          installPhase = "cp target/wasm32-unknown-unknown/release/hello_wasm.wasm $out";
          nativeBuildInputs = [
            rustc.llvmPackages.lld
            wasm-bindgen-cli
            wasm-pack

            # Dev tools
            wabt
            rust-analyzer
            rustfmt
            clippy
          ];
        };
      });
    };
}
