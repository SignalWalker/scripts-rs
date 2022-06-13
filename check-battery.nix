{
  config,
  pkgs,
  lib,
  ...
}:
with builtins; let
  std = pkgs.lib;
  cfg = config.services.check-battery;
in {
  options.services.check-battery = with lib; {
    enable = mkEnableOption "battery level notifications as a systemd service";
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
  imports = [];
  config = lib.mkIf cfg.enable {
    home.packages = with pkgs; [
      ash-scripts.rust.check-battery
    ];
    systemd.user.timers."check-battery@" = {
      Unit.Description = "battery level notifications";
      Unit.PartOf = "graphical-session.target";
      Timer.OnUnitActiveSec = cfg.interval;
      Timer.OnActiveSec = "0s";
      Install.WantedBy = ["graphical-session.target"];
    };
    systemd.user.services."check-battery@" = {
      Service.Type = "oneshot";
      Service.ExecStart = "check-battery -l ${cfg.loggingLevel} -n ${cfg.notificationLevel} -w ${toString cfg.warnMin} -s ${toString cfg.stopMin} %i";
    };
  };
}
