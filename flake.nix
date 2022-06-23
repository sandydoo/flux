{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-22.05";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nmattia/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
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

  outputs = { self, nixpkgs, flake-utils, fenix, naersk, crane, rust-overlay }:
    let
      SYSTEMS =
        [ "aarch64-darwin" "aarch64-linux" "x86_64-darwin" "x86_64-linux" ];
    in flake-utils.lib.eachSystem SYSTEMS (system:
      let
        # pkgs = import nixpkgs {
        #   inherit system;
        #   overlays = [ (import rust-overlay) ];
        # };

        pkgs = import nixpkgs {
          localSystem = system;
          crossSystem.config = "x86_64-w64-mingw32";
          overlays = [ (import rust-overlay) ];
        };

        inherit (pkgs) lib stdenv;
        # rustToolchain = with fenix.packages.${system};
        #   combine ([
        #     latest.rustc
        #     latest.cargo
        #     targets.wasm32-unknown-unknown.latest.rust-std
        #   ] ++ lib.optionals stdenv.isLinux
        #     [ targets.x86_64-pc-windows-gnu.latest.rust-std ]);

        rustToolchain = pkgs.pkgsBuildHost.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ] ++ lib.optionals stdenv.isLinux
            [ "x86_64-pc-windows-gnu" ];
        };

        naersk-lib = naersk.lib.${system}.override {
          rustc = rustToolchain;
          cargo = rustToolchain;
        };

        readVersionFrom = pathToCargoTOML:
          let cargoTOML = builtins.fromTOML (builtins.readFile pathToCargoTOML);
          in "${cargoTOML.package.version}_${
            builtins.substring 0 8 self.lastModifiedDate
          }_${self.shortRev or "dirty"}";

        fluxDesktopVersion = readVersionFrom ./flux-desktop/Cargo.toml;

        craneLib = (crane.mkLib pkgs).overrideScope' (final: prev: {
          rustc = rustToolchain;
          cargo = rustToolchain;
          rustfmt = rustToolchain;
        });

        flux-desktop-x86_64-pc-windows-gnu =
          { lib
          , stdenv
          }:
          craneLib.buildPackage {
            src = ./.;
            cargoExtraArgs = "-p flux-desktop --target x86_64-pc-windows-gnu";
            CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${stdenv.cc.targetPrefix}cc";
            # HOST_CC = "${pkgsCross.stdenv.cc.nativePrefix}cc";
            doCheck = false;
            # nativeBuildInputs = with pkgsCross.mingwW64; [
            #   stdenv.cc
            #   # windows.mingw_w64_pthreads
            #   # windows.pthreads
            # ];
            buildInputs = with pkgs.pkgsCross.mingwW64; [
              windows.mingw_w64_pthreads
              windows.pthreads
            ];
          };

      in lib.recursiveUpdate rec {
        devShells = {
          default = pkgs.mkShell {
            packages = with pkgs; [ nixfmt wasm-pack ];
            inputsFrom = with packages; [ flux-web flux-desktop ];
          };
        };

        packages = {
          default = packages.flux-web;

          flux = craneLib.buildPackage {
            src = ./.;
            cargoExtraArgs = "-p flux";
            doCheck = true;
          };

          flux-wasm = naersk-lib.buildPackage {
            name = "flux-wasm";
            version = readVersionFrom ./flux-wasm/Cargo.toml;
            src = ./.;
            copyBins = false;
            copyLibs = true;
            release = true;
            cargoBuildOptions = args:
              args ++ [ "-p flux-wasm" "--target wasm32-unknown-unknown" ];
            doCheck = true;
          };

          flux-web = import ./web/default.nix {
            inherit (pkgs) pkgs lib stdenv;
            inherit (packages) flux-wasm;
          };

          flux-desktop = naersk-lib.buildPackage {
            name = "flux-desktop";
            version = fluxDesktopVersion;
            src = ./.;
            release = true;
            cargoBuildOptions = args: args ++ [ "-p flux-desktop" ];
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
          flux-desktop-x86_64-pc-windows-gnu =
            pkgs.callPackage flux-desktop-x86_64-pc-windows-gnu { };
          };
      } (lib.optionalAttrs stdenv.isLinux {
        # Cross-compile the Windows executable only on Linux hosts.
        packages.flux-desktop-x86_64-pc-windows-gnu =
          pkgs.callPackage flux-desktop-x86_64-pc-windows-gnu { };
        }
      ));
}
