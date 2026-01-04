let 
  wasm = builtins.wasm <plugins/nix_wasm_plugin_test.wasm>;
  map = wasm "map";
  range = wasm "range";
  sum = wasm "sum";
  double = wasm "double";
in
  sum (map { list = range { start = 0; end = 100000; }; fun = double; })
