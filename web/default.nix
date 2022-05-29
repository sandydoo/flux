{ pkgs, lib, stdenv, flux-wasm }:

let
  packageJSON = builtins.fromJSON (builtins.readFile ./package.json);
  version = packageJSON.version;

  nodeDependencies = pkgs.mkYarnPackage {
    name = "flux-dependencies";
    src = lib.cleanSourceWith {
      src = ./.;
      filter = name: type:
        builtins.any (x: baseNameOf name == x) [ "package.json" "yarn.lock" ];
    };
    publishBinsFor = [ "webpack" "gh-pages" ];
  };

  packageJson = ''
    {
      "files": [
        "flux_wasm_bg.wasm",
        "flux_wasm.js",
        "flux_wasm_bg.js",
        "flux_wasm.d.ts"
      ],
      "module": "flux_wasm.js",
      "types": "flux_wasm.d.ts",
      "sideEffects": false
    }
  '';

  gitignoreSource = pkgs.nix-gitignore.gitignoreSource;
in stdenv.mkDerivation rec {
  pname = "flux-web";
  inherit version;
  src = gitignoreSource [ ] ./.;

  buildInputs = with pkgs; [
    nodeDependencies
    pkgs.yarn
    elmPackages.elm
    wasm-bindgen-cli
    binaryen
    flux-wasm
  ];

  patchPhase = ''
    ln -sf ${nodeDependencies}/libexec/*/node_modules .
  '';

  shellHook = ''
    ln -sf ${nodeDependencies}/libexec/*/node_modules .
  '';

  configurePhase = pkgs.elmPackages.fetchElmDeps {
    elmPackages = import ./elm-srcs.nix;
    elmVersion = "0.19.1";
    registryDat = ./registry.dat;
  };

  installPhase = ''
    mkdir -p $out

    mkdir -p ./flux
    wasm-bindgen \
      --target bundler \
      --out-dir ./flux \
      ${flux-wasm}/lib/flux_wasm.wasm

    mv flux/flux_wasm_bg.wasm flux/flux_wasm_bg_unoptimized.wasm
    wasm-opt -O3 -o flux/flux_wasm_bg.wasm flux/flux_wasm_bg_unoptimized.wasm
    echo '${packageJson}' > ./flux/package.json

    webpack \
      --mode production \
      --output-path=$out \
      --env skip-wasm-pack \
      --env path-to-elm=${pkgs.elmPackages.elm}/bin/elm
  '';
}
