{
  config,
  pkgs,
  lib,
  ...
}:
with builtins; let
  std = pkgs.lib;
  cfg = options.programs;
in {
  imports = [
    ./check-battery.nix
  ];
}
