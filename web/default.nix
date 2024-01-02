{
  pkgs,
  lib,
  stdenv,
  flux-wasm,
  mkYarnPackage,
}: let
  packageJSON = builtins.fromJSON (builtins.readFile ./package.json);
  version = packageJSON.version;

  nodeModules = mkYarnPackage {
    name = "flux-dependencies";
    src = lib.cleanSourceWith {
      src = ./.;
      filter = name: type:
        builtins.any (x: baseNameOf name == x) ["package.json" "yarn.lock"];
    };
    publishBinsFor = ["webpack" "gh-pages"];
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

  wasm-bindgen-cli-0_2_89 = pkgs.wasm-bindgen-cli.overrideAttrs (drv: rec {
    name = "wasm-bindgen-cli-${version}";
    version = "0.2.89";
    src = pkgs.fetchCrate {
      inherit (drv) pname;
      inherit version;
      sha256 = "sha256-IPxP68xtNSpwJjV2yNMeepAS0anzGl02hYlSTvPocz8=";
    };

    cargoDeps = drv.cargoDeps.overrideAttrs (lib.const {
      inherit src;
      name = "${drv.pname}-vendor.tar.gz";
      outputHash = "sha256-JJl9ufudSIjlC9dx7OUjk4LISf29drDKi8bBI313Tns=";
    });

    doCheck = false;
  });
in
  stdenv.mkDerivation {
    pname = "flux-web";
    inherit version;
    src = gitignoreSource [] ./.;

    nativeBuildInputs = with pkgs; [
      openssl
      pkg-config
    ];

    buildInputs = with pkgs; [
      nodeModules
      yarn
      nodePackages.pnpm
      elmPackages.elm
      wasm-bindgen-cli-0_2_89
      binaryen
    ];

    passthru = {
      inherit nodeModules;
    };

    patchPhase = ''
      ln -sf ${nodeModules}/libexec/*/node_modules .
    '';

    # This is, rather confusingly, called relative to the current working
    # directory, not the flake or this file. Make sure to run `nix develop` from
    # the root directory!
    shellHook = ''
      ln -sf ${nodeModules}/libexec/*/node_modules ./web
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
      wasm-opt -Os -o flux/flux_wasm_bg.wasm flux/flux_wasm_bg_unoptimized.wasm
      echo '${packageJson}' > ./flux/package.json

      webpack \
        --mode production \
        --output-path=$out \
        --env skip-wasm-pack \
        --env path-to-elm=${pkgs.elmPackages.elm}/bin/elm
    '';
  }
