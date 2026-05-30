use std::{cmp::Ordering, fs, io, path::Path, process::Command};

use color_eyre::Result;

use super::{DirColumn, DirEntry, EffectiveSettings};

pub(super) fn build_columns_from_path(
    path: &Path,
    settings: &EffectiveSettings,
    sudo_mode: bool,
    sudo_password: Option<&str>,
) -> Vec<DirColumn> {
    let target_dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| path.to_path_buf())
    };

    let mut chain = Vec::new();
    let mut cur = target_dir.clone();
    loop {
        chain.push(cur.clone());
        let Some(parent) = cur.parent() else {
            break;
        };
        if parent == cur {
            break;
        }
        cur = parent.to_path_buf();
    }
    chain.reverse();

    let mut cols = chain
        .iter()
        .map(|p| DirColumn::from_path(p.clone(), settings, sudo_mode, sudo_password))
        .collect::<Vec<_>>();

    for idx in 0..cols.len().saturating_sub(1) {
        let child = &chain[idx + 1];
        if let Some(selected_idx) = cols[idx].entries.iter().position(|e| &e.path == child) {
            cols[idx].selected = selected_idx;
        }
    }

    if cols.is_empty() {
        vec![DirColumn::from_path(target_dir, settings, sudo_mode, sudo_password)]
    } else {
        cols
    }
}

#[derive(Debug, Clone)]
pub(super) struct DirReadOutcome {
    pub(super) entries: Vec<DirEntry>,
    pub(super) permission_denied: bool,
    pub(super) sudo_password_required: bool,
}

pub(super) fn read_dir_entries(
    path: &Path,
    settings: &EffectiveSettings,
    sudo_mode: bool,
    sudo_password: Option<&str>,
) -> DirReadOutcome {
    if sudo_mode {
        match read_dir_entries_sudo(path, settings, sudo_password) {
            Ok(entries) => {
                return DirReadOutcome {
                    entries,
                    permission_denied: false,
                    sudo_password_required: false,
                };
            }
            Err(SudoReadError::PasswordRequired) => {
                return DirReadOutcome {
                    entries: Vec::new(),
                    permission_denied: true,
                    sudo_password_required: true,
                };
            }
            Err(SudoReadError::PermissionDenied) => {
                return DirReadOutcome {
                    entries: Vec::new(),
                    permission_denied: true,
                    sudo_password_required: false,
                };
            }
            Err(SudoReadError::Other) => {}
        }
    }

    let read_dir = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => {
            return DirReadOutcome {
                entries: Vec::new(),
                permission_denied: e.kind() == io::ErrorKind::PermissionDenied,
                sudo_password_required: false,
            };
        }
    };
    let mut entries = Vec::new();
    for dir_entry in read_dir.flatten() {
        let file_name = dir_entry.file_name().to_string_lossy().to_string();
        if !settings.show_hidden && file_name.starts_with('.') {
            continue;
        }
        let entry_path = dir_entry.path();
        let is_dir = entry_path.is_dir();
        let is_symlink = fs::symlink_metadata(&entry_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let is_executable = fs::metadata(&entry_path)
            .map(|m| {
                use std::os::unix::fs::PermissionsExt;
                m.permissions().mode() & 0o111 != 0
            })
            .unwrap_or(false);
        #[cfg(target_os = "windows")]
        let is_executable = entry_path
            .extension()
            .and_then(|x| x.to_str())
            .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "exe" | "bat" | "cmd" | "com"))
            .unwrap_or(false);
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let is_executable = false;
        entries.push(DirEntry {
            name: file_name,
            path: entry_path,
            is_dir,
            is_symlink,
            is_executable,
        });
    }
    entries.sort_by(|a, b| sort_entries(a, b, settings.sort.as_str()));
    DirReadOutcome {
        entries,
        permission_denied: false,
        sudo_password_required: false,
    }
}

fn sort_entries(a: &DirEntry, b: &DirEntry, sort_mode: &str) -> Ordering {
    let dir_first = b.is_dir.cmp(&a.is_dir);
    if dir_first != Ordering::Equal {
        return dir_first;
    }

    match sort_mode {
        "type" => extension_of(&a.name)
            .cmp(&extension_of(&b.name))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "size" => fs::metadata(&a.path)
            .ok()
            .map(|m| m.len())
            .unwrap_or(0)
            .cmp(&fs::metadata(&b.path).ok().map(|m| m.len()).unwrap_or(0))
            .reverse()
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "modified" => fs::metadata(&a.path)
            .and_then(|m| m.modified())
            .ok()
            .cmp(&fs::metadata(&b.path).and_then(|m| m.modified()).ok())
            .reverse()
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    }
}

enum SudoReadError {
    PasswordRequired,
    PermissionDenied,
    Other,
}

fn read_dir_entries_sudo(
    path: &Path,
    settings: &EffectiveSettings,
    sudo_password: Option<&str>,
) -> Result<Vec<DirEntry>, SudoReadError> {
    let mut cmd = Command::new("sudo");
    cmd.arg("-S")
        .arg("-p")
        .arg("")
        .arg("ls")
        .arg("-la")
        .arg("--group-directories-first")
        .arg(path);
    let output = if let Some(pw) = sudo_password {
        use std::process::Stdio;
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| SudoReadError::Other)?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = std::io::Write::write_all(&mut stdin, format!("{pw}\n").as_bytes());
        }
        child.wait_with_output().map_err(|_| SudoReadError::Other)?
    } else {
        cmd.arg("-n").output().map_err(|_| SudoReadError::Other)?
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        if stderr.contains("password") {
            return Err(SudoReadError::PasswordRequired);
        }
        if stderr.contains("permission denied") {
            return Err(SudoReadError::PermissionDenied);
        }
        return Err(SudoReadError::Other);
    }
    let out = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    for line in out.lines() {
        if line.starts_with("total ") || line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let perms = match parts.next() {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 9 {
            continue;
        }
        let name = cols[8..].join(" ");
        if name == "." || name == ".." {
            continue;
        }
        if !settings.show_hidden && name.starts_with('.') {
            continue;
        }
        let entry_path = path.join(&name);
        let is_dir = perms.starts_with('d');
        let is_symlink = perms.starts_with('l');
        let is_executable = perms.chars().nth(3) == Some('x')
            || perms.chars().nth(6) == Some('x')
            || perms.chars().nth(9) == Some('x');
        entries.push(DirEntry {
            name,
            path: entry_path,
            is_dir,
            is_symlink,
            is_executable,
        });
    }
    entries.sort_by(|a, b| sort_entries(a, b, settings.sort.as_str()));
    Ok(entries)
}

fn extension_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_lowercase())
        .unwrap_or_default()
}

pub(super) fn copy_path(source: &Path, destination: &Path) -> io::Result<()> {
    if source.is_dir() {
        copy_dir_recursive(source, destination)
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination).map(|_| ())
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let src = entry.path();
        let dst = destination.join(entry.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

pub(super) fn move_path(source: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(source, destination) {
        Ok(_) => Ok(()),
        Err(_) => {
            copy_path(source, destination)?;
            if source.is_dir() {
                fs::remove_dir_all(source)
            } else {
                fs::remove_file(source)
            }
        }
    }
}
