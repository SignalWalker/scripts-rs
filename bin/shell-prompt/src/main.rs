use clap::Parser;
use console::{Color, Style, Term};
use git2::Repository;

use script_lib::{battery::Battery, log::init_fern};
use std::{io::Write, str::FromStr};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ColorState {
    Never,
    Auto,
    Always,
}

impl FromStr for ColorState {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "never" => Ok(Self::Never),
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Parser)]
#[clap(version, about = "Generate shell prompt")]
pub struct Args {
    #[clap(short, long, default_value = "Info")]
    pub log_lvl: log::LevelFilter,
    /// user name
    #[clap(short, long, env)]
    pub user: String,
    /// hostname
    #[clap(long, env)]
    pub host: Option<String>,
    /// current directory
    #[clap(long, env)]
    pub pwd: String,
    /// user home dir
    #[clap(long, env)]
    pub home: String,
    /// if given, the batteries with which to call check-battery
    #[clap(short, long, env = "PROMPT_BATTERY_LIST")]
    pub batteries: Option<Vec<String>>,
    /// whether to display git information, if in a git repo
    #[clap(short = 'g', long)]
    pub with_git: bool,
    /// whether to display time
    #[clap(short = 't', long)]
    pub with_time: bool,
    /// whether to display the date
    #[clap(short = 'd', long, requires = "with-time")]
    pub with_date: bool,
    /// the status of the previous command
    #[clap(short = 's', long)]
    pub prev_status: Option<String>,
    /// when to use color
    #[clap(short = 'c', long, default_value = "auto", default_missing_value = "always", possible_values = ["never", "auto", "always"])]
    pub color: ColorState,
    /// extra data before command prompt
    #[clap(long, default_missing_value = "")]
    pub prompt_ext: Option<String>,
    /// extra data after status line
    #[clap(long, default_missing_value = "")]
    pub status_ext: Option<String>,
    /// enable logging
    #[clap(long)]
    pub verbose: bool,
    /// disable replacing user home with ~ in directory display
    #[clap(long)]
    pub no_mangle: bool,
    /// zsh compatibility
    #[clap(long)]
    pub zsh: bool,
}

fn main() {
    let mut args = Args::parse();
    if args.verbose {
        init_fern(std::io::stderr(), args.log_lvl);
    }

    match args.color {
        ColorState::Never => console::set_colors_enabled(false),
        ColorState::Auto => {}
        ColorState::Always => console::set_colors_enabled(true),
    }

    let host = args.host.clone().unwrap_or_else(|| {
        std::fs::read_to_string("/etc/hostname")
            .ok()
            .unwrap_or_default()
            .trim()
            .to_owned()
    });

    if !args.no_mangle {
        args.pwd = args.pwd.replacen(&args.home, "~", 1);
    }

    let time_format = match args.with_date {
        true => time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
        false => time::macros::format_description!("[hour]:[minute]:[second]"),
    };
    let red = Style::new().fg(Color::Red);
    let green = Style::new().fg(Color::Green);
    let blue = Style::new().fg(Color::Blue);
    let yellow = Style::new().fg(Color::Yellow);
    let magenta = Style::new().fg(Color::Magenta);
    let mut term = Term::stdout();

    if args.with_time {
        write!(
            &mut term,
            "[{}] ",
            magenta.apply_to(
                match time::OffsetDateTime::now_local() {
                    Ok(o) => o,
                    Err(_e) => {
                        time::OffsetDateTime::now_utc()
                    }
                }
                .format(&time_format)
                .unwrap()
            )
        )
        .unwrap();
    }

    if let Some(batteries) = args.batteries.as_ref() {
        let rem = batteries
            .iter()
            .filter_map(|name| Battery::from_name(name).ok())
            .fold(0.0, |acc, b| acc + b.part_actual())
            / batteries.len() as f32;
        write!(
            &mut term,
            "{} ",
            yellow.apply_to(format!(
                "⚡{:.0}{}%",
                rem * 100.0,
                if args.zsh { "%" } else { "" }
            ))
        )
        .unwrap();
    }

    write!(
        &mut term,
        "{}@{}:{} ",
        blue.apply_to(&args.user),
        blue.apply_to(host),
        green.apply_to(&args.pwd)
    )
    .unwrap();

    if let Some(status_ext) = args.status_ext.as_ref() {
        write!(&mut term, "{}", status_ext).unwrap();
    }

    if args.with_git {
        if let Ok(repo) = Repository::open(&args.pwd) {
            write!(&mut term, "\n\t{{ ⑆ {} : 位 {} }}", "()", "()").unwrap();
        }
    }

    writeln!(&mut term).unwrap();
    match args.prev_status.as_deref() {
        None | Some("0") => {}
        Some(status) => {
            write!(&mut term, "[{}] ", red.apply_to(status)).unwrap();
        }
    }
    if let Some(prompt_ext) = args.prompt_ext.as_ref() {
        write!(&mut term, "{}", prompt_ext).unwrap();
    }
    write!(&mut term, "> ").unwrap();
}
