use clap::Parser;

use notify_rust::{Hint, Notification, Urgency};

use bincode::Options;
use directories::{BaseDirs, ProjectDirs, UserDirs};
use lazy_static::lazy_static;
use script_lib::{
    battery::{Battery, BatteryStatus},
    log::init_fern,
    notif::NOTIF_ICON,
};
use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Seek;
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

lazy_static! {
    static ref BINCODE_OPTS: bincode::DefaultOptions = bincode::DefaultOptions::new();
}

fn main() -> std::io::Result<()> {
    let args @ Args {
        log_lvl,
        notif_lvl,
        warn_min,
        stop_min,
        ..
    } = Args::parse();
    init_fern(std::io::stderr(), log_lvl);
    log::debug!("Checking levels of {:?}", args.batteries);

    log::debug!("Ensuring existence of $XDG_RUNTIME_DIR/check-battery...");
    let base_dirs = BaseDirs::new().expect("failed to get XDG base dirs");
    let runtime_path = base_dirs
        .runtime_dir()
        .expect("failed to get runtime dir")
        .join("check-battery");
    fs::create_dir_all(&runtime_path)?;

    let mem_path = runtime_path.join("mem");
    log::debug!("Deserializing {:?}", &mem_path);
    let mut mem_file: File = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&mem_path)
        .unwrap_or_else(|_| panic!("Could not open {:?}", &mem_path));
    let mem_batteries: HashMap<String, Battery> = match BINCODE_OPTS.deserialize_from(&mem_file) {
        Ok(m) => m,
        Err(e) => {
            log::error!("Deserialization: {:?}", e);
            HashMap::default()
        }
    };
    log::debug!("mem_batteries: {:?}", &mem_batteries);

    let mut batteries: HashMap<String, Battery> = HashMap::new();
    for bat_name in &args.batteries {
        let battery = match Battery::from_name(bat_name) {
            Ok(b) => b,
            Err(_e) => continue,
        };
        batteries.insert(battery.name.clone(), battery);
        let battery = &batteries[bat_name];

        let rem = battery.part_actual();
        let percent = rem * 100.0;

        if let Some(notif_lvl) = notif_lvl {
            log::debug!("notification icon: {:?}", NOTIF_ICON.to_str());
            let mut base_notif = Notification::new();
            base_notif
                .appname("check-battery")
                .summary(&format!("Battery: {percent}%"))
                .body(&format!("{} ({})", battery.name, battery.status))
                .icon(NOTIF_ICON.to_str().unwrap())
                .hint(Hint::Category("system".to_string()))
                .hint(Hint::Custom(
                    "x-dunst-stack-tag".to_owned(),
                    battery.name.clone(),
                ))
                .hint(Hint::CustomInt("value".to_owned(), percent as i32));
            match (battery.status, notif_lvl) {
                (BatteryStatus::Unknown | BatteryStatus::Discharging, _) if percent <= warn_min => {
                    Some(base_notif.urgency(Urgency::Critical).timeout(0))
                }
                (_, log::LevelFilter::Info)
                    if battery.is_full(0.95)
                        && !mem_batteries
                            .get(&battery.name)
                            .map_or(false, |bat| bat.is_full(0.95)) =>
                {
                    Some(base_notif.urgency(Urgency::Low))
                }
                (_, log::LevelFilter::Trace) => Some(base_notif.urgency(Urgency::Low)),
                _ => None,
            }
            .and_then(|not| not.show().ok());
        }
    }
    for (name, bat) in batteries.iter() {
        println!("{}={}", name, bat.part_actual());
    }
    if let Some(stop_min) = stop_min {
        let percent: f32 = 100.0 * batteries.values().map(|bat| bat.part_actual()).sum::<f32>()
            / batteries.len() as f32;
        if percent <= stop_min {
            log::warn!("Total battery percent ({percent}%) is below the hibernation threshold ({stop_min}%).");
            // Command::new("systemctl").arg("hibernate").spawn().unwrap();
        }
    }

    log::debug!("Truncating {:?}...", &mem_path);
    mem_file
        .set_len(0)
        .unwrap_or_else(|_| panic!("Unable to truncate {:?}", &mem_path));
    mem_file
        .rewind()
        .unwrap_or_else(|_| panic!("Unable to rewind {:?}", &mem_path));
    log::debug!("Serializing {:?} into {:?}", &batteries, &mem_path);
    if let Err(e) = BINCODE_OPTS.serialize_into(mem_file, &batteries) {
        log::error!("Serialization: {:?}", e);
    }
    Ok(())
}
