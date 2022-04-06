use clap::Parser;
use lazy_static::lazy_static;
use regex::Regex;
use signal_hook::consts::SIGUSR1;
use signal_hook::flag;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time;

lazy_static! {
    static ref LOCAL_REG: Regex = Regex::new(".*inet[6]? ([0-9\\./]*)").unwrap();
}

#[derive(Debug, Parser)]
#[clap(version, about = "Check IP address")]
pub struct Args {
    /// output logging level
    #[clap(short, long, default_value = "Info", possible_values = ["Error", "Warn", "Info", "Debug", "Trace"])]
    pub log_lvl: log::LevelFilter,
    /// check public ip
    #[clap(
        short,
        long,
        conflicts_with = "interface",
        required_unless_present = "interface"
    )]
    pub public: bool,
    /// interface for which to check local ip
    #[clap(
        short,
        long,
        conflicts_with = "public",
        required_unless_present = "public"
    )]
    pub interface: Option<String>,
    /// run as daemon; sleep X milliseconds between checks
    #[clap(short, long)]
    pub daemon_millis: Option<u64>,
}

enum IPSource {
    Private(String),
    Public,
}

fn get_local(interface: impl AsRef<str>) -> Option<String> {
    let local = Command::new("ip")
        .args(&["a", "show", interface.as_ref()])
        .output()
        .ok()?;
    Some(
        LOCAL_REG
            .captures(String::from_utf8(local.stdout).ok()?.trim())?
            .get(1)?
            .as_str()
            .to_string(),
    )
}

fn get_remote() -> Option<String> {
    let public = Command::new("curl")
        .args(&["-s", "https://am.i.mullvad.net/ip"])
        .output()
        .ok()?;
    Some(String::from_utf8(public.stdout).ok()?.trim().to_string()).filter(|s| !s.is_empty())
}

fn get_ip(src: &IPSource) -> Option<String> {
    match src {
        IPSource::Private(interface) => get_local(interface),
        IPSource::Public => get_remote(),
    }
}

fn wait(dur: time::Duration, sig: Arc<AtomicBool>) {
    let start = time::Instant::now();
    let sleep_dur = time::Duration::from_millis(200);
    while start.elapsed() < dur && !sig.load(Ordering::Relaxed) {
        thread::sleep(sleep_dur);
    }
    sig.swap(false, Ordering::Relaxed);
}

fn daemon_loop(src: &IPSource, sleep_dur: time::Duration) {
    let click_flag = Arc::new(AtomicBool::new(false));
    flag::register(SIGUSR1, Arc::clone(&click_flag)).unwrap();
    loop {
        println!("Checking...");
        let ip = get_ip(src);
        let ip_ref = ip.as_ref().map_or("[Not Found]", String::as_str);
        println!("{}", ip_ref);
        wait(sleep_dur, click_flag.clone());
    }
}

fn main() {
    let args = Args::parse();
    let src = match args.public {
        true => IPSource::Public,
        false => IPSource::Private(args.interface.unwrap()),
    };
    match args.daemon_millis {
        Some(millis) => daemon_loop(&src, time::Duration::from_millis(millis)),
        None => print!(
            "{}",
            get_ip(&src).unwrap_or_else(|| "[Not Found]".to_string())
        ),
    }
}
