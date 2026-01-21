let
  grep = builtins.wasm <plugins/nix_wasm_plugin_grep.wasm> "grep";
in
  map builtins.baseNameOf (grep { path = ../..; pattern = "hello 123"; inherit builtins; })
