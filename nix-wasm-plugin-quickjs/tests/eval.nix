builtins.wasm { path = <plugins/nix_wasm_plugin_quickjs.wasm>; } ''
  ({
    num: 1,
    float: 1.5,
    str: "hello",
    bool: true,
    nil: null,
    undef: undefined,
    arr: [1, "two", false],
    obj: { nested: 3 }
  })
''
