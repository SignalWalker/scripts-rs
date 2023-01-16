use std::env;
use std::path::PathBuf;

lazy_static::lazy_static! {
    pub static ref NOTIF_ICON: PathBuf =
        option_env!("SYSTEM_NOTIFICATION_ICON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let path_str = format!("{}/system_notif_icon.png", env::var("XDG_CONFIG_HOME").expect("expected XDG_CONFIG_HOME to be set"));
            PathBuf::from(path_str)
        })
        .canonicalize()
        .expect(&format!("expected extant file"));
}
