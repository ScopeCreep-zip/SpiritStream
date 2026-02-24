{
  description = "SpiritStream — multi-destination streaming application";

  inputs = {
    konductor.url = "github:braincraftio/konductor";
    nixpkgs.follows = "konductor/nixpkgs";
    flake-utils.follows = "konductor/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, konductor, ... }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = system: import nixpkgs { inherit system; };
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = pkgsFor system;
          server = pkgs.callPackage ./nix/server.nix { };
          desktop = pkgs.callPackage ./nix/desktop.nix { };
        in
        {
          spiritstream-server = server;
          spiritstream-desktop-unwrapped = desktop;
          default = pkgs.callPackage ./nix/package.nix {
            spiritstream-server = server;
            spiritstream-desktop-unwrapped = desktop;
          };
          spiritstream = self.packages.${system}.default;
        });

      overlays.default = final: prev: {
        spiritstream-server = final.callPackage ./nix/server.nix { };
        spiritstream-desktop-unwrapped = final.callPackage ./nix/desktop.nix { };
        spiritstream = final.callPackage ./nix/package.nix { };
      };

      homeManagerModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.programs.spiritstream;
          defaultPkg = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        in
        {
          options.programs.spiritstream = {
            enable = lib.mkEnableOption "SpiritStream multi-destination streaming";

            package = lib.mkOption {
              type = lib.types.package;
              default = defaultPkg;
              description = "The spiritstream package to use.";
            };
          };

          config = lib.mkIf cfg.enable {
            home.packages = [ cfg.package ];
          };
        };

      devShells = forAllSystems (system: {
        default = konductor.devShells.${system}.frontend;
      });
    };
}
