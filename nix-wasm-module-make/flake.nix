# The flake published alongside the nix-make dependency scanner (see the
# installPhase of `nix-wasm-plugins` in ../flake.nix), so that it can be used
# as a flake input, e.g. from FlakeHub:
#
#   builtins.wasm { path = inputs.nix-wasm-module-make.nixWasmModules.nix_make; ... }
{
  description = "The nix-make C/C++ dependency scanner as a Nix Wasm module";

  outputs =
    { self }:
    let
      schemasSchema = doc: {
        version = 1;
        inherit doc;
        inventory = output: {
          children = builtins.mapAttrs (schemaName: schema: {
            shortDescription = "A schema checker for the `${schemaName}` flake output";
            what = "flake schema";
            evalChecks.isValidSchema =
              schema.version or 0 == 1
              && builtins.isString (schema.doc or null)
              && builtins.isFunction (schema.inventory or null);
          }) output;
        };
      };
    in
    {
      nixWasmModules.nix_make = ./nix_wasm_plugin_nix_make.wasm;

      schemas = {
        inherit (self.exportedSchemas) nixWasmModules;
        schemas = schemasSchema ''
          The `schemas` flake output defines [flake schemas](https://manual.determinate.systems/protocols/flake-schemas.html) for this flake's outputs.
        '';
        exportedSchemas = schemasSchema ''
          The `exportedSchemas` flake output defines flake schemas that you intend for other flakes to use.
        '';
      };

      exportedSchemas = {
        nixWasmModules = {
          version = 1;
          doc = ''
            The `nixWasmModules` flake output contains WebAssembly modules that you can call from Nix expressions using [`builtins.wasm`](https://manual.determinate.systems/protocols/wasm.html).
          '';
          inventory = output: {
            children = builtins.mapAttrs (moduleName: module: {
              what = "Nix Wasm module";
              evalChecks.isPath = builtins.isPath module;
            }) output;
          };
        };
      };
    };
}
