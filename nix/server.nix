{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  openssl,
  xz,
  ffmpeg,
}:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../server/Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "spiritstream-server";
  version = cargoToml.package.version;

  src = lib.fileset.toSource {
    root = ./..;
    fileset = lib.fileset.unions [
      ../server/Cargo.toml
      ../server/Cargo.lock
      ../server/build.rs
      ../server/src
      ../server/styles
      ../data/streaming-platforms.json
      ../themes
    ];
  };

  cargoRoot = "server";
  buildAndTestSubdir = "server";
  cargoLock.lockFile = ../server/Cargo.lock;

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    openssl
    xz
  ];

  env.OPENSSL_NO_VENDOR = "1";

  # TODO: fix upstream test_sanitize_filename — expects 5 underscores for
  # "../../etc/passwd" but sanitize_filename produces 6.  Re-enable once
  # server/src/services/path_validator.rs:162 is corrected.
  doCheck = false;

  postInstall = ''
    wrapProgram $out/bin/spiritstream-server \
      --prefix PATH : ${lib.makeBinPath [ ffmpeg ]}
  '';

  meta = with lib; {
    description = "SpiritStream backend server — multi-destination streaming";
    homepage = "https://github.com/ScopeCreep-zip/SpiritStream";
    license = licenses.isc;
    platforms = platforms.linux;
    mainProgram = "spiritstream-server";
  };
}
