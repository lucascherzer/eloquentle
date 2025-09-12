{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    # Explicitly pin naersk to use our nixpkgs
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    # fenix = {
    #   url = "github:nix-community/fenix/";
    #   inputs.nixpkgs.follows = "nixpkgs";
    # };
  };

  outputs =
    {
      self,
      flake-utils,
      naersk,
      nixpkgs,
    # fenix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
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
          DIRENV_LOG_FORMAT = "";
          shellHook = ''
            echo "Entering devshell"
          '';
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            nushell
          ];
        };
      }
    )
    // {
      # Dedicated container output that will always build on x86_64-linux
      packages.x86_64-linux.container =
        let
          # Use the same nixpkgs instance
          linuxPkgs = nixpkgs.legacyPackages.x86_64-linux;
          linuxNaersk = linuxPkgs.callPackage naersk { };

          # Build the binary for Linux
          linuxBinary = linuxNaersk.buildPackage {
            src = ./.;
            # Optimize for release
            release = true;
          };
        in
        linuxPkgs.dockerTools.buildLayeredImage {
          name = "eloquentle";

          # Only include the binary and necessary runtime files
          contents = [
            # Just copy the binary
            (linuxPkgs.runCommand "eloquentle-binary" { } ''
              mkdir -p $out/bin
              cp ${linuxBinary}/bin/eloquentle $out/bin/
              chmod +x $out/bin/eloquentle
            '')
          ];

          config = {
            Cmd = [ "/bin/eloquentle" ];
          };
        };
    };
}
