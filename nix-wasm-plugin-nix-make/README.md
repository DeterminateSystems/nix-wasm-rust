# `nix-wasm-plugin-nix-make`

An `#include` dependency scanner for using Nix as a C/C++ build system, where every compilation unit is a separate derivation. It computes, at evaluation time, the transitive closure of `#include`s of each source file so that exactly the required headers can be passed as inputs to the derivation that compiles it. See `packaging/nix-make` in the Nix repository for the Nix side.

## Usage

```nix
let
  getDeps = builtins.wasm { path = ./nix_wasm_plugin_nix_make.wasm; function = "getDeps"; };
in
getDeps {
  inherit builtins;

  # Directories scanned recursively to build the file index. Each file is
  # known by `prefix/<relative path>`. Symlinks are ignored.
  roots = [ { root = ./src/libutil; prefix = ""; } ];

  # Include search path (relative to the prefix namespace), in order.
  includeDirs = [ "" "include" "unix" "unix/include" ];

  # Extra index entries, e.g. generated headers or out-of-tree sources.
  files = { "include/nix/util/config.hh" = builtins.toFile "config.hh" "..."; };

  # The compilation units to compute closures for (keys into the index).
  sources = [ "hash.cc" "unix/file-descriptor.cc" ];
}
```

The result is a list, sorted by `path`, of attribute sets:

```nix
{
  path = "hash.cc";
  src = ./src/libutil/hash.cc;
  includes = { "include/nix/util/hash.hh" = ./src/libutil/include/nix/util/hash.hh; ... };
  externalIncludes = [ "openssl/sha.h" "sodium.h" ... ];
}
```

Includes are resolved like a compiler does: `#include "x"` is first looked up relative to the including file's directory, then in each of `includeDirs`; `#include <x>` only in `includeDirs`. Angle-bracket includes that are not found in the index are reported in `externalIncludes` so the caller can map them to external dependencies. Quoted includes that cannot be resolved produce a warning.

Limitations: preprocessor conditionals are ignored, so the closure may over-approximate (e.g. includes guarded by `#ifdef _WIN32` are still included).
