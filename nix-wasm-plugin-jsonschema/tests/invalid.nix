builtins.wasm {
  path = <plugins/nix_wasm_plugin_jsonschema.wasm>;
  function = "validateJSONSchema";
} {
  schema = builtins.readFile ./schema.json;
  instance = {
    age = -5;
  };
}
