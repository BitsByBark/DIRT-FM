use std::{fs, io, path::Path};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::{
    app::{
        App, ClipboardItem, DialogState, InputMode, OverlayFrame, bookmark_matches_slot,
        copy_path, move_path, path_last_segment, read_dir_entries,
    },
};

impl App {
    fn build_keybinds_overlay(&self) -> OverlayFrame {
        let mut lines = Vec::new();
        lines.push(("NAVIGATION".to_string(), String::new(), true));
        lines.push((self.keymap.navigation.up.join("/"), "Move up".to_string(), false));
        lines.push((self.keymap.navigation.down.join("/"), "Move down".to_string(), false));
        lines.push((self.keymap.navigation.open.join("/"), "Open".to_string(), false));
        lines.push((self.keymap.navigation.parent.join("/"), "Parent".to_string(), false));
        lines.push(("".to_string(), "".to_string(), false));
        lines.push(("SELECTION".to_string(), String::new(), true));
        lines.push((self.keymap.selection.mode.clone(), "Toggle selection mode".to_string(), false));
        lines.push((self.keymap.selection.toggle.clone(), "Toggle selected".to_string(), false));
        lines.push((self.keymap.selection.range_up.join("/"), "Range up".to_string(), false));
        lines.push((self.keymap.selection.range_down.join("/"), "Range down".to_string(), false));
        lines.push((self.keymap.selection.exit.clone(), "Exit selection mode".to_string(), false));
        lines.push(("".to_string(), "".to_string(), false));
        lines.push(("SEARCH".to_string(), String::new(), true));
        lines.push((self.keymap.search.local.clone(), "Local search".to_string(), false));
        lines.push((self.keymap.search.global.clone(), "Global search".to_string(), false));
        lines.push(("".to_string(), "".to_string(), false));
        lines.push(("MODES".to_string(), String::new(), true));
        lines.push((self.keymap.selection.mode.clone(), "Toggle select mode".to_string(), false));
        lines.push(("Ctrl+B".to_string(), "Bookmark set mode".to_string(), false));
        lines.push(("Ctrl+1..9".to_string(), "Open bookmark slot".to_string(), false));
        lines.push(("/sudo".to_string(), "Toggle sudo mode".to_string(), false));
        lines.push((self.keymap.search.local.clone(), "Enter search mode".to_string(), false));
        lines.push((self.keymap.search.global.clone(), "Enter global search mode".to_string(), false));
        lines.push(("Esc".to_string(), "Exit active mode".to_string(), false));
        lines.push(("".to_string(), "".to_string(), false));
        lines.push(("FILE OPS".to_string(), String::new(), true));
        lines.push((self.keymap.file_ops.new_file.clone(), "New file".to_string(), false));
        lines.push((self.keymap.file_ops.new_dir.clone(), "New folder".to_string(), false));
        lines.push((self.keymap.file_ops.copy.clone(), "Copy".to_string(), false));
        lines.push((self.keymap.file_ops.cut.clone(), "Cut".to_string(), false));
        lines.push((self.keymap.file_ops.paste.clone(), "Paste".to_string(), false));
        lines.push((self.keymap.file_ops.trash.clone(), "Delete to trash".to_string(), false));
        lines.push(("".to_string(), "".to_string(), false));
        lines.push(("APP".to_string(), String::new(), true));
        lines.push((self.keymap.app.quit.clone(), "Quit".to_string(), false));
        lines.push((self.keymap.app.next_profile.clone(), "Next profile".to_string(), false));
        lines.push((self.keymap.app.prev_profile.clone(), "Previous profile".to_string(), false));
        OverlayFrame {
            title: "KEYBINDS".to_string(),
            lines,
            scroll: 0,
        }
    }

    pub(crate) fn invalidate_keybinds_overlay_cache(&mut self) {
        self.keybinds_overlay_cache = None;
    }

    pub(crate) fn keybinds_overlay_cached(&mut self) -> OverlayFrame {
        if let Some(cached) = &self.keybinds_overlay_cache {
            return cached.clone();
        }
        let overlay = self.build_keybinds_overlay();
        self.keybinds_overlay_cache = Some(overlay.clone());
        overlay
    }

    pub(crate) fn handle_keybinds_overlay_key(&mut self, key: crossterm::event::KeyEvent) {
        let Some(overlay) = self.keybinds_overlay.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc | KeyCode::Enter => self.keybinds_overlay = None,
            KeyCode::Up | KeyCode::Char('k') => overlay.scroll = overlay.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => overlay.scroll = overlay.scroll.saturating_add(1),
            _ => {}
        }
    }

    pub(crate) fn handle_input_mode(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_target = None;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => {
                self.commit_input_mode();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    pub(crate) fn start_new_file(&mut self) {
        self.input_mode = InputMode::NewFile;
        self.input_buffer.clear();
        self.rename_target = None;
    }

    pub(crate) fn start_new_dir(&mut self) {
        self.input_mode = InputMode::NewDir;
        self.input_buffer.clear();
        self.rename_target = None;
    }

    pub(crate) fn start_rename(&mut self) {
        let Some(path) = self.selected_entry_path() else {
            return;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.input_mode = InputMode::Rename;
        self.input_buffer = name;
        self.rename_target = Some(path);
    }

    fn commit_input_mode(&mut self) {
        let name = self.input_buffer.trim();
        if name.is_empty() {
            self.status_message = "name cannot be empty".to_string();
            self.input_mode = InputMode::None;
            self.input_buffer.clear();
            return;
        }
        let target = self.current_dir.join(name);
        let result = match self.input_mode {
            InputMode::NewFile => fs::write(&target, ""),
            InputMode::NewDir => fs::create_dir_all(&target),
            InputMode::Rename => {
                if let Some(source) = self.rename_target.clone() {
                    let dest = source
                        .parent()
                        .map(|p| p.join(name))
                        .unwrap_or_else(|| self.current_dir.join(name));
                    fs::rename(source, dest)
                } else {
                    Err(io::Error::other("missing rename source"))
                }
            }
            InputMode::None => Ok(()),
        };
        self.status_message = match result {
            Ok(_) => format!("created {}", target.display()),
            Err(e) => {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    self.dialog = Some(DialogState::Message {
                        title: "PERMISSION ERROR".to_string(),
                        text: format!("Permission denied:\n{}", target.display()),
                    });
                }
                format!("create failed: {e}")
            }
        };
        self.input_mode = InputMode::None;
        self.input_buffer.clear();
        self.rename_target = None;
        self.refresh_active_column();
    }

    pub(crate) fn request_delete_to_trash(&mut self) {
        let paths = self.operation_paths();
        if paths.is_empty() {
            self.status_message = "nothing selected".to_string();
            return;
        }
        self.dialog = Some(DialogState::ConfirmDelete {
            paths,
            yes_selected: true,
        });
    }

    pub(crate) fn handle_dialog_key(&mut self, key: crossterm::event::KeyEvent) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        match dialog {
            DialogState::ConfirmDelete { yes_selected, .. } => match key.code {
                KeyCode::Left | KeyCode::Char('h') => *yes_selected = true,
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => *yes_selected = false,
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    *yes_selected = true;
                    self.confirm_dialog();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.dialog = None;
                    self.status_message = "delete canceled".to_string();
                }
                KeyCode::Enter => self.confirm_dialog(),
                _ => {}
            },
            DialogState::ConfirmBookmarkOverwrite { yes_selected, .. } => match key.code {
                KeyCode::Left | KeyCode::Char('h') => *yes_selected = true,
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => *yes_selected = false,
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    *yes_selected = true;
                    self.confirm_dialog();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.dialog = None;
                    self.status_message = "bookmark overwrite canceled".to_string();
                }
                KeyCode::Enter => self.confirm_dialog(),
                _ => {}
            },
            DialogState::Message { .. } => match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.dialog = None;
                }
                _ => {}
            },
        }
    }

    fn confirm_dialog(&mut self) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        match dialog {
            DialogState::ConfirmDelete {
                paths,
                yes_selected,
            } => {
                if !yes_selected {
                    self.status_message = "delete canceled".to_string();
                    return;
                }
                let mut ok_count = 0usize;
                let mut fail_count = 0usize;
                for path in &paths {
                    match trash::delete(path) {
                        Ok(_) => ok_count += 1,
                        Err(_) => fail_count += 1,
                    }
                }
                self.clear_all_selection();
                self.status_message = if fail_count == 0 {
                    format!("trashed {} item(s)", ok_count)
                } else {
                    format!("trash: {} ok, {} failed", ok_count, fail_count)
                };
                self.refresh_active_column();
            }
            DialogState::ConfirmBookmarkOverwrite {
                slot,
                new_path,
                yes_selected,
                ..
            } => {
                if !yes_selected {
                    self.status_message = "bookmark overwrite canceled".to_string();
                    return;
                }
                let slot_num = slot.to_digit(10).map(|n| n as u8);
                let label = path_last_segment(Path::new(&new_path));
                if let Some(existing) = self
                    .config
                    .sidebar
                    .bookmarks
                    .iter_mut()
                    .find(|b| bookmark_matches_slot(b, slot))
                {
                    existing.slot = slot_num;
                    existing.name = label;
                    existing.path = new_path;
                    self.status_message = format!("updated bookmark {slot}");
                } else {
                    self.status_message = format!("bookmark {slot} not found");
                }
            }
            DialogState::Message { .. } => {}
        }
    }

    fn refresh_active_column(&mut self) {
        if let Some(col) = self.columns.last_mut() {
            let read = read_dir_entries(
                &col.path,
                &self.effective,
                self.sudo_mode,
                if self.sudo_password_input.is_empty() {
                    None
                } else {
                    Some(self.sudo_password_input.as_str())
                },
            );
            col.entries = read.entries;
            col.permission_denied = read.permission_denied;
            col.sudo_password_required = read.sudo_password_required;
            if col.selected >= col.entries.len() && !col.entries.is_empty() {
                col.selected = col.entries.len() - 1;
            }
        }
    }

    pub(crate) fn copy_selected(&mut self) {
        let paths = self.operation_paths();
        if paths.is_empty() {
            self.status_message = "nothing selected".to_string();
            return;
        }
        self.clipboard = Some(ClipboardItem {
            paths: paths.clone(),
            cut: false,
        });
        self.status_message = format!("copied {} item(s)", paths.len());
    }

    pub(crate) fn cut_selected(&mut self) {
        let paths = self.operation_paths();
        if paths.is_empty() {
            self.status_message = "nothing selected".to_string();
            return;
        }
        self.clipboard = Some(ClipboardItem {
            paths: paths.clone(),
            cut: true,
        });
        self.status_message = format!("cut {} item(s)", paths.len());
    }

    pub(crate) fn paste_clipboard(&mut self) {
        let Some(item) = self.clipboard.clone() else {
            self.status_message = "clipboard empty".to_string();
            return;
        };
        let mut ok_count = 0usize;
        let mut fail_count = 0usize;
        for source in &item.paths {
            let Some(name) = source.file_name() else {
                fail_count += 1;
                continue;
            };
            let mut destination = self.current_dir.join(name);
            if destination == *source {
                destination = self
                    .current_dir
                    .join(format!("{}_copy", name.to_string_lossy()));
            }

            let result = if item.cut {
                move_path(source, &destination)
            } else {
                copy_path(source, &destination)
            };
            if result.is_ok() {
                ok_count += 1;
            } else {
                fail_count += 1;
            }
        }
        if item.cut && fail_count == 0 {
            self.clipboard = None;
            self.clear_all_selection();
        }
        self.status_message = if fail_count == 0 {
            if item.cut {
                format!("moved {} item(s)", ok_count)
            } else {
                format!("copied {} item(s)", ok_count)
            }
        } else {
            format!("paste: {} ok, {} failed", ok_count, fail_count)
        };
        self.refresh_active_column();
    }

    fn delete_selected_to_trash(&mut self) {
        let paths = self.operation_paths();
        if paths.is_empty() {
            self.status_message = "nothing selected".to_string();
            return;
        };
        let mut ok_count = 0usize;
        let mut fail_count = 0usize;
        for path in &paths {
            match trash::delete(path) {
                Ok(_) => ok_count += 1,
                Err(_) => fail_count += 1,
            }
        }
        self.clear_all_selection();
        self.status_message = if fail_count == 0 {
            format!("trashed {} item(s)", ok_count)
        } else {
            format!("trash: {} ok, {} failed", ok_count, fail_count)
        };
        self.refresh_active_column();
    }
}
