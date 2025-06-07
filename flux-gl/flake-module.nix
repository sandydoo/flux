{ inputs, ... }: {
  perSystem = { config, lib, pkgs, system, ... }:
    let
      inherit (inputs) crane;
      inherit (pkgs) stdenv stdenvNoCC;

      src = ../.;

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

      crateNameFromCargoToml = packagePath:
        craneLib.crateNameFromCargoToml {
          cargoToml = lib.path.append packagePath "Cargo.toml";
        };
    in {
    packages = {
      flux-gl = craneLib.buildPackage {
        inherit (crateNameFromCargoToml ./flux) version;
        pname = "flux-gl";
        inherit src;
        cargoExtraArgs = "-p flux-gl";
        doCheck = true;
      };

      flux-gl-desktop-wrapped =
        let
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
            name = "flux-gl-desktop-wrapped";
            inherit (config.packages.flux-gl-desktop) version;
            nativeBuildInputs = [pkgs.makeWrapper];
            buildCommand = ''
              mkdir -p $out/bin
              cp ${config.packages.flux-gl-desktop}/bin/flux-desktop $out/bin
              wrapProgram $out/bin/flux-desktop \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibraries}
            '';
            passthru.unwrapped = config.packages.flux-gl-desktop;
          };

      flux-gl-desktop = craneLib.buildPackage {
        inherit (crateNameFromCargoToml ./flux-desktop) version;
        pname = "flux-gl-desktop";
        inherit src;
        release = true;
        cargoExtraArgs = "-p flux-gl-desktop";
        doCheck = true;
      };

      flux-gl-wasm = craneLib.buildPackage {
        pname = "flux-gl-wasm";
        src = lib.cleanSourceWith {
          inherit src;
          filter = path: type:
            (lib.hasSuffix "\.vert" path) ||
            (lib.hasSuffix "\.frag" path) ||
            (craneLib.filterCargoSources path type);
        };
        cargoExtraArgs = "--package flux-gl-wasm";
        CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
        doCheck = false;
      };
    };
  };
}
