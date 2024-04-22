{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    wgsl-analyzer-flake = {
      url = "github:wgsl-analyzer/wgsl-analyzer";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        crane.follows = "crane";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
    wgsl-analyzer-flake,
    ...
  }:
    flake-utils.lib.eachSystem [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ] (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [
          (import rust-overlay)
          wgsl-analyzer-flake.overlays.${system}.default
        ];
      };

      inherit (pkgs) lib stdenv stdenvNoCC;

      rustExtensions = [
        "cargo"
        "rust-src"
        "rust-analyzer"
        "rustfmt"
      ];

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = rustExtensions;
        targets = ["wasm32-unknown-unknown"];
      };

      craneLib = (crane.mkLib pkgs).overrideScope' (final: prev: {
        rustc = rustToolchain;
        cargo = rustToolchain;
        rustfmt = rustToolchain;
      });

      crateNameFromCargoToml = packagePath:
        craneLib.crateNameFromCargoToml {cargoToml = lib.path.append packagePath "Cargo.toml";};

      crossCompileFor = {
        hostPkgs,
        targetTriple,
        packagePath,
      }: let
        rustToolchain = hostPkgs.pkgsBuildHost.rust-bin.stable.latest.default.override {
          targets = [targetTriple];
          extensions = rustExtensions ++ ["rustc"];
        };

        craneLib = (crane.mkLib hostPkgs).overrideScope' (final: prev: {
          rustc = rustToolchain;
          cargo = rustToolchain;
          rustfmt = rustToolchain;
        });

        # Uppercase and replace `-` with `_`.
        shellEnvTriple =
          builtins.replaceStrings (["-"] ++ lib.lowerChars)
          (["_"] ++ lib.upperChars)
          targetTriple;
      in {
        inherit rustToolchain;

        package = craneLib.buildPackage rec {
          inherit (crateNameFromCargoToml packagePath) pname version;
          src = ./.;
          cargoExtraArgs = "-p ${packagePath} --target ${targetTriple}";

          # HOST_CC = "${pkgsCross.stdenv.cc.nativePrefix}cc";
          preConfigure = ''
            export CARGO_TARGET_${shellEnvTriple}_LINKER=${hostPkgs.stdenv.cc.targetPrefix}cc
          '';

          shellHook = preConfigure;
          doCheck = false;

          buildInputs =
            lib.optionals hostPkgs.hostPlatform.isWindows
            (with hostPkgs; [
              windows.mingw_w64_pthreads
              windows.pthreads
            ]);
        };
      };
    in
      lib.recursiveUpdate rec {
        devShells = {
          default = pkgs.mkShell {
            packages = with pkgs; [nixfmt wasm-pack wgsl-analyzer cargo-outdated];
            inputsFrom = with packages; [flux-web flux-desktop];
            nativeBuildInputs = [rustToolchain];
          };
        };

        formatter = pkgs.alejandra;

        apps.default = flake-utils.lib.mkApp {
          name = "flux-desktop";
          drv = packages.flux-desktop-wrapped;
        };

        packages = {
          default = packages.flux-web;

          flux = craneLib.buildPackage {
            inherit (crateNameFromCargoToml ./flux) pname version;
            src = ./.;
            cargoExtraArgs = "-p flux";
            doCheck = true;
          };

          flux-wasm = craneLib.buildPackage {
            inherit (crateNameFromCargoToml ./flux-wasm) pname version;
            src = ./.;

            # By default, crane adds the `--workspace` flag to all commands.
            # This is a bit of an issue because it builds all the packages in
            # the workspace, even those that don’t support the wasm32 target (hi
            # glutin).
            cargoExtraArgs = "--package flux-wasm";
            CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
            doCheck = false; # This doesn’t disable the checks…
          };

          # TODO: move out to flux-next
          flux-next-wasm = craneLib.buildPackage {
            pname = "flux-next-wasm";
            src = lib.cleanSourceWith {
              src = ./flux-next;
              filter = path: type:
                (lib.hasSuffix "\.wgsl" path) ||
                (craneLib.filterCargoSources path type);
            };
            cargoExtraArgs = "--package flux-wasm";
            CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
            doCheck = false;
          };

          flux-web = pkgs.callPackage ./web/default.nix {
            inherit (packages) flux-wasm flux-next-wasm;
          };

          flux-desktop-wrapped = let
            runtimeLibraries = with pkgs; [
              wayland
              wayland-protocols
              libxkbcommon
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
              nativeBuildInputs = [pkgs.makeWrapper];
              buildCommand = ''
                mkdir -p $out/bin
                cp ${packages.flux-desktop}/bin/flux-desktop $out/bin
                wrapProgram $out/bin/flux-desktop \
                  --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibraries}
              '';
              passthru.unwrapped = packages.flux-desktop;
            };

          flux-desktop = craneLib.buildPackage {
            inherit (crateNameFromCargoToml ./flux-desktop) pname version;
            src = ./.;
            release = true;
            cargoExtraArgs = "-p flux-desktop";
            doCheck = true;
            nativeBuildInputs =
              lib.optionals stdenv.isDarwin
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
          overlays = [(import rust-overlay)];
        };

        fluxDesktopCrossWindows = crossCompileFor {
          hostPkgs = crossPkgs;
          targetTriple = "x86_64-pc-windows-gnu";
          packageName = "flux-desktop";
        };
      in {
        devShells.crossShell = crossPkgs.mkShell {
          inputsFrom = [self.packages.${system}.flux-desktop-x86_64-pc-windows-gnu];
          nativeBuildInputs = [fluxDesktopCrossWindows.rustToolchain];
        };

        # Cross-compile the Windows executable only on Linux hosts.
        packages.flux-desktop-x86_64-pc-windows-gnu =
          fluxDesktopCrossWindows.package;
      })));
}
