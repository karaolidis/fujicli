{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    lib = {
      # FIXME: https://github.com/NixOS/nix/issues/12281
      url = "git+https://git.karaolidis.com/karaolidis/nix-lib.git";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        treefmt-nix.follows = "treefmt-nix";
      };
    };
  };

  outputs =
    inputs:
    {
      overlays.default =
        final: prev:
        let
          pkgs = final;
        in
        {
          fujicli = pkgs.rustPlatform.buildRustPackage {
            pname = "fujicli";
            version = "0.1.0";

            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };
        };
    }
    // (
      let
        system = "x86_64-linux";

        pkgs = import inputs.nixpkgs {
          inherit system;
          config.allowUnfree = true;
          overlays = [
            inputs.lib.overlays.default
            inputs.self.overlays.default
          ];
        };

        treefmt = inputs.treefmt-nix.lib.evalModule pkgs ./treefmt.nix;
      in
      {
        devShells.${system}.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            cargo-udeps
            cargo-outdated
          ];

          shellHook = ''
            TOP="$(git rev-parse --show-toplevel)"
            export CARGO_HOME="$TOP/.cargo"
          '';
        };

        packages.${system} = with pkgs; {
          default = fujicli;
          inherit fujicli;
        };

        formatter.${system} = treefmt.config.build.wrapper;

        checks.${system} =
          let
            packages = pkgs.lib.mapAttrs' (
              name: pkgs.lib.nameValuePair "package-${name}"
            ) inputs.self.packages.${system};

            devShells = pkgs.lib.mapAttrs' (
              name: pkgs.lib.nameValuePair "devShell-${name}"
            ) inputs.self.devShells.${system};

            formatter.formatting = treefmt.config.build.check inputs.self;
          in
          packages // devShells // formatter;
      }
    );
}
