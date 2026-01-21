builtins.wasm <plugins/nix_wasm_plugin_yaml.wasm> "toYAML"
  (builtins.wasm <plugins/nix_wasm_plugin_yaml.wasm> "fromYAML"
    (builtins.readFile ./test1.yaml))
