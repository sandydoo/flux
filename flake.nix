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
    flake-utils.lib.eachSystem [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ] (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        inherit (pkgs) lib stdenv stdenvNoCC;

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
          in {
            inherit rustToolchain;

            package = craneLib.buildPackage rec {
              src = ./.;
              cargoExtraArgs = "-p ${packageName} --target ${targetTriple}";

              # HOST_CC = "${pkgsCross.stdenv.cc.nativePrefix}cc";
              preConfigure = ''
                export CARGO_TARGET_${shellEnvTriple}_LINKER=${hostPkgs.stdenv.cc.targetPrefix}cc
              '';

              shellHook = preConfigure;
              doCheck = false;

              buildInputs = lib.optionals hostPkgs.hostPlatform.isWindows
                (with hostPkgs; [
                  windows.mingw_w64_pthreads
                  windows.pthreads
                ]);
            };
          };

      in lib.recursiveUpdate rec {
        devShells = {
          default = pkgs.mkShell {
            packages = with pkgs; [ nixfmt wasm-pack ];
            inputsFrom = with packages; [ flux-web flux-desktop ];
            nativeBuildInputs = [ rustToolchain ];
          };
        };

        formatter = pkgs.nixfmt;

        apps.default = flake-utils.lib.mkApp {
          name = "flux-desktop";
          drv = packages.flux-desktop-wrapped;
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
            # This is a bit of an issue because it builds all the packages in
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

          flux-desktop-wrapped = let
            runtimeLibraries = with pkgs;
              [ wayland
                wayland-protocols
                xorg.libX11
                xorg.libXcursor
                xorg.libXrandr
                xorg.libXi
                libGL
              ];
          in
          # Can’t use symlinkJoin because of the hooks are passed to the
          # dependency-only build.
          stdenvNoCC.mkDerivation {
            name = "flux-desktop-wrapped";
            inherit (packages.flux-desktop) version;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            buildCommand = ''
              mkdir -p $out/bin
              cp ${packages.flux-desktop}/bin/flux-desktop $out/bin
              wrapProgram $out/bin/flux-desktop \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibraries}
            '';
            passthru.unwrapped = packages.flux-desktop;
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
      } (lib.optionalAttrs stdenv.isLinux (let
        crossPkgs = import nixpkgs {
          inherit system;
          crossSystem.config = "x86_64-w64-mingw32";
          overlays = [ (import rust-overlay) ];
        };

        fluxDesktopCrossWindows = crossCompileFor {
          hostPkgs = crossPkgs;
          targetTriple = "x86_64-pc-windows-gnu";
          packageName = "flux-desktop";
        };
      in {
        devShells.crossShell = crossPkgs.mkShell {
          inputsFrom = [ self.packages.${system}.flux-desktop-x86_64-pc-windows-gnu ];
          nativeBuildInputs = [ fluxDesktopCrossWindows.rustToolchain ];
        };

        # Cross-compile the Windows executable only on Linux hosts.
        packages.flux-desktop-x86_64-pc-windows-gnu =
          fluxDesktopCrossWindows.package;
      })));
}
