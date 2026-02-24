{
  lib,
  symlinkJoin,
  makeWrapper,
  spiritstream-server,
  spiritstream-desktop-unwrapped,
}:

symlinkJoin {
  name = "spiritstream-${spiritstream-desktop-unwrapped.version}";
  paths = [ spiritstream-desktop-unwrapped ];
  nativeBuildInputs = [ makeWrapper ];

  postBuild = ''
    wrapProgram $out/bin/spiritstream-desktop \
      --set SPIRITSTREAM_SERVER_PATH "${spiritstream-server}/bin/spiritstream-server"
  '';

  meta = spiritstream-desktop-unwrapped.meta // {
    mainProgram = "spiritstream-desktop";
  };
}
