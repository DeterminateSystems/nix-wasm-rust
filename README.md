# `nix-wasm-rust`

This crate helps you to write functions in Rust that can be executed by Nix at evaluation time using `builtins.wasm`. This repository also contains a number of tests and demos.

To try it out:

```console
# nix shell .#nix # to get a version of Nix that has `builtins.wasm`

# nix build # to build the Wasm functions

# ls -l ./result/
.r--r--r--@  20k root 31 Dec  1969 nix_wasm_plugin_fib.wasm
.r--r--r--@  71k root 31 Dec  1969 nix_wasm_plugin_fib_wasi.wasm
.r--r--r--@  33k root 31 Dec  1969 nix_wasm_plugin_grep.wasm
.r--r--r--@  60k root 31 Dec  1969 nix_wasm_plugin_ini.wasm
.r--r--r--@  24k root 31 Dec  1969 nix_wasm_plugin_mandelbrot.wasm
.r--r--r--@ 586k root 31 Dec  1969 nix_wasm_plugin_quickjs.wasm
.r--r--r--@  20k root 31 Dec  1969 nix_wasm_plugin_test.wasm
.r--r--r--@ 185k root 31 Dec  1969 nix_wasm_plugin_yaml.wasm

# nix eval --impure --expr \
  'builtins.wasm { path = ./result/nix_wasm_plugin_fib.wasm; function = "fib"; } 40'
warning: 'nix_wasm_plugin_fib.wasm' function 'fib': greetings from Wasm!
165580141

# nix eval --raw --impure --expr \
  'builtins.wasm { path = ./result/nix_wasm_plugin_mandelbrot.wasm; function = "mandelbrot"; } { width = 150; }'
```
