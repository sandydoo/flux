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

        target = "wasm32-unknown-unknown";
        toolchain = with fenix.packages.${system};
          combine [
            latest.rustc
            latest.cargo
            targets.${target}.latest.rust-std
          ];

        packageJson = ''
          {
            "files": [
              "flux_bg.wasm",
              "flux.js",
              "flux_bg.js",
              "flux.d.ts"
            ],
            "module": "flux.js",
            "types": "flux.d.ts",
            "sideEffects": false
          }
        '';
      in
      rec {
        packages.flux-wasm =
          (naersk.lib.${system}.override {
            rustc = toolchain;
            cargo = toolchain;
          }).buildPackage {
            src = ./.;
            copyLibs = true;
            copyTarget = true;
            compressTarget = false;
            release = false;

            CARGO_BUILD_TARGET = target;
         };

         packages.flux-web =
           pkgs.stdenv.mkDerivation {
             name = "flux";
             src = ./.;

             nativeBuildInputs = [ pkgs.wasm-bindgen-cli ];

             # Need to reorganize the repo and bundle all the browser stuff into one location
             buildPhase = ''
               mkdir -p $out/pkg
               wasm-bindgen --target bundler --out-dir $out/pkg ${packages.flux-wasm}/lib/flux.wasm
               echo $packageJson >> $out/pkg/package.json

               cp ./index.js $out
               cp ./index.html $out
               cp ./package.json $out
               cp ./webpack.config.js $out
             '';

             dontInstall = true;
           };

         defaultPackage = packages.flux-web;

         devShell = pkgs.mkShell {
           packages = [ toolchain pkgs.wasm-bindgen-cli ];
         };
      });
}

