{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";
    flake-parts.url = "github:hercules-ci/flake-parts";
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

  outputs = inputs@{
    flake-parts,
    self,
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
    wgsl-analyzer-flake,
    ...
  }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        ./flake-module.nix
        ./flux-gl/flake-module.nix
      ];

      flake = {};
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      perSystem = { config, system, pkgs, ...}: {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [
            (import rust-overlay)
            wgsl-analyzer-flake.overlays.${system}.default
          ];
        };

        formatter = pkgs.alejandra;

        packages = {
          default = config.packages.web;

          web = pkgs.callPackage ./web/default.nix {
            inherit (config.packages) flux-wasm flux-gl-wasm;
          };
        };
      };
    };
}
