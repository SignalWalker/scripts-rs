{ self, ... }: { config
               , pkgs
               , lib
               , ...
               }:
with builtins; let
  std = pkgs.lib;
  cfg = config.services.check-battery;
in
{
  options.services.check-battery = with lib; {
    enable = mkEnableOption "battery level notifications";
    package = mkOption {
      type = types.package;
      default = self.packages.${pkgs.system}.check-battery;
    };
    systemd = {
      enable = (mkEnableOption "systemd service") // { default = true; };
      target = mkOption {
        type = types.str;
        default = "graphical-session.target";
      };
    };
    interval = mkOption {
      type = types.str;
      description = "Interval at which to check battery levels.";
      default = "60s";
    };
    notificationLevel = mkOption {
      type = types.enum [ "Warn" "Info" "Trace" ];
      description = "Level of notifications to display, if any.";
      default = "Warn";
    };
    loggingLevel = mkOption {
      type = types.enum [ "Error" "Warn" "Info" "Debug" "Trace" ];
      description = "Logging verbosity";
      default = "Info";
    };
    warnMin = mkOption {
      type = types.ints.between 0.0 100.0;
      description = "Minimum battery level below which to start sending warning notifications.";
      default = 20;
    };
    stopMin = mkOption {
      type = types.ints.between 0.0 100.0;
      description = "Minimum battery level below which to hibernate the system.";
      default = 6;
    };
  };
  imports = [ ];
  config = lib.mkIf cfg.enable (lib.mkMerge [
    {
      home.packages = with pkgs; [ cfg.package ];
    }
    (lib.mkIf cfg.systemd.enable {
      systemd.user.timers."check-battery@" = {
        Unit.Description = "battery level notifications";
        Unit.PartOf = [ cfg.systemd.target ];
        Timer.OnUnitActiveSec = cfg.interval;
        Timer.OnActiveSec = "0s";
      };
      systemd.user.services."check-battery@" = {
        Unit.PartOf = [ cfg.systemd.target ];
        Service.Type = "oneshot";
        Service.ExecStart = "${cfg.package}/bin/check-battery -l ${cfg.loggingLevel} -n ${cfg.notificationLevel} -w ${toString cfg.warnMin} -s ${toString cfg.stopMin} %i";
      };
    })
  ]);
}
