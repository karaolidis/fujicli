{ pkgs, ... }:
{
  projectRootFile = "flake.nix";

  programs = {
    nixfmt = {
      enable = true;
      strict = true;
    };

    rustfmt.enable = true;
    shellcheck.enable = true;

    mandoc = {
      enable = true;
      level = "style";
      manpath = [ "man" ];
      manuals = with pkgs; [
        exiftool
        systemd
        wireshark-cli
      ];
    };
  };

  settings.global.excludes = [ ".envrc" ];
}
