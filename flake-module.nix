{ inputs, ... }: {
  perSystem = { config, lib, pkgs, system, ... }:
    let
      inherit (inputs) crane;
      inherit (pkgs) stdenv stdenvNoCC;

      rustExtensions = [
        "cargo"
        "rust-src"
        "rust-analyzer"
        "rustfmt"
      ];

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = rustExtensions;
        targets = [ "wasm32-unknown-unknown" ];
      };

      craneLib = (crane.mkLib pkgs).overrideScope (final: prev: {
        rustc = rustToolchain;
        cargo = rustToolchain;
        rustfmt = rustToolchain;
      });

      src = lib.cleanSourceWith {
        src = craneLib.path ./.;
        filter = path: type:
          (lib.hasSuffix "\.wgsl" path) ||
          (craneLib.filterCargoSources path type);
      };

      commonArgs = {
        inherit src;
      };

      cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
        pname = "flux-dependencies"; # TODO: throws warning otherwise
      });

      individualCrateArgs = {
        inherit cargoArtifacts;
        inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
      };

      fileSetForCrate = crate: lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./.cargo
          ./Cargo.toml
          ./Cargo.lock
          ./flux
          ./flux-desktop
          ./flux-wasm
          ./flux-gl
          crate
        ];
      };
    in {
    devShells = {
      default = pkgs.mkShell {
        packages = with pkgs; [nixfmt-rfc-style wasm-pack cargo-outdated nodePackages.pnpm];
        inputsFrom = with config.packages; [flux flux-desktop flux-wasm];
        nativeBuildInputs = [rustToolchain];
      };
    };

    packages = {
      flux = craneLib.buildPackage (individualCrateArgs // {
        pname = "flux";
        src = fileSetForCrate ./flux;
        cargoExtraArgs = "-p flux";
      });

      flux-desktop-wrapped =
        let
          runtimeLibraries = with pkgs; [
            wayland
            wayland-protocols
            libxkbcommon
            xorg.libX11
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi
          ];
        in
          # Can’t use symlinkJoin because of the hooks are passed to the
          # dependency-only build.
          stdenvNoCC.mkDerivation {
            name = "flux-desktop-wrapped";
            inherit (config.packages.flux-desktop) version;
            nativeBuildInputs = [pkgs.makeWrapper];
            buildCommand = ''
              mkdir -p $out/bin
              cp ${config.packages.flux-desktop}/bin/flux-desktop $out/bin
              wrapProgram $out/bin/flux-desktop \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibraries}
            '';
            passthru.unwrapped = config.packages.flux-desktop;
          };

      flux-desktop = craneLib.buildPackage (individualCrateArgs // {
        pname = "flux-desktop";
        src = fileSetForCrate ./flux-desktop;
        release = true;
        cargoExtraArgs = "-p flux-desktop";
        nativeBuildInputs =
          lib.optionals stdenv.isDarwin
          (with pkgs.darwin.apple_sdk.frameworks; [
            AppKit
            ApplicationServices
            CoreFoundation
            CoreGraphics
            CoreVideo
            Foundation
            QuartzCore
          ]);
      });

      flux-wasm = craneLib.buildPackage (individualCrateArgs // {
        pname = "flux-wasm";
        src = fileSetForCrate ./flux-wasm;
        cargoExtraArgs = "--package flux-wasm";
        CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
        doCheck = false;
      });
    };
  };
}
