use std::{fmt::Display, fs::File, io::Read, str::FromStr};

use lazy_static::lazy_static;
use regex::Regex;

const ACPI_PATH: &str = "/sys/class/power_supply";

pub enum BatteryStatus {
    Unknown,
    Discharging,
    Charging,
    Full,
}

impl Display for BatteryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BatteryStatus::Unknown => "Unknown",
            BatteryStatus::Discharging => "Discharging",
            BatteryStatus::Charging => "Charging",
            BatteryStatus::Full => "Full",
        })
    }
}

impl FromStr for BatteryStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().trim() {
            "unknown" => Ok(Self::Unknown),
            "discharging" => Ok(Self::Discharging),
            "charging" => Ok(Self::Charging),
            "full" => Ok(Self::Full),
            _ => Err(()),
        }
    }
}

lazy_static! {
    static ref REM_REG: Regex = Regex::new("POWER_SUPPLY_(?:ENERGY|CHARGE)_NOW=([0-9]+)").unwrap();
    static ref FULL_REG: Regex =
        Regex::new("POWER_SUPPLY_(?:ENERGY|CHARGE)_FULL=([0-9]+)").unwrap();
    static ref FULL_DESIGN_REG: Regex =
        Regex::new("POWER_SUPPLY_(?:ENERGY|CHARGE)_FULL_DESIGN=([0-9]+)").unwrap();
}

pub struct Battery {
    pub name: String,
    pub remaining: u32,
    pub actual_max: u32,
    pub factory_max: u32,
    pub status: BatteryStatus,
}

impl Battery {
    pub fn from_name(name: &str) -> std::io::Result<Self> {
        let bat_path = std::path::Path::new(&format!("{}/{}", ACPI_PATH, name))
            .to_string_lossy()
            .to_string();
        let mut uevent = match File::open(format!("{}/uevent", bat_path)) {
            Ok(f) => f,
            Err(e) => {
                log::error!(
                    "Failed to open uevent file ({}/uevent) for {}: {:?}",
                    bat_path,
                    name,
                    e
                );
                return Err(e);
            }
        };
        let uevent = {
            let mut buf = String::new();
            if let Err(e) = uevent.read_to_string(&mut buf) {
                log::error!(
                    "Failed to read uevent file ({}/uevent) for {}: {:?}",
                    bat_path,
                    name,
                    e
                );
                return Err(e);
            }
            buf
        };
        let remaining = u32::from_str(&REM_REG.captures(&uevent).unwrap()[1]).unwrap();
        let actual_max = u32::from_str(&FULL_REG.captures(&uevent).unwrap()[1]).unwrap();
        let factory_max = u32::from_str(&FULL_DESIGN_REG.captures(&uevent).unwrap()[1]).unwrap();

        let status = match File::open(format!("{}/status", bat_path)) {
            Ok(mut f) => {
                let mut buf = String::new();
                if let Err(e) = f.read_to_string(&mut buf) {
                    log::error!(
                        "Failed to read status ({}/status) for {}: {:?}",
                        bat_path,
                        name,
                        e
                    );
                    return Err(e);
                }
                match BatteryStatus::from_str(&buf) {
                    Ok(b) => b,
                    Err(_e) => {
                        log::error!(
                            "Unrecognized status ({}/status) for {}: {:?}",
                            bat_path,
                            name,
                            buf
                        );
                        BatteryStatus::Unknown
                    }
                }
            }
            Err(e) => {
                log::error!(
                    "Failed to open status file ({}/status) for {}: {:?}",
                    bat_path,
                    name,
                    e
                );
                return Err(e);
            }
        };

        Ok(Self {
            name: name.to_owned(),
            remaining,
            actual_max,
            factory_max,
            status,
        })
    }

    pub fn percent_actual(&self) -> f32 {
        self.remaining as f32 / self.actual_max as f32
    }

    pub fn percent_factory(&self) -> f32 {
        self.remaining as f32 / self.factory_max as f32
    }
}
