{self, ...}: {
  config,
  pkgs,
  lib,
  ...
}:
with builtins; let
  std = pkgs.lib;
  ntf = config.services.notify-failure;
in {
  options = with lib; {
    services.notify-failure = {
      enable = mkEnableOption "notify the user upon failure of systemd service";
      package = mkOption {
        type = types.package;
        default = self.packages.${pkgs.system}.notify-failure;
      };
      systemd = {
        enable = (mkEnableOption "systemd unit override") // {default = true;};
      };
    };
  };
  disabledModules = [];
  imports = [];
  config = lib.mkIf ntf.enable (lib.mkMerge [
    {
      home.packages = [ntf.package];
    }
    (lib.mkIf ntf.systemd.enable {
      xdg.configFile."systemd/user/toplevel-override.conf" = {
        text = ''
          [Unit]
          OnFailure=failure-notification@%n
        '';
      };
      # prevent recursion
      xdg.configFile."systemd/user/failure-notification@.service.d/toplevel-override.conf" = {
        text = "";
      };
      systemd.user.services."failure-notification@" = {
        Unit = {
          Description = "systemd service failure notifications";
          PartOf = ["graphical-session.target"];
        };
        Service = {
          Type = "oneshot";
          ExecStart = "${ntf.package}/bin/notify-failure %i";
        };
      };
    })
  ]);
  meta = {};
}
