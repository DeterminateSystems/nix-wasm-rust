with import <nixpkgs> {};

rec {
  getDeps = builtins.wasm {
    path = ../target/wasm32-unknown-unknown/release/nix_wasm_plugin_nix_make.wasm;
    function = "getDeps";
  };

  compileCpp = source: runCommandCC
    "${builtins.baseNameOf source.path}.o"
    {
      __structuredAttrs = true;
      includes = source.includes;
      srcPath = source.path;
      src = source.src;
      buildInputs = map (dep: pkgs'.${dep}) source.deps;
      inherit (source) deps;
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

  link = name: objects: runCommandCC
    name
    {
      inherit objects;
      buildInputs = map (dep: pkgs'.${dep}) (builtins.concatLists (map (obj: obj.deps) objects));
    }
    ''
      mkdir -p $out/lib
      g++ -o $out/lib/$name.so \
        $(find $objects -name '*.o' -type f) \
        -lboost_context -lboost_iostreams -lboost_url -larchive -lcrypto -lsodium -lblake3 -lbrotlicommon -lbrotlienc -lbrotlidec -lcpuid -shared
    '';

  sources = getDeps {
    inherit builtins;
    dirs = [
      { root = /home/eelco/Dev/nix/src/libutil;
        prefix = "";
      }
      #{ root = /home/eelco/Dev/nix/src/libstore;
      #  prefix = "";
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
      "store-config-private.hh" = pkgs.writeText "store-config-private.hh"
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
          #define SANDBOX_SHELL "${pkgs.busybox}/bin/busybox"
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

  allSources = getDeps {
    inherit builtins;
    dirs = [
      { root = /home/eelco/Dev/nix/src;
        prefix = "";
      }
      #{ root = /home/eelco/Dev/nix/src/libstore;
      #  prefix = "";
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
      "store-config-private.hh" = pkgs.writeText "store-config-private.hh"
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
          #define SANDBOX_SHELL "${pkgs.busybox}/bin/busybox"
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


  pkgs' = pkgs // {
    libcpuid = pkgs.runCommand "libcpuid" { inherit (pkgs) libcpuid; }
      ''
        ln -s $libcpuid $out
      '';
  };

  all = map compileCpp sources;

  libutil = link "libnixutil.so" (map compileCpp sources);
}
