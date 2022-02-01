{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    naersk = {
      url = "github:nmattia/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, fenix, flake-utils, naersk, nixpkgs }:
    let
      SYSTEMS =
        [ "aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux" ];
    in flake-utils.lib.eachSystem SYSTEMS (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib stdenv;

        toolchain = with fenix.packages.${system};
          combine ([
            latest.rustc
            latest.cargo
            targets.wasm32-unknown-unknown.latest.rust-std
          ] ++ lib.optionals stdenv.buildPlatform.isWindows
            [ targets.x86_64-pc-windows-gnu.latest.rust-std ]);

        naersk-lib = naersk.lib.${system}.override {
          rustc = toolchain;
          cargo = toolchain;
        };

        readVersionFrom = pathToCargoTOML:
          let cargoTOML = builtins.fromTOML (builtins.readFile pathToCargoTOML);
          in "${cargoTOML.package.version}_${
            builtins.substring 0 8 self.lastModifiedDate
          }_${self.shortRev or "dirty"}";

        fluxDesktopVersion = readVersionFrom ./crates/flux-desktop/Cargo.toml;

        flux-desktop-x86_64-pc-windows-gnu = naersk-lib.buildPackage {
          name = "flux-desktop";
          version = fluxDesktopVersion;
          src = ./.;
          preBuild = ''
            export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-args=$(echo $NIX_LDFLAGS | tr ' ' '\n' | grep -- '^-L' | tr '\n' ' ')"
            export NIX_LDFLAGS=
          '';
          cargoBuildOptions = args:
            args ++ [ "-p flux-desktop" "--target x86_64-pc-windows-gnu" ];
          nativeBuildInputs = with pkgs; [ pkgsCross.mingwW64.stdenv.cc ];
          buildInputs = with pkgs.pkgsCross.mingwW64; [
            windows.mingw_w64_pthreads
            windows.pthreads
          ];
          singleStep = true;
        };

      in lib.recursiveUpdate rec {
        defaultPackage = packages.flux-web;

        devShell = pkgs.mkShell {
          packages = with pkgs; [ nixfmt wasm-pack ];
          inputsFrom = [ packages.flux-web packages.flux-desktop ];
        };

        packages.flux = naersk-lib.buildPackage {
          name = "flux";
          version = readVersionFrom ./crates/flux/Cargo.toml;
          src = ./.;
          cargoBuildOptions = args: args ++ [ "-p flux" ];
        };

        packages.flux-wasm = naersk-lib.buildPackage {
          name = "flux-wasm";
          version = readVersionFrom ./crates/flux-wasm/Cargo.toml;
          src = ./.;
          copyBins = false;
          copyLibs = true;
          release = true;
          cargoBuildOptions = args:
            args ++ [ "-p flux-wasm" "--target wasm32-unknown-unknown" ];
        };

        packages.flux-web = import ./web/default.nix {
          inherit (pkgs) pkgs lib stdenv;
          flux-wasm = packages.flux-wasm;
        };

        packages.flux-desktop = naersk-lib.buildPackage {
          name = "flux-desktop";
          version = fluxDesktopVersion;
          src = ./.;
          release = true;
          cargoBuildOptions = args: args ++ [ "-p flux-desktop" ];
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
      } (lib.optionalAttrs stdenv.isLinux {
        # Cross-compile the Windows executable only on Linux hosts.
        packages.flux-desktop-x86_64-pc-windows-gnu =
          flux-desktop-x86_64-pc-windows-gnu;
      }));
}
