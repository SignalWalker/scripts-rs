inputs@{ self, ... }: {
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
    (import ./check-battery.nix inputs)
  ];
}
