{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/master";
    systems.url = "github:nix-systems/default";
  };

  outputs =
    {
      self,
      nixpkgs,
      systems,
    }:
    let
      eachSystem = nixpkgs.lib.genAttrs (import systems);
    in
    {
      devShells = eachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            nativeBuildInputs = [
              pkgs.buildPackages.rustc
              pkgs.buildPackages.cargo
              pkgs.buildPackages.bacon
              pkgs.buildPackages.rust-analyzer
              pkgs.buildPackages.cargo-nextest
              pkgs.buildPackages.cocogitto
            ];

            buildInputs = [

            ];
          };
        }
      );

      packages = eachSystem (system: {

      });
    };
}
