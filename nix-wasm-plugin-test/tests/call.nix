let 
  wasm = builtins.wasm <plugins/nix_wasm_plugin_test.wasm>;
  map = wasm "map";
  range = wasm "range";
in
  map { list = range { start = 0; end = 10; }; fun = x: x * x; }
