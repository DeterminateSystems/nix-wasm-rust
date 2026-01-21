let
  wasm = builtins.wasm <plugins/nix_wasm_plugin_test.wasm>;
  lazyMap = wasm "lazyMap";
in
  builtins.elemAt (lazyMap { fun = (x: x * 2); list = [1 2 3 (throw "foo")]; }) 2
