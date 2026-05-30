use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::App;

impl App {
    pub(crate) fn toggle_sudo_mode(&mut self) {
        self.sudo_mode = !self.sudo_mode;
        self.sudo_password_prompt = false;
        self.sudo_password_input.clear();
        self.status_message = if self.sudo_mode {
            "sudo mode enabled".to_string()
        } else {
            "sudo mode disabled".to_string()
        };
        self.refresh_all_columns();
    }

    pub(crate) fn handle_sudo_password_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.sudo_password_prompt = false;
                self.sudo_password_input.clear();
                self.status_message = "sudo mode disabled".to_string();
                self.sudo_mode = false;
                self.refresh_all_columns();
            }
            KeyCode::Backspace => {
                self.sudo_password_input.pop();
            }
            KeyCode::Enter => {
                self.sudo_password_prompt = false;
                self.refresh_all_columns();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.sudo_password_input.push(c);
            }
            _ => {}
        }
    }

    fn refresh_all_columns(&mut self) {
        let mut any_password_required = false;
        for col in &mut self.columns {
            let read = super::super::read_dir_entries(
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
            any_password_required |= read.sudo_password_required;
        }
        self.sudo_password_prompt = self.sudo_mode && any_password_required;
    }
}
