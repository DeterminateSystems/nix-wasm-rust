builtins.wasm
  {
    path = <plugins/nix_wasm_plugin_yaml.wasm>;
    function = "toYAML";
  }
  [
    {
      foo = 123;
      bar = null;
      xyzzy = "bla";
      a.b.c = "abc";
    }
  ]
