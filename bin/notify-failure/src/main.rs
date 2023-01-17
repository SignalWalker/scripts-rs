use clap::Parser;
use notify_rust::{Hint, Notification};
use script_lib::notif::NOTIF_ICON;

#[derive(Debug, Parser)]
#[command(version, about = "Notifies the user about failed systemd services")]
pub struct Args {
    #[arg(short, long, default_value = "Info")]
    pub log_lvl: log::LevelFilter,
    #[arg()]
    pub unit_name: String,
}

pub fn main() {
    let Args { log_lvl, unit_name } = Args::parse();
    // init_fern(std::io::stderr(), log_lvl);
    Notification::new()
        .appname("notify-failure")
        .summary(&format!("{unit_name}"))
        .body("Systemd unit failed")
        .icon(NOTIF_ICON.to_str().unwrap())
        .hint(Hint::Category("system".to_string()))
        .urgency(notify_rust::Urgency::Critical)
        .timeout(0)
        .show()
        .unwrap();
}
