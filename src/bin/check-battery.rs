use clap::Parser;

use notify_rust::{Hint, Notification, Urgency};

use scripts_rs::{
    battery::{Battery, BatteryStatus},
    init_fern, NOTIF_ICON,
};
use std::process::Command;

#[derive(Debug, Parser)]
#[clap(version, about = "Checks battery levels, outputs battery percentages")]
pub struct Args {
    /// output logging level
    #[clap(short, long, default_value = "Info", possible_values = ["Error", "Warn", "Info", "Debug", "Trace"])]
    pub log_lvl: log::LevelFilter,
    /// notification level; enables notifications if specified (Warn = critical level warnings, Info = +Max Level Notification, Trace = every run)
    #[clap(short, long, possible_values = ["None", "Warn", "Info", "Trace"])]
    pub notif_lvl: Option<log::LevelFilter>,
    /// battery level at which to begin sending warning notifications
    #[clap(short, long, default_value = "20.0", requires = "notif-lvl")]
    pub warn_min: f32,
    /// average battery level at which to hibernate the system
    #[clap(short, long)]
    pub stop_min: Option<f32>,
    /// batteries to check
    #[clap()]
    pub batteries: Vec<String>,
}

fn main() {
    let Args {
        log_lvl,
        notif_lvl,
        warn_min,
        stop_min,
        batteries,
    } = Args::parse();
    init_fern(std::io::stdout(), log_lvl);
    log::trace!("Checking levels of {:?}", batteries);
    let mut percents = Vec::new();
    for battery in &batteries {
        let battery = match Battery::from_name(battery) {
            Ok(b) => b,
            Err(_e) => continue,
        };

        let percent = battery.percent_actual();
        percents.push(battery.percent_actual());

        if let Some(notif_lvl) = notif_lvl {
            log::trace!("{:?}", NOTIF_ICON.to_str());
            let mut base_notif = Notification::new();
            base_notif
                .appname("check-battery")
                .summary(&format!("Battery: {}%", percent * 100.0))
                .body(&format!("{} ({})", battery.name, battery.status))
                .icon(NOTIF_ICON.to_str().unwrap())
                .hint(Hint::Category("system".to_string()))
                .hint(Hint::Custom(
                    "x-dunst-stack-tag".to_owned(),
                    battery.name.clone(),
                ))
                .hint(Hint::CustomInt(
                    "value".to_owned(),
                    (percent * 100.0) as i32,
                ));
            match (battery.status, notif_lvl) {
                (BatteryStatus::Unknown | BatteryStatus::Discharging, _)
                    if percent * 100.0 <= warn_min =>
                {
                    base_notif
                        .urgency(Urgency::Critical)
                        .timeout(0)
                        .show()
                        .unwrap();
                }
                (BatteryStatus::Charging, log::LevelFilter::Info) if percent >= 1.0 => {
                    base_notif.urgency(Urgency::Low).show().unwrap();
                }
                (_, log::LevelFilter::Trace) => {
                    base_notif.urgency(Urgency::Low).show().unwrap();
                }
                _ => {}
            }
        }
    }
    for (perc, bat) in percents.iter().zip(batteries) {
        println!("{}={}", bat, perc);
    }
    if let Some(stop_min) = stop_min {
        let percent: f32 = percents.iter().sum::<f32>() / percents.len() as f32;
        if percent <= stop_min {
            Command::new("systemctl").arg("hibernate").spawn().unwrap();
        }
    }
}
