let
  wasm = builtins.wasm <plugins/nix_wasm_plugin_test.wasm>;
  counter = wasm "counter";
in
  [ (counter "x") (counter "y") (counter "z") ]
