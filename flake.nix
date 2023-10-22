{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        craneLib = crane.lib.${system};
        my-crate = craneLib.buildPackage {
          src = craneLib.cleanCargoSource (craneLib.path ./.);

          buildInputs = [
            pkgs.openssl
            pkgs.zlib
            pkgs.sqlite
            pkgs.gcc
            pkgs.pkg-config
            pkgs.diesel-cli
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
        };

      in
      {
        packages.default = my-crate;

        devShells.default = craneLib.devShell {
          inputsFrom = [ my-crate ];
          packages = [
            pkgs.cargo-audit
            pkgs.cargo-watch
            pkgs.openssl
            pkgs.zlib
            pkgs.sqlite
            pkgs.gcc
            pkgs.pkg-config
            pkgs.rust-analyzer
            pkgs.diesel-cli
          ];
          shellHook = ''
            export LD_LIBRARY_PATH=${pkgs.openssl}/lib:$LD_LIBRARY_PATH
	    export DATABASE_URL=database.db

            # Only copy config.yaml to if it doesn't already exist
            [ ! -e local.config.yaml ] && cp config.yaml local.config.yaml
          '';
        };
      });
}
