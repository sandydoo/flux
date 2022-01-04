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
      in
      rec {
        packages.flux-wasm = import ./flux/default.nix {
          inherit pkgs system fenix naersk;
        };

        packages.flux-web = import ./web/default.nix {
          inherit pkgs;
          flux-wasm = packages.flux-wasm;
        };

        defaultPackage = packages.flux-web;

        devShell = pkgs.mkShell {
          packages = [
            pkgs.wasm-pack
          ];

          inputsFrom = [
            packages.flux-wasm
            packages.flux-web
          ];
        };
      });
}

