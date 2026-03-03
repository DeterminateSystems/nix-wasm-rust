with import <nixpkgs> {};

rec {
  cc = builtins.wasm { 
    path = ../target/wasm32-unknown-unknown/release/nix_wasm_plugin_cc.wasm;
    function = "cc";
  };

/*
  nixSrc = builtins.fetchTree {
    type = "git";
    url = "/home/eelco/Dev/nix";
  };
*/
  
  sources = cc { 
    inherit builtins;
    dirs = [
      { root = /home/eelco/Dev/nix/src/libutil;
        prefix = "";
      }
      #{ root = /home/eelco/Dev/nix/src/libstore;
      #  prefix = "libstore";
      #}
    ];
    files = {
      "nix/store/config.hh" = builtins.toFile "config.hh"
        ''
          #pragma once
          #define NIX_LOCAL_SYSTEM "x86_64-linux"
          #define NIX_SUPPORT_ACL 1
          #define NIX_WITH_AWS_AUTH 1
        '';
      "util-config-private.hh" = builtins.toFile "util-config-private.hh"
        ''
          #pragma once
          #define HAVE_LIBCPUID 1
          #define HAVE_POSIX_FALLOCATE 1
        '';
      "store-config-private.hh" = builtins.toFile "store-config-private.hh"
        ''
          #pragma once
          #define CAN_LINK_SYMLINK 1
          #define DETERMINATE_NIX_VERSION "3.16.3"
          #define HAVE_EMBEDDED_SANDBOX_SHELL 0
          #define HAVE_LCHOWN 1
          #define HAVE_POSIX_FALLOCATE 1
          #define HAVE_SECCOMP 1
          #define HAVE_STATVFS 1
          #undef IS_STATIC
          #define LSOF "lsof"
          #define NIX_CONF_DIR "/etc/nix"
          #define NIX_DATA_DIR "/home/eelco/Dev/nix/outputs/out/share"
          #define NIX_LOG_DIR "/nix/var/log/nix"
          #define NIX_MAN_DIR "/home/eelco/Dev/nix/outputs/out/share/man"
          #define NIX_PREFIX "/home/eelco/Dev/nix/outputs/out"
          #define NIX_STATE_DIR "/nix/var/nix"
          #define NIX_STORE_DIR "/nix/store"
          #define NIX_USE_WASMTIME 1
          #define PACKAGE_VERSION "2.33.3"
          #define SANDBOX_SHELL "/nix/store/cbwbz05v2iqhn2d1w118y1rw97cqimjf-busybox-1.36.1/bin/busybox"
        '';
      "util-unix-config-private.hh" = builtins.toFile "util-unix-config-private.hh"
        ''
          #pragma once
          #define HAVE_CLOSE_RANGE 1
          #define HAVE_DECL_AT_SYMLINK_NOFOLLOW 1
          #define HAVE_LUTIMES 1
          #define HAVE_PIPE2 1
          #define HAVE_STRSIGNAL 1
          #define HAVE_SYSCONF 1
          #define HAVE_UTIMENSAT 1
        '';
    };
  };

  source = builtins.elemAt sources 0;

  compileCpp = source: runCommandCC 
    "${builtins.baseNameOf source.path}.o"
    {
      __structuredAttrs = true;
      includes = source.includes;
      srcPath = source.path;
      src = source.src;
      buildInputs = [ boost nlohmann_json libsodium libarchive brotli libcpuid libblake3 openssl ];
    }
    ''
      for name in "''${!includes[@]}"; do
        mkdir -p "$(dirname "$name")"
        ln -s "''${includes[$name]}" "$name"
      done

      srcDir="$(dirname "$srcPath")"
      mkdir -p "$srcDir"
      ln -s "$src" "$srcPath"

      mkdir -p "$out/$srcDir"
      # FIXME: figure out the -I flags automatically.
      gcc -std=c++23 -O1 -c "$srcPath" -o "$out/$srcDir/$(basename "$srcPath").o" -I . -I include -I unix/include -I linux/include -I windows/include -I widecharwidth
    '';

  all = map compileCpp sources;

  compile1 = compileCpp source;

  compile_ = map compileCpp (builtins.filter (x: x.path == "unix/processes.cc") sources);

  link = name: objects: runCommandCC
    name
    {
      inherit objects;
      buildInputs = [ boost libarchive openssl libsodium libblake3 brotli libcpuid ];
    }
    ''
      mkdir -p $out/lib
      g++ -o $out/lib/$name.so \
        $(find $objects -name '*.o' -type f) \
        -lboost_context -lboost_iostreams -lboost_url -larchive -lcrypto -lsodium -lblake3 -lbrotlicommon -lbrotlienc -lbrotlidec -lcpuid -shared
    '';

  libutil = link "libnixutil.so" (map compileCpp sources);
}
