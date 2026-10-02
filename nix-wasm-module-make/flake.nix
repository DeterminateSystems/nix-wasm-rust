# The flake published alongside the nix-make dependency scanner (see the
# installPhase of `nix-wasm-plugins` in ../flake.nix), so that it can be used
# as a flake input, e.g. from FlakeHub:
#
#   builtins.wasm { path = inputs.nix-wasm-module-make.nixWasmModules.nix_make; ... }
{
  description = "The nix-make C/C++ dependency scanner as a Nix Wasm module";

  outputs =
    { self }:
    {
      nixWasmModules.nix_make = ./nix_wasm_plugin_nix_make.wasm;
    };
}
