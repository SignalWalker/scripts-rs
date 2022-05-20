use std::path::PathBuf;
lazy_static::lazy_static! {
    pub static ref NOTIF_ICON: PathBuf = PathBuf::from("/home/ash/.config/system_notif_icon.png").canonicalize().unwrap();
}
