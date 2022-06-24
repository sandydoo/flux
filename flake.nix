{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-22.05";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay }:
    let
      SYSTEMS =
        [ "aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux" ];
    in flake-utils.lib.eachSystem SYSTEMS (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        inherit (pkgs) lib stdenv;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
        };

        craneLib = (crane.mkLib pkgs).overrideScope' (final: prev: {
          rustc = rustToolchain;
          cargo = rustToolchain;
          rustfmt = rustToolchain;
        });

        crossCompileFor = { hostPkgs, targetTriple, packageName }:
          let
            rustToolchain =
              hostPkgs.pkgsBuildHost.rust-bin.stable.latest.default.override {
                targets = [ targetTriple ];
              };

            craneLib = (crane.mkLib hostPkgs).overrideScope' (final: prev: {
              rustc = rustToolchain;
              cargo = rustToolchain;
              rustfmt = rustToolchain;
            });

            # Uppercase and replace `-` with `_`.
            shellEnvTriple = builtins.replaceStrings ([ "-" ] ++ lib.lowerChars)
              ([ "_" ] ++ lib.upperChars) targetTriple;
          in craneLib.buildPackage {
            src = ./.;
            cargoExtraArgs = "-p ${packageName} --target ${targetTriple}";
            "CARGO_TARGET_${shellEnvTriple}_LINKER" =
              "${hostPkgs.stdenv.cc.targetPrefix}cc";
            # HOST_CC = "${pkgsCross.stdenv.cc.nativePrefix}cc";
            doCheck = false;
            buildInputs = lib.optionals hostPkgs.hostPlatform.isWindows
              (with hostPkgs; [ windows.mingw_w64_pthreads windows.pthreads ]);
          };

      in lib.recursiveUpdate rec {
        devShells = {
          default = pkgs.mkShell {
            packages = with pkgs; [ nixfmt wasm-pack ];
            inputsFrom = with packages; [ flux-web flux-desktop ];
            nativeBuildInputs = [ rustToolchain ];
          };
        };

        packages = {
          default = packages.flux-web;

          flux = craneLib.buildPackage {
            src = ./.;
            cargoExtraArgs = "-p flux";
            doCheck = true;
          };

          flux-wasm = craneLib.buildPackage rec {
            src = ./.;

            # By default, crane adds the `--workspace` flag to all commands.
            # This is a bit of an issue, because it builds all the packages in
            # the workspace, even those that don’t support the wasm32 target (hi
            # glutin).
            cargoBuildCommand = "cargo build --release";
            cargoCheckCommand = "cargo check --release";
            cargoExtraArgs =
              "--package flux-wasm --target wasm32-unknown-unknown";
            doCheck = false; # This doesn’t disable the checks…
          };

          flux-web = pkgs.callPackage ./web/default.nix {
            inherit (packages) flux-wasm;
          };

          flux-desktop = craneLib.buildPackage {
            src = ./.;
            release = true;
            cargoExtraArgs = "-p flux-desktop";
            doCheck = true;
            nativeBuildInputs = lib.optionals stdenv.isDarwin
              (with pkgs.darwin.apple_sdk.frameworks; [
                AppKit
                ApplicationServices
                CoreFoundation
                CoreGraphics
                CoreVideo
                Foundation
                OpenGL
                QuartzCore
              ]);
          };
        };
      } (lib.optionalAttrs stdenv.isLinux {
        # Cross-compile the Windows executable only on Linux hosts.
        packages.flux-desktop-x86_64-pc-windows-gnu = let
          crossPkgs = import nixpkgs {
            inherit system;
            crossSystem.config = "x86_64-w64-mingw32";
            overlays = [ (import rust-overlay) ];
          };
        in crossCompileFor {
          hostPkgs = crossPkgs;
          targetTriple = "x86_64-pc-windows-gnu";
          packageName = "flux-desktop";
        };
      }));
}
