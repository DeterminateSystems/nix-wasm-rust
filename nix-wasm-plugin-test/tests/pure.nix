let
  counter = builtins.wasm {
    path = <plugins/nix_wasm_plugin_test.wasm>;
    function = "counter";
  };
in
[
  (counter "x")
  (counter "y")
  (counter "z")
]
