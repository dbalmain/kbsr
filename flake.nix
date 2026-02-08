{
  description = "kbsr - learn keyboard shortcuts through spaced repetition";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        kbsr = craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
        };
      in
      {
        packages = {
          default = kbsr;
          kbsr = kbsr;
        };

        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            cargo-edit
          ];
        };
      });
}
