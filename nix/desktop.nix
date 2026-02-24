{
  lib,
  rustPlatform,
  cargo-tauri,
  pkg-config,
  nodejs,
  pnpm_9,
  fetchPnpmDeps,
  pnpmConfigHook,
  jq,
  moreutils,
  wrapGAppsHook4,
  webkitgtk_4_1,
  libsoup_3,
  glib-networking,
  libayatana-appindicator,
  librsvg,
}:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../apps/desktop/src-tauri/Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "spiritstream-desktop-unwrapped";
  version = cargoToml.package.version;

  src = lib.fileset.toSource {
    root = ./..;
    fileset = lib.fileset.unions [
      ../apps/desktop/src-tauri
      ../apps/desktop/package.json
      ../apps/web
      ../themes
      ../package.json
      ../pnpm-lock.yaml
      ../pnpm-workspace.yaml
    ];
  };

  cargoRoot = "apps/desktop/src-tauri";
  buildAndTestSubdir = "apps/desktop/src-tauri";
  cargoLock.lockFile = ../apps/desktop/src-tauri/Cargo.lock;

  pnpmDeps = fetchPnpmDeps {
    pname = "spiritstream-pnpm-deps";
    version = cargoToml.package.version;
    src = lib.fileset.toSource {
      root = ./..;
      fileset = lib.fileset.unions [
        ../package.json
        ../pnpm-lock.yaml
        ../pnpm-workspace.yaml
        ../apps/desktop/package.json
        ../apps/web/package.json
      ];
    };
    pnpm = pnpm_9;
    fetcherVersion = 1;
    hash = "sha256-p0z2HqgvkStbcsPXY0mJxdBMaEOHhoV2qePo2l5YQvo=";
  };

  nativeBuildInputs = [
    cargo-tauri.hook
    nodejs
    pnpm_9
    pnpmConfigHook
    pkg-config
    jq
    moreutils
    wrapGAppsHook4
  ];

  buildInputs = [
    webkitgtk_4_1
    libsoup_3
    glib-networking
    libayatana-appindicator
    librsvg
  ];

  postPatch = ''
    # Remove sidecar bundling — server is provided via env var wrapper
    jq '
      del(.bundle.externalBin) |
      .bundle.targets = ["deb"]
    ' apps/desktop/src-tauri/tauri.conf.json | sponge apps/desktop/src-tauri/tauri.conf.json

    # libappindicator-sys uses dlopen at runtime — patch absolute path
    substituteInPlace $cargoDepsCopy/libappindicator-sys-*/src/lib.rs \
      --replace-fail "libayatana-appindicator3.so.1" "${libayatana-appindicator}/lib/libayatana-appindicator3.so.1"
  '';

  meta = with lib; {
    description = "SpiritStream desktop — multi-destination streaming application";
    homepage = "https://github.com/ScopeCreep-zip/SpiritStream";
    license = licenses.isc;
    platforms = platforms.linux;
    mainProgram = "spiritstream-desktop";
  };
}
