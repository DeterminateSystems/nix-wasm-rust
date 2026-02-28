let
  grep = builtins.wasm { path = <plugins/nix_wasm_plugin_grep.wasm>; function = "grep"; };
in
  map builtins.baseNameOf (grep { path = ../..; pattern = "hello 123"; inherit builtins; })
