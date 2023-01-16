use fern::Output;
use termion::color::{self, Color, Fg};

lazy_static::lazy_static! {
    pub static ref DATE_FORMAT: &'static [time::format_description::FormatItem<'static>] = time::macros::format_description!("[hour]:[minute]:[second]");
}

pub fn init_fern(output: impl Into<Output>, log_lvl: log::LevelFilter) {
    fern::Dispatch::new()
        .format(|out, message, record| {
            let ltime = time::OffsetDateTime::now_local().map_or_else(
                |_| String::default(),
                |ltime| {
                    ltime.format(&DATE_FORMAT).map_or_else(
                        |_| String::default(),
                        |ltime| format!("{}{ltime}{}", Fg(color::Blue), Fg(color::Reset)),
                    )
                },
            );
            let lcolor = match record.level() {
                log::Level::Error => Fg(color::Red).to_string(),
                log::Level::Warn => Fg(color::Yellow).to_string(),
                log::Level::Info => Fg(color::Green).to_string(),
                log::Level::Debug => Fg(color::Blue).to_string(),
                log::Level::Trace => Fg(color::Cyan).to_string(),
            };
            out.finish(format_args!(
                "[{}][{}{}{}][{}{}{}] {}",
                ltime,
                Fg(color::Yellow),
                record.target(),
                Fg(color::Reset),
                lcolor,
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
