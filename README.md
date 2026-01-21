# `nix-wasm-rust`

This crate helps you to write functions in Rust that can be executed by Nix at evaluation time using `builtins.wasm`. This repository also contains a number of tests and demos.

To try it out:

```console
# nix shell .#nix # to get a version of Nix that has `builtins.wasm`

# nix build # to build the Wasm functions

# ls -l ./result/
-r--r--r-- 3 root root  17396 Jan  1  1970 nix_wasm_plugin_fib.wasm
-r--r--r-- 3 root root  64292 Jan  1  1970 nix_wasm_plugin_ini.wasm
-r--r--r-- 3 root root  33277 Jan  1  1970 nix_wasm_plugin_mandelbrot.wasm
-r--r--r-- 3 root root  29635 Jan  1  1970 nix_wasm_plugin_test.wasm
-r--r--r-- 3 root root 205035 Jan  1  1970 nix_wasm_plugin_yaml.wasm

# nix eval --impure --expr 'builtins.wasm ./result/nix_wasm_plugin_fib.wasm "fib" 40'
warning: '/nix/store/1c9yg0mv…-nix-wasm-rust-0.1.0/nix_wasm_plugin_fib.wasm' function 'fib': greetings from Wasm!
165580141

# nix eval --raw --impure --expr 'builtins.wasm ./result/nix_wasm_plugin_mandelbrot.wasm "mandelbrot" { width = 150; }'
```
