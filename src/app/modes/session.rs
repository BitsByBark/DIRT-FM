use crate::app::{
    App, Bookmark, DialogState, RuntimePlan, bookmark_matches_slot, build_columns_from_path,
    discover_drives, expand_tilde, load_recents, path_last_segment,
};
use crate::ui::preview;

impl App {
    pub(crate) fn set_bookmark_slot(&mut self, slot: char) {
        let target_path = self
            .selected_dir_path()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.current_dir.clone());
        if !target_path.is_dir() {
            self.status_message = "bookmark target must be a directory".to_string();
            return;
        }
        let target = target_path.display().to_string();
        let label = path_last_segment(&target_path);
        let slot_num = slot.to_digit(10).map(|n| n as u8);
        if let Some(existing) = self
            .config
            .sidebar
            .bookmarks
            .iter()
            .find(|b| bookmark_matches_slot(b, slot))
        {
            if existing.path != target {
                self.dialog = Some(DialogState::ConfirmBookmarkOverwrite {
                    slot,
                    existing_path: existing.path.clone(),
                    new_path: target,
                    yes_selected: true,
                });
                return;
            }
        } else {
            self.config.sidebar.bookmarks.push(Bookmark {
                slot: slot_num,
                name: label,
                path: target,
            });
            self.status_message = format!("set bookmark {slot}");
            return;
        }
        self.status_message = format!("bookmark {slot} unchanged");
    }

    pub(crate) fn open_bookmark_slot(&mut self, slot: char) {
        let Some(path_raw) = self
            .config
            .sidebar
            .bookmarks
            .iter()
            .find(|b| bookmark_matches_slot(b, slot))
            .map(|b| b.path.clone())
        else {
            self.status_message = format!("bookmark {slot} is empty");
            return;
        };
        let target = expand_tilde(&path_raw);
        if !target.is_dir() {
            self.status_message = format!("bookmark {slot} is invalid");
            return;
        }
        self.current_dir = target.clone();
        self.columns = build_columns_from_path(
            &target,
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
        self.search_mode = crate::app::SearchMode::None;
        self.search_query.clear();
        self.global_results.clear();
        self.global_selected = 0;
        self.clear_simple_selection();
        self.status_message = format!("opened bookmark {slot}");
    }

    pub(crate) fn next_profile(&mut self) {
        if self.config.profiles.is_empty() {
            return;
        }
        self.active_profile = (self.active_profile + 1) % self.config.profiles.len();
        self.effective = self
            .config
            .effective_for_profile(self.config.profiles.get(self.active_profile));
        self.sync_runtime_plan();
        self.reinitialize_columns_preserve_current();
        self.clear_simple_selection();
        self.invalidate_keybinds_overlay_cache();
    }

    pub(crate) fn prev_profile(&mut self) {
        if self.config.profiles.is_empty() {
            return;
        }
        self.active_profile = if self.active_profile == 0 {
            self.config.profiles.len() - 1
        } else {
            self.active_profile - 1
        };
        self.effective = self
            .config
            .effective_for_profile(self.config.profiles.get(self.active_profile));
        self.sync_runtime_plan();
        self.reinitialize_columns_preserve_current();
        self.clear_simple_selection();
        self.invalidate_keybinds_overlay_cache();
    }

    pub(crate) fn sync_runtime_plan(&mut self) {
        self.runtime_plan = RuntimePlan::from_effective(&self.effective);
        if !self.runtime_plan.panels.sidebar {
            self.recents.clear();
            self.drives.clear();
        } else if self.recents.is_empty() && self.drives.is_empty() {
            self.recents = load_recents().unwrap_or_default();
            self.drives = discover_drives();
        }
        self.image_protocol = if self.runtime_plan.features.preview_images {
            preview::detect_image_protocol()
        } else {
            None
        };
    }
}
