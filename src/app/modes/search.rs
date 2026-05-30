use std::path::Path;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::{
    app::{App, SearchMode, build_columns_from_path, global_search_paths, init_config_file, init_keymap_file, init_layout_file, init_theme_file, load_keymap_config},
    ui::searchbar::{SearchbarCommand, parse_command},
};

impl App {
    pub(crate) fn start_local_search(&mut self) {
        self.search_mode = SearchMode::Local;
        self.search_query.clear();
    }

    pub(crate) fn start_global_search(&mut self) {
        self.search_mode = SearchMode::Global;
        self.search_query.clear();
        self.run_global_search();
    }

    pub(crate) fn handle_search_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_mode = SearchMode::None;
                self.search_query.clear();
                self.global_results.clear();
                self.global_selected = 0;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.on_search_query_changed();
            }
            KeyCode::Enter => {
                if self.search_mode == SearchMode::Global {
                    self.open_global_selected();
                } else {
                    if let Some(cmd) = parse_command(self.search_query.trim()) {
                        match cmd {
                            SearchbarCommand::ToggleSudo => self.toggle_sudo_mode(),
                            SearchbarCommand::ConfigInit => {
                                self.status_message = match init_config_file() {
                                    Ok(msg) => msg,
                                    Err(e) => format!("config init failed: {e}"),
                                };
                            }
                            SearchbarCommand::ConfigLayoutInit => {
                                self.status_message = match init_layout_file() {
                                    Ok(msg) => msg,
                                    Err(e) => format!("config layout init failed: {e}"),
                                };
                            }
                            SearchbarCommand::ConfigThemeInit => {
                                self.status_message = match init_theme_file() {
                                    Ok(msg) => msg,
                                    Err(e) => format!("config theme init failed: {e}"),
                                };
                            }
                            SearchbarCommand::KeymapInit => {
                                self.status_message = match init_keymap_file() {
                                    Ok(msg) => msg,
                                    Err(e) => format!("keymap init failed: {e}"),
                                };
                                self.keymap = load_keymap_config();
                                self.invalidate_keybinds_overlay_cache();
                            }
                            SearchbarCommand::Keybinds => {
                                self.keybinds_overlay = Some(self.keybinds_overlay_cached());
                            }
                        }
                    }
                    self.search_mode = SearchMode::None;
                    self.search_query.clear();
                }
            }
            KeyCode::Up => self.select_prev(),
            KeyCode::Down => self.select_next(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_query.push(c);
                self.on_search_query_changed();
            }
            _ => {}
        }
    }

    fn on_search_query_changed(&mut self) {
        match self.search_mode {
            SearchMode::Local => {
                if matches!(
                    self.search_query.trim(),
                    "/sudo" | "sudo" | "/keybinds" | "keybinds"
                ) {
                    return;
                }
                let filtered = self
                    .columns
                    .last()
                    .map(|c| self.local_filtered_indices(c))
                    .unwrap_or_default();
                if let Some(first) = filtered.first().copied()
                    && let Some(current) = self.columns.last_mut()
                {
                    current.selected = first;
                }
            }
            SearchMode::Global => self.run_global_search(),
            SearchMode::None => {}
        }
    }

    fn run_global_search(&mut self) {
        self.global_results = global_search_paths(
            &self.current_dir,
            &self.search_query,
            self.effective.search.max_depth,
            &self.effective.search.ignored_dirs,
        );
        self.global_selected = 0;
    }

    pub(crate) fn open_global_selected(&mut self) {
        let Some(path) = self.global_results.get(self.global_selected).cloned() else {
            return;
        };
        let target_dir = if path.is_dir() {
            path
        } else {
            path.parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.current_dir.clone())
        };
        self.current_dir = target_dir.clone();
        self.columns = build_columns_from_path(
            &target_dir,
            &self.effective,
            self.sudo_mode,
            if self.sudo_password_input.is_empty() {
                None
            } else {
                Some(self.sudo_password_input.as_str())
            },
        );
        self.sudo_password_prompt = self.sudo_mode && self.columns.iter().any(|c| c.sudo_password_required);
        self.track_recent_dir();
        self.search_mode = SearchMode::None;
        self.search_query.clear();
        self.global_results.clear();
        self.global_selected = 0;
    }
}
