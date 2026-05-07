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

      flux-desktop-unwrapped = craneLib.buildPackage (individualCrateArgs // {
        pname = "flux-desktop";
        src = fileSetForCrate ./flux-desktop;
        release = true;
        cargoExtraArgs = "-p flux-desktop";
      });

      desktopRuntimeLibraries = with pkgs; [
        vulkan-loader
        libglvnd
        wayland
        wayland-protocols
        libxkbcommon
        libX11
        libXcursor
        libXrandr
        libXi
      ];
    in {
    devShells = {
      default = pkgs.mkShell {
        packages = with pkgs; [nixfmt wasm-pack cargo-outdated nodePackages.pnpm];
        inputsFrom = [config.packages.flux flux-desktop-unwrapped config.packages.flux-wasm];
        nativeBuildInputs = [rustToolchain];
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath desktopRuntimeLibraries;
      };
    };

    packages = {
      flux = craneLib.buildPackage (individualCrateArgs // {
        pname = "flux";
        src = fileSetForCrate ./flux;
        cargoExtraArgs = "-p flux";
      });

      flux-desktop =
        # Can’t use symlinkJoin because of the hooks are passed to the
        # dependency-only build.
        stdenvNoCC.mkDerivation {
          name = "flux-desktop";
          inherit (flux-desktop-unwrapped) version;
          nativeBuildInputs = [pkgs.makeWrapper];
          buildCommand = ''
            mkdir -p $out/bin
            cp ${flux-desktop-unwrapped}/bin/flux-desktop $out/bin
            wrapProgram $out/bin/flux-desktop \
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath desktopRuntimeLibraries}
          '';
          passthru.unwrapped = flux-desktop-unwrapped;
        };

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
