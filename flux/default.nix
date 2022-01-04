{ pkgs, system, fenix, naersk }:

let
  target = "wasm32-unknown-unknown";

  toolchain = with fenix.packages.${system};
    combine [
      latest.rustc
      latest.cargo
      targets.${target}.latest.rust-std
    ];

  rust = naersk.lib.${system}.override {
    rustc = toolchain;
    cargo = toolchain;
  };

  gitignoreSource = pkgs.nix-gitignore.gitignoreSource;
in
rust.buildPackage {
  src = gitignoreSource [ ] ./.;
  copyLibs = true;
  copyTarget = false;
  compressTarget = true;
  release = true;

  CARGO_BUILD_TARGET = target;
}
