#![feature(path_try_exists)]

use fern::Output;
use std::path::PathBuf;
use termion::color::{self, Color, Fg};

pub mod battery;
pub mod git;

lazy_static::lazy_static! {
    pub static ref NOTIF_ICON: PathBuf = PathBuf::from("/home/ash/.config/system_notif_icon.png").canonicalize().unwrap();
}

pub fn init_fern(output: impl Into<Output>, log_lvl: log::LevelFilter) {
    fern::Dispatch::new()
        .format(|out, message, record| {
            let color = match record.level() {
                log::Level::Error => Fg(color::Red).to_string(),
                log::Level::Warn => Fg(color::Yellow).to_string(),
                log::Level::Info => Fg(color::Green).to_string(),
                log::Level::Debug => Fg(color::Blue).to_string(),
                log::Level::Trace => Fg(color::Cyan).to_string(),
            };
            out.finish(format_args!(
                "{}{}{}[{}]{}[{}]{} {}",
                Fg(color::Blue),
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S%.6f]"),
                Fg(color::Reset),
                record.target(),
                color,
                record.level(),
                Fg(color::Reset),
                message
            ))
        })
        .level(log_lvl)
        .chain(output)
        .apply()
        .unwrap();
}
