{
  description = "eloquentle - A terminal-based word game";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, advisory-db, ... }:
    let
      # Define the container output separately
      containerOutput = {
        packages.x86_64-linux.container =
          let
            # Use the same nixpkgs instance
            linuxPkgs = nixpkgs.legacyPackages.x86_64-linux;
            linuxCrane = crane.mkLib linuxPkgs;

            # Build the binary for Linux using crane
            linuxBinary = linuxCrane.buildPackage {
              src = linuxCrane.cleanCargoSource ./.;
              strictDeps = true;
              # Optimize for release
              CARGO_BUILD_RELEASE = true;
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

      # Per-system outputs
      systemOutputs = flake-utils.lib.eachDefaultSystem (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          craneLib = crane.mkLib pkgs;

          # Common arguments for all crane builds
          # Changes here will rebuild all dependency crates
          commonArgs = {
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;

            buildInputs = [ ]
              ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
                # Additional darwin specific inputs can be set here
                pkgs.libiconv
              ];
          };

          # Build *just* the cargo dependencies, so we can reuse them
          # This is the key to incremental builds with crane
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          # Build the actual binary
          # Additional args can be added here without rebuilding dependencies
          eloquentle = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;

            meta = with pkgs.lib; {
              description = "A terminal-based word game";
              homepage = "https://github.com/yourusername/eloquentl";
              license = licenses.mit; # Update this if different
              mainProgram = "eloquentle";
            };
          });
        in
        {
          # `nix build`
          packages = {
            default = eloquentle;
            inherit eloquentle;
          };

          # `nix run`
          apps.default = flake-utils.lib.mkApp {
            drv = eloquentle;
          };

          # `nix flake check`
          checks = {
            # Build the crate as part of checks
            inherit eloquentle;

            # Run tests
            eloquentle-test = craneLib.cargoTest (commonArgs // {
              inherit cargoArtifacts;
            });

            # Run clippy (basic linting)
            eloquentle-clippy = craneLib.cargoClippy (commonArgs // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets";
            });
          };

          # `nix develop`
          devShells.default = craneLib.devShell {
            # Inherit inputs from checks
            checks = self.checks.${system};

            packages = with pkgs; [
              rust-analyzer
              cargo-watch
              cargo-edit
              nushell
            ];

            DIRENV_LOG_FORMAT = "";
            shellHook = ''
              echo "Entering devshell"
            '';
          };
        }
      );
    in
    # Recursively merge the system outputs with the container output
    nixpkgs.lib.recursiveUpdate systemOutputs containerOutput;
}
