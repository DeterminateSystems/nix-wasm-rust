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

  # Suffixes identifying compilation units among the indexed files.
  sourceExtensions = [ ".cc" ];

  # Include search path (relative to the prefix namespace), in order.
  includeDirs = [ "" "include" "unix" "unix/include" ];

  # Extra index entries, e.g. generated headers or out-of-tree sources.
  files = { "include/nix/util/config.hh" = builtins.toFile "config.hh" "..."; };

  # Optional: the compilation units to compute closures for (keys into the
  # index). By default, every indexed file with a source extension is a
  # compilation unit, so adding a source file needs no build system change.
  sources = [ "hash.cc" "unix/file-descriptor.cc" ];

  # Optional: paths (files, or directories with everything below them) to
  # leave out of the compilation units, e.g. sources for other platforms.
  excludeSources = [ "windows" "freebsd" ];

  # Optional: macros known to be defined (with their value) or undefined,
  # used to evaluate preprocessor conditionals around #includes.
  defines = { __linux__ = "1"; HAVE_SECCOMP = "1"; };
  undefines = [ "_WIN32" "__APPLE__" ];

  # Optional: macros to report the use of, per compilation unit, with their
  # values (any type; passed through as is). A unit "uses" a macro if the
  # identifier occurs in the unit or any file in its include closure.
  trackedDefines = { HAVE_SECCOMP = 1; NIX_STORE_DIR = "/nix/store"; IS_STATIC = null; };

  # Optional: files generated at build time (e.g. by bison), mapped to the
  # real file whose #includes they are assumed to inherit. They are not
  # read; the build has to provide them.
  generated = { "parser-tab.hh" = "parser.y"; "parser-tab.cc" = "parser.y"; };
}
```

The result is a list, sorted by `path`, of attribute sets:

```nix
{
  path = "hash.cc";
  src = ./src/libutil/hash.cc;  # null for a generated unit
  includes = { "include/nix/util/hash.hh" = ./src/libutil/include/nix/util/hash.hh; ... };
  generatedIncludes = [ "parser-tab.hh" ... ];  # generated files the unit includes
  externalIncludes = [ "openssl/sha.h" "sodium.h" ... ];
  usedDefines = { HAVE_FOO = 1; ... };  # the entries of `trackedDefines` occurring in the unit or its includes
}
```

Every regular file under the roots is indexed, but a file is only read and parsed for `#include`s once it is reached from a compilation unit, so files that no unit needs (e.g. headers for other platforms) are never read.

Includes are resolved like a compiler does: `#include "x"` is first looked up relative to the including file's directory, then in each of `includeDirs`; `#include <x>` only in `includeDirs`. Angle-bracket includes that are not found in the index are reported in `externalIncludes` so the caller can map them to external dependencies. Quoted includes that cannot be resolved produce a warning.

Preprocessor conditionals (`#if`, `#ifdef`, `#ifndef`, `#elif`, `#else`, `#endif`) are evaluated against the macros in `defines` (an attribute set of macro names to values) and `undefines` (a list of macro names known to be undefined). A condition that cannot be decided from these keeps both branches, so the result is always an over-approximation of what the compiler will include. For example, with `undefines = [ "_WIN32" ]`, an include guarded by `#ifdef _WIN32` is skipped, while one guarded by `#if __GNUC__ >= 12` is kept unless `__GNUC__` is given.
