use std::{collections::HashSet, path::PathBuf};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, bookmark_slot_from_key};

impl App {
    fn toggle_focused_selection(&mut self) {
        let Some(path) = self.selected_entry_path() else {
            return;
        };
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        if !self.mode_selected_paths.insert(path.clone()) {
            self.mode_selected_paths.remove(&path);
        }
    }

    pub(crate) fn select_prev_range_simple(&mut self) {
        self.select_range_step_simple(-1);
    }

    pub(crate) fn select_next_range_simple(&mut self) {
        self.select_range_step_simple(1);
    }

    fn select_range_step_simple(&mut self, delta: isize) {
        let Some(current_path) = self.columns.last().map(|c| c.path.clone()) else {
            return;
        };
        self.ensure_simple_column(current_path);
        let Some(current) = self.columns.last_mut() else {
            return;
        };
        if current.entries.is_empty() {
            return;
        }
        let previous = current.selected;
        if delta < 0 {
            if current.selected == 0 {
                return;
            }
            current.selected -= 1;
        } else if current.selected + 1 < current.entries.len() {
            current.selected += 1;
        } else {
            return;
        }
        let anchor = self.simple_range_anchor.unwrap_or(previous);
        self.simple_range_anchor = Some(anchor);
        let lo = anchor.min(current.selected);
        let hi = anchor.max(current.selected);
        let new_range: HashSet<PathBuf> = (lo..=hi)
            .map(|idx| current.entries[idx].path.clone())
            .collect();
        for p in new_range.symmetric_difference(&self.simple_last_range) {
            if self.simple_selected_paths.contains(p) {
                self.simple_selected_paths.remove(p);
            } else {
                self.simple_selected_paths.insert(p.clone());
            }
        }
        self.simple_last_range = new_range;
    }

    pub(crate) fn operation_paths(&self) -> Vec<PathBuf> {
        let set = self.selection_set();
        if !set.is_empty() {
            return set.iter().cloned().collect();
        }
        self.selected_entry_path().into_iter().collect()
    }

    pub(crate) fn selection_set(&self) -> &HashSet<PathBuf> {
        if self.selection_mode {
            &self.mode_selected_paths
        } else {
            &self.simple_selected_paths
        }
    }

    pub(crate) fn clear_simple_selection(&mut self) {
        self.simple_selected_paths.clear();
        self.simple_range_anchor = None;
        self.simple_last_range.clear();
        self.simple_column_path = None;
    }

    pub(crate) fn clear_all_selection(&mut self) {
        self.clear_simple_selection();
        self.mode_selected_paths.clear();
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.locked_column_path = None;
        self.selection_mode = false;
    }

    fn ensure_simple_column(&mut self, path: PathBuf) {
        if self.simple_column_path.as_ref() != Some(&path) {
            self.simple_selected_paths.clear();
            self.simple_range_anchor = None;
            self.simple_last_range.clear();
            self.simple_column_path = Some(path);
        }
    }

    pub(crate) fn enter_selection_mode(&mut self) {
        if self.selection_mode {
            return;
        }
        self.selection_mode = true;
        self.mode_selected_paths.clear();
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.locked_column_path = self.columns.last().map(|c| c.path.clone());
        self.clear_simple_selection();
    }

    fn exit_selection_mode(&mut self) {
        self.selection_mode = false;
        self.mode_selected_paths.clear();
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.locked_column_path = None;
    }

    pub(crate) fn handle_selection_mode_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.awaiting_bookmark_slot
            && let Some(slot) = bookmark_slot_from_key(key)
        {
            self.set_bookmark_slot(slot);
            return;
        }
        match key.code {
            KeyCode::Esc => self.exit_selection_mode(),
            KeyCode::Char(' ') if key.modifiers.is_empty() => self.toggle_focused_selection(),
            KeyCode::Char('/') if key.modifiers.is_empty() => self.start_local_search(),
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.start_global_search()
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => self.select_prev_range_mode(),
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.select_next_range_mode()
            }
            KeyCode::Up | KeyCode::Char('k') => self.select_prev_mode(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_mode(),
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {}
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.copy_selected(),
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => self.cut_selected(),
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.paste_clipboard()
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_delete_to_trash()
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.awaiting_bookmark_slot = true;
                self.status_message = "bookmark set mode: press 1-9".to_string();
            }
            KeyCode::Char(c)
                if key.modifiers.contains(KeyModifiers::CONTROL) && ('1'..='9').contains(&c) =>
            {
                self.open_bookmark_slot(c);
            }
            KeyCode::Char(c) if key.modifiers.is_empty() && ('1'..='9').contains(&c) => {
                self.open_bookmark_slot(c);
            }
            KeyCode::Char('n') if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                self.start_new_dir()
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.start_new_file()
            }
            KeyCode::F(2) => self.start_rename(),
            _ => {}
        }
    }

    fn select_prev_mode(&mut self) {
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.select_prev();
    }

    fn select_next_mode(&mut self) {
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.select_next();
    }

    fn select_prev_range_mode(&mut self) {
        self.select_range_step_mode(-1);
    }

    fn select_next_range_mode(&mut self) {
        self.select_range_step_mode(1);
    }

    fn select_range_step_mode(&mut self, delta: isize) {
        let Some(current) = self.columns.last_mut() else {
            return;
        };
        if current.entries.is_empty() {
            return;
        }
        let previous = current.selected;
        if delta < 0 {
            if current.selected == 0 {
                return;
            }
            current.selected -= 1;
        } else if current.selected + 1 < current.entries.len() {
            current.selected += 1;
        } else {
            return;
        }
        let anchor = self.mode_range_anchor.unwrap_or(previous);
        self.mode_range_anchor = Some(anchor);
        let lo = anchor.min(current.selected);
        let hi = anchor.max(current.selected);
        let new_range: HashSet<PathBuf> = (lo..=hi)
            .map(|idx| current.entries[idx].path.clone())
            .collect();
        for p in new_range.symmetric_difference(&self.mode_last_range) {
            if self.mode_selected_paths.contains(p) {
                self.mode_selected_paths.remove(p);
            } else {
                self.mode_selected_paths.insert(p.clone());
            }
        }
        self.mode_last_range = new_range;
    }
}
