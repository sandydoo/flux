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
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        toolchain = with fenix.packages.${system};
          combine [
            latest.rustc
            latest.cargo
            targets.wasm32-unknown-unknown.latest.rust-std
          ];

        naersk-lib = naersk.lib.${system}.override {
          rustc = toolchain;
          cargo = toolchain;
        };
      in rec {
        packages.flux = naersk-lib.buildPackage {
          src = ./.;
          release = true;
          cargoBuildOptions = args: args ++ [ "-p flux" ];
        };

        packages.flux-wasm = naersk-lib.buildPackage {
          src = ./.;
          copyBins = false;
          copyLibs = true;
          release = true;
          cargoBuildOptions = args:
            args ++ [ "-p flux-wasm" "--target wasm32-unknown-unknown" ];
        };

        packages.flux-desktop = naersk-lib.buildPackage {
          src = ./.;
          release = true;
          cargoBuildOptions = args: args ++ [ "-p flux-desktop" ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin
            (with pkgs.darwin.apple_sdk.frameworks; [
              OpenGL
              AppKit
              ApplicationServices
              CoreFoundation
              CoreGraphics
              CoreVideo
              Foundation
              QuartzCore
            ]);
        };

        packages.flux-web = import ./web/default.nix {
          inherit pkgs;
          flux-wasm = packages.flux-wasm;
        };

        defaultPackage = packages.flux-desktop;

        devShell = pkgs.mkShell {
          packages = [ pkgs.wasm-pack ];

          inputsFrom = [
            packages.flux
            packages.flux-desktop
            packages.flux-web
          ];
        };
      });
}

