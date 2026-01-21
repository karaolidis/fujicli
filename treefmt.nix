{ ... }:
{
  projectRootFile = "flake.nix";

  programs = {
    nixfmt = {
      enable = true;
      strict = true;
    };

    rustfmt.enable = true;
    shellcheck.enable = true;
  };

  settings.global.excludes = [ ".envrc" ];
}
