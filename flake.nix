{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs =
    {
      self,
      flake-utils,
      naersk,
      nixpkgs,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        naersk' = pkgs.callPackage naersk { };

      in
      rec {
        # For `nix build` & `nix run`:
        packages.default = naersk'.buildPackage {
          src = ./.;
        };

        # For backward compatibility
        defaultPackage = packages.default;

        # For `nix develop`:
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
          ];
        };
      }
    )
    // {
      # Dedicated container output that will always build on x86_64-linux
      packages.x86_64-linux.container =
        let
          linuxPkgs = import nixpkgs { system = "x86_64-linux"; };
          linuxNaersk = linuxPkgs.callPackage naersk { };
          linuxBinary = linuxNaersk.buildPackage { src = ./.; };
        in
        linuxPkgs.dockerTools.buildLayeredImage {
          name = "eloquentle";
          config = {
            Cmd = [ "${linuxBinary}/bin/eloquentle" ];
          };
        };
    };
}
