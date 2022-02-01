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

        toolchain = with fenix.packages.${system};
          combine [
            latest.rustc
            latest.cargo
            targets.wasm32-unknown-unknown.latest.rust-std
            targets.x86_64-pc-windows-gnu.latest.rust-std
          ];

        naersk-lib = naersk.lib.${system}.override {
          rustc = toolchain;
          cargo = toolchain;
        };

        flux-desktop-x86_64-pc-windows-gnu = naersk-lib.buildPackage {
          name = "flux-desktop";
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

      in pkgs.lib.recursiveUpdate rec {
        defaultPackage = packages.flux-web;

        devShell = pkgs.mkShell {
          packages = [ pkgs.wasm-pack ];
          inputsFrom = [ packages.flux-web ];
        };

        packages.flux = naersk-lib.buildPackage {
          name = "flux";
          src = ./.;
          cargoBuildOptions = args: args ++ [ "-p flux" ];
        };

        packages.flux-wasm = naersk-lib.buildPackage {
          name = "flux-wasm";
          src = ./.;
          copyBins = false;
          copyLibs = true;
          release = true;
          cargoBuildOptions = args:
            args ++ [ "-p flux-wasm" "--target wasm32-unknown-unknown" ];
        };

        packages.flux-web = import ./web/default.nix {
          inherit pkgs;
          flux-wasm = packages.flux-wasm;
        };

        packages.flux-desktop = naersk-lib.buildPackage {
          name = "flux-desktop";
          src = ./.;
          release = true;
          cargoBuildOptions = args: args ++ [ "-p flux-desktop" ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.buildPlatform.isWindows
            (with pkgs.pkgsCross.mingwW64.windows; [
              mingw_w64_pthreads
              pthreads
            ]);
          nativeBuildInputs =
            pkgs.lib.optionals pkgs.stdenv.buildPlatform.isWindows
            (with pkgs; [ pkgsCross.mingwW64.stdenv.cc ])
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin
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
      } (pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
        packages.flux-desktop-x86_64-pc-windows-gnu =
          flux-desktop-x86_64-pc-windows-gnu;
      }));
}
