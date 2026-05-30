use std::{collections::HashMap, fs, io, path::{Path, PathBuf}};

use color_eyre::Result;
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

use super::{KeymapApp, KeymapConfig, KeymapFileOps, KeymapNavigation, KeymapSearch, KeymapSelection, ThemeRegistry, expand_tilde};

pub(crate) fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path.push("dirt.toml");
    path
}

pub(crate) fn layout_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path.push("layout.toml");
    path
}

pub(crate) fn theme_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path.push("theme.toml");
    path
}

pub(crate) fn keymap_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path.push("keymap.toml");
    path
}

pub(crate) fn load_recents() -> Result<Vec<PathBuf>> {
    let mut recents_path = state_dir();
    recents_path.push("recents.toml");
    if !recents_path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(recents_path)?;
    let recents: RecentsState = toml::from_str(&raw)?;
    Ok(recents
        .recent_dirs
        .into_iter()
        .map(|p| expand_tilde(&p))
        .collect())
}

pub(crate) fn save_recents(recents: &[PathBuf]) -> io::Result<()> {
    let mut recents_path = state_dir();
    fs::create_dir_all(&recents_path)?;
    recents_path.push("recents.toml");
    let state = RecentsState {
        recent_dirs: recents.iter().map(|p| p.display().to_string()).collect(),
    };
    let body = toml::to_string(&state).unwrap_or_else(|_| String::from("recent_dirs = []\n"));
    fs::write(recents_path, body)
}

pub(crate) fn discover_drives() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        let mut mounts = vec!["/".to_string()];
        if let Ok(content) = fs::read_to_string("/proc/mounts") {
            for line in content.lines().take(20) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 1 {
                    let mountpoint = parts[1];
                    if mountpoint.starts_with("/mnt") || mountpoint.starts_with("/media") {
                        mounts.push(mountpoint.to_string());
                    }
                }
            }
        }
        mounts.sort();
        mounts.dedup();
        mounts
    }

    #[cfg(target_os = "macos")]
    {
        let mut mounts = vec!["/".to_string(), "/Volumes".to_string()];
        mounts.sort();
        mounts.dedup();
        mounts
    }

    #[cfg(target_os = "windows")]
    {
        vec!["C:\\".to_string()]
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        vec!["/".to_string()]
    }
}

pub(crate) fn load_themes_registry() -> Result<ThemeRegistry> {
    let mut by_name = HashMap::new();
    by_name.insert("dark".to_string(), load_default_theme_fallback());
    Ok(ThemeRegistry { by_name })
}

pub(crate) fn ensure_local_defaults_files() -> io::Result<()> {
    let dir = PathBuf::from("defaults");
    fs::create_dir_all(&dir)?;
    let layout = dir.join("layout.toml");
    if !layout.exists() {
        fs::write(&layout, default_config_contents())?;
    }
    let theme = dir.join("theme.toml");
    if !theme.exists() {
        fs::write(&theme, default_theme_contents())?;
    }
    let keymap = dir.join("keymap.toml");
    if !keymap.exists() {
        fs::write(&keymap, default_keymap_contents())?;
    }
    Ok(())
}

pub(crate) fn init_config_file() -> io::Result<String> {
    let path = config_path();
    if path.exists() {
        return Ok("Config already exists at ~/.config/dirt/dirt.toml".to_string());
    }
    write_default_config(&path)?;
    Ok("Config created at ~/.config/dirt/dirt.toml".to_string())
}

pub(crate) fn init_layout_file() -> io::Result<String> {
    let path = layout_config_path();
    if path.exists() {
        return Ok(format!("Layout already exists at {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, default_config_contents())?;
    Ok(format!("Layout created at {}", path.display()))
}

pub(crate) fn init_theme_file() -> io::Result<String> {
    let path = theme_config_path();
    if path.exists() {
        return Ok(format!("Theme already exists at {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, default_theme_contents())?;
    Ok(format!("Theme created at {}", path.display()))
}

pub(crate) fn init_keymap_file() -> io::Result<String> {
    let path = keymap_path();
    if path.exists() {
        return Ok(format!("Keymap already exists at {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, default_keymap_contents())?;
    Ok(format!("Keymap created at {}", path.display()))
}

pub(crate) fn load_keymap_config() -> KeymapConfig {
    let path = keymap_path();
    if path.exists()
        && let Ok(raw) = fs::read_to_string(path)
        && let Ok(k) = toml::from_str::<KeymapConfig>(&raw)
    {
        return k;
    }
    toml::from_str(default_keymap_contents()).unwrap_or_else(|_| KeymapConfig {
        navigation: KeymapNavigation {
            up: vec!["Up".to_string(), "k".to_string()],
            down: vec!["Down".to_string(), "j".to_string()],
            open: vec!["Right".to_string(), "l".to_string(), "Enter".to_string()],
            parent: vec!["Left".to_string(), "h".to_string(), "Backspace".to_string()],
        },
        selection: KeymapSelection {
            range_up: vec!["Shift+Up".to_string()],
            range_down: vec!["Shift+Down".to_string()],
            mode: "Ctrl+S".to_string(),
            toggle: "Space".to_string(),
            exit: "Esc".to_string(),
        },
        search: KeymapSearch {
            local: "/".to_string(),
            global: "Ctrl+F".to_string(),
        },
        file_ops: KeymapFileOps {
            new_file: "Ctrl+N".to_string(),
            new_dir: "Ctrl+Shift+N".to_string(),
            copy: "Ctrl+C".to_string(),
            cut: "Ctrl+X".to_string(),
            paste: "Ctrl+V".to_string(),
            trash: "Ctrl+D".to_string(),
        },
        app: KeymapApp {
            quit: "q".to_string(),
            next_profile: "p".to_string(),
            prev_profile: "P".to_string(),
        },
    })
}

pub(crate) fn default_config_contents() -> &'static str {
    include_str!("../../defaults/layout.toml")
}

pub(crate) fn default_keymap_contents() -> &'static str {
    include_str!("../../defaults/keymap.toml")
}

pub(crate) fn default_theme_contents() -> &'static str {
    include_str!("../../defaults/theme.toml")
}

fn state_dir() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path
}

pub(crate) fn load_default_theme_fallback() -> Theme {
    let path = PathBuf::from("defaults/theme.toml");
    if let Ok(raw) = fs::read_to_string(path)
        && let Ok(value) = toml::from_str::<toml::Value>(&raw)
    {
        return Theme::from_toml_value(&value);
    }
    if let Ok(value) = toml::from_str::<toml::Value>(default_theme_contents()) {
        return Theme::from_toml_value(&value);
    }
    Theme::hardcoded_defaults()
}

fn write_default_config(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_config_contents())
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct RecentsState {
    #[serde(default)]
    pub(crate) recent_dirs: Vec<String>,
}
