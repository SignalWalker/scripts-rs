use std::{
    fmt::Display,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

use lazy_static::lazy_static;
use regex::Regex;

use serde::{Deserialize, Serialize};

const ACPI_PATH: &str = "/sys/class/power_supply";

#[derive(Eq, PartialEq, Hash, Serialize, Deserialize, Copy, Clone, Debug)]
pub enum BatteryStatus {
    Unknown,
    Discharging,
    Charging,
    NotCharging,
    Full,
}

impl Display for BatteryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BatteryStatus::Unknown => "Unknown",
            BatteryStatus::Discharging => "Discharging",
            BatteryStatus::Charging => "Charging",
            BatteryStatus::NotCharging => "Not charging",
            BatteryStatus::Full => "Full",
        })
    }
}

impl FromStr for BatteryStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unknown" => Ok(Self::Unknown),
            "discharging" => Ok(Self::Discharging),
            "charging" => Ok(Self::Charging),
            "not charging" => Ok(Self::NotCharging),
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Battery {
    pub name: String,
    pub remaining: u32,
    pub actual_max: u32,
    pub factory_max: u32,
    pub status: BatteryStatus,
}

struct ByteLines<Base: Read> {
    base: BufReader<Base>,
}

impl<Base: Read> Iterator for ByteLines<Base> {
    type Item = std::io::Result<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = Vec::new();
        match self.base.read_until(b'\n', &mut buf) {
            Err(e) => Some(Err(e)),
            Ok(0) => None,
            Ok(_i) => Some(Ok(buf)),
        }
    }
}

impl<Base: Read> ByteLines<Base> {
    pub fn new(base: Base) -> Self {
        Self {
            base: BufReader::new(base),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UEventEntryRaw(String, Vec<u8>);

#[derive(Debug, Clone)]
pub struct UEventEntry(String, String);

impl Battery {
    pub fn from_name(name: &str) -> std::io::Result<Self> {
        let bat_path: PathBuf = Path::new(ACPI_PATH).join(name);
        let uevent = match File::open(&bat_path.join("uevent")) {
            Ok(f) => f,
            Err(e) => {
                log::error!(
                    "Failed to open uevent file ({}/uevent) for {}: {:?}",
                    bat_path.to_str().unwrap(),
                    name,
                    e
                );
                return Err(e);
            }
        };
        let mut remaining = None;
        let mut actual_max = None;
        let mut factory_max = None;
        let mut status = None;
        for line_res in ByteLines::new(uevent) {
            match line_res {
                Err(e) => {
                    log::error!(
                        "Failed to read uevent file ({}/uevent) for {}: {:?}",
                        bat_path.to_str().unwrap(),
                        name,
                        e
                    );
                    return Err(e);
                }
                Ok(line) => {
                    let mut split = line.split(|b| *b == b'=');
                    let key = std::str::from_utf8(split.next().unwrap()).unwrap();
                    let mut val = || std::str::from_utf8(split.next().unwrap()).unwrap().trim();
                    match key {
                        "POWER_SUPPLY_STATUS" => {
                            let val = val();
                            status = Some(match BatteryStatus::from_str(val) {
                                Ok(b) => b,
                                Err(_e) => {
                                    log::warn!(
                                        "Unrecognized status ({}/status) for {}: {}",
                                        bat_path.to_str().unwrap(),
                                        name,
                                        val
                                    );
                                    BatteryStatus::Unknown
                                }
                            });
                        }
                        "POWER_SUPPLY_ENERGY_NOW" | "POWER_SUPPLY_CHARGE_NOW" => {
                            let val = val();
                            remaining =
                                Some(u32::from_str(val).unwrap_or_else(|_| {
                                    panic!("Failed to read remaining: {}", val)
                                }));
                        }
                        "POWER_SUPPLY_ENERGY_FULL" | "POWER_SUPPLY_CHARGE_FULL" => {
                            let val = val();
                            actual_max = Some(
                                u32::from_str(val)
                                    .unwrap_or_else(|_| panic!("Failed to read maximum: {}", val)),
                            );
                        }
                        "POWER_SUPPLY_ENERGY_FULL_DESIGN" | "POWER_SUPPLY_CHARGE_FULL_DESIGN" => {
                            let val = val();
                            factory_max = Some(u32::from_str(val).unwrap_or_else(|_| {
                                panic!("Failed to read factory maximum: {}", val)
                            }));
                        }
                        _ => continue,
                    }
                }
            }
            if remaining.is_some()
                && actual_max.is_some()
                && factory_max.is_some()
                && status.is_some()
            {
                break;
            }
        }
        let remaining = remaining.unwrap();
        let actual_max = actual_max.unwrap();
        let factory_max = factory_max.unwrap();
        let status = status.unwrap();

        Ok(Self {
            name: name.to_owned(),
            remaining,
            actual_max,
            factory_max,
            status,
        })
    }

    pub fn part_actual(&self) -> f32 {
        self.remaining as f32 / self.actual_max as f32
    }

    pub fn part_factory(&self) -> f32 {
        self.remaining as f32 / self.factory_max as f32
    }

    pub fn is_full(&self, threshold: f32) -> bool {
        self.status == BatteryStatus::Full
            || self.status == BatteryStatus::NotCharging
            || (self.status == BatteryStatus::Charging && self.part_actual() >= threshold)
    }
}
