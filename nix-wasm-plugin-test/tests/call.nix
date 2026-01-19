let 
  wasm = builtins.wasm <plugins/nix_wasm_plugin_test.wasm>;
  strictMap = wasm "strictMap";
  range = wasm "range";
  sum = wasm "sum";
  double = wasm "double";
in
  sum (strictMap { list = range { start = 0; end = 100000; }; fun = double; })
