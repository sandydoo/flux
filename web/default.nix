{ pkgs, flux-wasm }:

let
  nodeDependencies = pkgs.mkYarnPackage {
    name = "flux-dependencies";
    src = pkgs.lib.cleanSourceWith {
      src = ./.;
      filter = name: type: builtins.any (x: baseNameOf name == x) [
        "package.json"
        "yarn.lock"
      ];
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
in
pkgs.stdenv.mkDerivation rec {
  name = "flux-web";
  src = gitignoreSource [ ] ./.;

  buildInputs = with pkgs; [
    nodeDependencies
    pkgs.yarn
    elmPackages.elm
    wasm-bindgen-cli
    flux-wasm
  ];

  patchPhase = ''
    ln -sf ${nodeDependencies}/libexec/*/node_modules .
  '';

  # Notice that the path here is relative to the toplevel flake. $src does not
  # work here.
  shellHook = ''
    ln -sf ${nodeDependencies}/libexec/*/node_modules ./web
  '';

  configurePhase = pkgs.elmPackages.fetchElmDeps {
    elmPackages = import ./elm-srcs.nix;
    elmVersion = "0.19.1";
    registryDat = ./registry.dat;
  };

  installPhase = ''
    mkdir -p $out

    mkdir -p ./flux
    wasm-bindgen --target bundler --out-dir ./flux ${flux-wasm}/lib/flux_wasm.wasm
    echo '${packageJson}' > ./flux/package.json

    webpack --mode production --output-path=$out --env skip-wasm-pack
  '';
}
