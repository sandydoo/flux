{
  pkgs,
  lib,
  stdenv,
  pnpm,
  flux-wasm,
  flux-gl-wasm,
}: let
  packageJSON = builtins.fromJSON (builtins.readFile ./package.json);
  version = packageJSON.version;

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
      ${wasm-bindgen-cli-0_2_92}/bin/wasm-bindgen \
        --target bundler \
        --out-name index \
        --out-dir $out \
        ${wasmPkg}/lib/${lib.replaceStrings ["-"] ["_"] wasmPkg.pname}.wasm

      mv $out/index_bg.wasm $out/index_bg_unoptimized.wasm
      ${pkgs.binaryen}/bin/wasm-opt -Os -o $out/index_bg.wasm $out/index_bg_unoptimized.wasm
      echo '${packageJson}' > $out/package.json
    '';

  flux-wasm-packed = prepareWasm flux-wasm;
  flux-gl-wasm-packed = prepareWasm flux-gl-wasm;

  gitignoreSource = pkgs.nix-gitignore.gitignoreSource;

  wasm-bindgen-cli-0_2_92 = pkgs.wasm-bindgen-cli.overrideAttrs (drv: rec {
    name = "wasm-bindgen-cli-${version}";
    version = "0.2.92";
    src = pkgs.fetchCrate {
      inherit (drv) pname;
      inherit version;
      hash = "sha256-1VwY8vQy7soKEgbki4LD+v259751kKxSxmo/gqE6yV0=";
    };

    cargoDeps = drv.cargoDeps.overrideAttrs (lib.const {
      inherit src;
      name = "${drv.pname}-vendor.tar.gz";
      outputHash = "sha256-2R9oZ7T5ulNUrCmXlBzOni9v0ZL+ixUHXmbnbOMhBao=";
    });

    doCheck = false;
  });
in
  stdenv.mkDerivation (finalAttrs: {
    pname = "flux-web";
    inherit version;
    src = gitignoreSource [] ./.;

    nativeBuildInputs = with pkgs; [
      openssl
      pkg-config
      nodejs
      pnpm
      pnpm.configHook
      elmPackages.elm
      wasm-bindgen-cli-0_2_92
      binaryen
    ];

    pnpmDeps = pnpm.fetchDeps {
      inherit (finalAttrs) pname src version;
      hash = "sha256-CHVjCvJOQ52vQc8yDabgIyTh+9d0HQelBO13vUiogT8=";
    };

    preConfigure = pkgs.elmPackages.fetchElmDeps {
      elmPackages = import ./elm-srcs.nix;
      elmVersion = "0.19.1";
      registryDat = ./registry.dat;
    };

    preInstall = ''
      ln -s ${flux-wasm-packed} ./flux
      ln -s ${flux-gl-wasm-packed} ./flux-gl
    '';

    installPhase = ''
      runHook preInstall

      mkdir -p $out

      pnpm run build \
        --output-path=$out \
        --env SKIP_WASM_PACK \
        --env ELM_BIN=${pkgs.elmPackages.elm}/bin/elm

      runHook postInstall
    '';
  })
