builtins.wasm { path = <plugins/nix_wasm_plugin_yaml.wasm>; function = "fromYAML"; } (builtins.readFile ./test1.yaml)
