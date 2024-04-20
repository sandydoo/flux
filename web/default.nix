{
  pkgs,
  lib,
  stdenv,
  flux-wasm,
  flux-next-wasm,
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

  # Prepare the wasm package the same way that wasm-pack does.
  # TODO: maybe do this in the flux-wasm build
  prepareWasm = wasmPkg:
    let
      packageJson = ''
        {
          "files": [
            "index_bg.wasm",
            "indexjs",
            "index_bg.js",
            "index.d.ts"
          ],
          "module": "index.js",
          "types": "index.d.ts",
          "sideEffects": [
            "./index.js",
            "./snippets/*"
          ]
        }
      '';

    in
    pkgs.runCommand "flux-wasm" {} ''
      mkdir $out
      ${wasm-bindgen-cli-0_2_91}/bin/wasm-bindgen \
        --target bundler \
        --out-name index \
        --out-dir $out \
        ${wasmPkg}/lib/flux_wasm.wasm

      mv $out/index_bg.wasm $out/index_bg_unoptimized.wasm
      ${pkgs.binaryen}/bin/wasm-opt -Os -o $out/index_bg.wasm $out/index_bg_unoptimized.wasm
      echo '${packageJson}' > $out/package.json
    '';

  flux-wasm-packed = prepareWasm flux-wasm;
  flux-next-wasm-packed = prepareWasm flux-next-wasm;

  gitignoreSource = pkgs.nix-gitignore.gitignoreSource;

  # Newer versions don't build flux-wasm properly. Last tested: 0.2.87.
  wasm-bindgen-cli-0_2_91 = pkgs.wasm-bindgen-cli.overrideAttrs (drv: rec {
    name = "wasm-bindgen-cli-${version}";
    version = "0.2.91";
    src = pkgs.fetchCrate {
      inherit (drv) pname;
      inherit version;
      hash = "sha256-f/RK6s12ItqKJWJlA2WtOXtwX4Y0qa8bq/JHlLTAS3c=";
    };

    cargoDeps = drv.cargoDeps.overrideAttrs (lib.const {
      inherit src;
      name = "${drv.pname}-vendor.tar.gz";
      outputHash = "sha256-UcqGAeHfov0yABuxfxpHCFJKkJqaOtrDJY+LL0/sKSM=";
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
      wasm-bindgen-cli-0_2_91
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

      ln -s ${flux-wasm-packed} ./flux
      ln -s ${flux-next-wasm-packed} ./flux-next

      webpack \
        --mode production \
        --output-path=$out \
        --env skip-wasm-pack \
        --env path-to-elm=${pkgs.elmPackages.elm}/bin/elm
    '';
  }
