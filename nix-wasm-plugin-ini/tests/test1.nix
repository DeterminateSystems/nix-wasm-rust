builtins.wasm {
  path = <plugins/nix_wasm_plugin_ini.wasm>;
  function = "fromINI";
} (builtins.readFile ./test1.ini)
