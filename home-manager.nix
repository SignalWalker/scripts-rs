inputs @ {self, ...}: {
  config,
  pkgs,
  lib,
  ...
}:
with builtins; let
  std = pkgs.lib;
  cfg = options.programs;
in {
  options = with lib; {
  };
  imports = [
    (import ./check-battery.nix inputs)
  ];
  config = {
  };
}
