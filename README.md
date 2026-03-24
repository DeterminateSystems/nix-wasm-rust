# `nix-wasm-rust`

This [crate] helps you to write functions in [Rust] that can be executed by Nix at evaluation time using [`builtins.wasm`][blog-post].
This repository also contains a number of tests and demos.

You can try it out using [Determinate Nix][det-nix] version **3.17.0** or later.
If you have Determinate Nix installed, upgrade to the latest version:

```shell
sudo determinate-nixd upgrade
```

If you're not using Determinate Nix, install it using the instructions in our [docs].
Otherwise, you can add a version of the Determinate Nix CLI with Wasm support to your shell session:

```shell
nix shell .#nix
```

Build the Wasm functions and inspect the symlinked output path:

```shell
nix build
ls -l ./result
```

You should see a set of Wasm binaries:

```shell
.r--r--r--@  20k root 31 Dec  1969 nix_wasm_plugin_fib.wasm
.r--r--r--@  71k root 31 Dec  1969 nix_wasm_plugin_fib_wasi.wasm
.r--r--r--@  33k root 31 Dec  1969 nix_wasm_plugin_grep.wasm
.r--r--r--@  60k root 31 Dec  1969 nix_wasm_plugin_ini.wasm
.r--r--r--@  24k root 31 Dec  1969 nix_wasm_plugin_mandelbrot.wasm
.r--r--r--@ 586k root 31 Dec  1969 nix_wasm_plugin_quickjs.wasm
.r--r--r--@  20k root 31 Dec  1969 nix_wasm_plugin_test.wasm
.r--r--r--@ 185k root 31 Dec  1969 nix_wasm_plugin_yaml.wasm
```

Try out the Fibonacci function:

```shell
nix eval --impure --expr \
  'builtins.wasm { path = ./result/nix_wasm_plugin_fib.wasm; function = "fib"; } 40'

# warning: 'nix_wasm_plugin_fib.wasm' function 'fib': greetings from Wasm!
# 165580141
```

And the Mandelbrot function:

```shell
nix eval --raw --impure --expr \
  'builtins.wasm { path = ./result/nix_wasm_plugin_mandelbrot.wasm; function = "mandelbrot"; } { width = 150; }'
```

[blog-post]: https://determinate.systems/blog/builtins-wasm
[crate]: https://crates.io
[det-nix]: https://docs.determinate.systems/determinate-nix
[docs]: https://docs.determinate.systems
[rust]: https://rust-lang.org
