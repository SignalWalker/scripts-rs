use clap::Parser;
use notify_rust::{Hint, Notification};
use scripts_rs::{init_fern, NOTIF_ICON};

#[derive(Debug, Parser)]
#[clap(version, about = "Notifies the user about failed systemd services")]
pub struct Args {
    #[clap(short, long, default_value = "Info")]
    pub log_lvl: log::LevelFilter,
    #[clap()]
    pub unit_name: String,
}

pub fn main() {
    let Args { log_lvl, unit_name } = Args::parse();
    init_fern(std::io::stdout(), log_lvl);
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
