let 
  strictMap = builtins.wasm { path = <plugins/nix_wasm_plugin_test.wasm>; function = "strictMap"; };
  range = builtins.wasm { path = <plugins/nix_wasm_plugin_test.wasm>; function = "range"; };
  sum = builtins.wasm { path = <plugins/nix_wasm_plugin_test.wasm>; function = "sum"; };
  double = builtins.wasm { path = <plugins/nix_wasm_plugin_test.wasm>; function = "double"; };
in
  sum (strictMap { list = range { start = 0; end = 100000; }; fun = double; })
