use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub workers: usize,
    pub keep_if_larger: bool,
    pub suffix: String,
    pub ascii: bool,
    pub mouse: bool,
    pub last_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(4);
        Self {
            workers: cores.clamp(1, 4),
            keep_if_larger: true,
            suffix: "-imgshrink".into(),
            ascii: false,
            mouse: true,
            last_dir: None,
        }
    }
}

pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(appdata) = env_dir("APPDATA") {
            return appdata.join("ImgShrink");
        }
        if let Some(home) = home() {
            return home.join("AppData").join("Roaming").join("ImgShrink");
        }
        return fallback_base();
    }
    if cfg!(target_os = "macos") {
        return home()
            .map(|home| home.join("Library/Application Support/ImgShrink"))
            .unwrap_or_else(fallback_base);
    }
    env_dir("XDG_CONFIG_HOME")
        .map(|dir| dir.join("imgshrink"))
        .or_else(|| home().map(|home| home.join(".config").join("imgshrink")))
        .unwrap_or_else(fallback_base)
}

pub fn state_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(local) = env_dir("LOCALAPPDATA") {
            return local.join("ImgShrink");
        }
        if let Some(home) = home() {
            return home.join("AppData").join("Local").join("ImgShrink");
        }
        return fallback_base().join("state");
    }
    if cfg!(target_os = "macos") {
        return home()
            .map(|home| home.join("Library/Logs/ImgShrink"))
            .unwrap_or_else(|| fallback_base().join("logs"));
    }
    env_dir("XDG_STATE_HOME")
        .map(|dir| dir.join("imgshrink"))
        .or_else(|| home().map(|home| home.join(".local").join("state").join("imgshrink")))
        .unwrap_or_else(|| fallback_base().join("state"))
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn home() -> Option<PathBuf> {
    env_dir("HOME")
        .or_else(|| env_dir("USERPROFILE"))
        .or_else(|| {
            let drive = env_dir("HOMEDRIVE")?;
            let path = env_dir("HOMEPATH")?;
            Some(drive.join(path))
        })
}

fn fallback_base() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".imgshrink")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn journal_path() -> PathBuf {
    state_dir().join("journal.json")
}

pub fn log_dir() -> PathBuf {
    state_dir().join("logs")
}

pub fn load() -> Config {
    let path = config_path();
    fs_read(&path)
        .and_then(|text| toml::from_str::<Config>(&text).ok())
        .unwrap_or_default()
}

pub fn save(config: &Config) -> std::io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text =
        toml::to_string_pretty(config).map_err(|error| std::io::Error::other(error.to_string()))?;
    std::fs::write(path, text)
}

fn fs_read(path: &PathBuf) -> Option<String> {
    std::fs::read_to_string(path).ok()
}
