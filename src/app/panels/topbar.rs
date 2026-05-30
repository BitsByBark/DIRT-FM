use ratatui::{
    Frame,
    layout::{Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
};

use crate::app::{App, InputMode, SearchMode, parse_color};

impl App {
    pub(crate) fn draw_top_bar(&self, frame: &mut Frame, area: Rect) {
        let mode_active =
            self.sudo_mode || self.awaiting_bookmark_slot || self.selection_mode || self.search_mode != SearchMode::None;
        let mode_color = self.ui_mode_color();
        let top_bar_fg = if mode_active {
            mode_color
        } else {
            parse_color(&self.effective.theme_colors.vars.primary_fg)
        };
        let top_bar_border = top_bar_fg;
        let constraints = self.main_panel_constraints();
        let segments = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        let mut seg_idx = 0;
        if self.runtime_plan.panels.sidebar {
            let (lhs, rhs) = if self.search_mode != SearchMode::None {
                ("DIRT::".to_string(), "search".to_string())
            } else if self.awaiting_bookmark_slot {
                ("DIRT::".to_string(), "bookmark".to_string())
            } else if self.selection_mode {
                ("DIRT::".to_string(), "selection".to_string())
            } else {
                let profile_name = self
                    .config
                    .profiles
                    .get(self.active_profile)
                    .map(|p| p.name.as_str())
                    .unwrap_or("default");
                ("DIRT // ".to_string(), profile_name.to_string())
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(lhs, Style::default().fg(top_bar_fg).add_modifier(Modifier::BOLD)),
                    Span::styled(rhs, Style::default().fg(top_bar_fg)),
                ]))
                .style(Style::default().fg(top_bar_fg))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .padding(Padding::horizontal(1))
                        .border_style(Style::default().fg(top_bar_border)),
                ),
                segments[seg_idx],
            );
            seg_idx += 1;
        }

        if self.runtime_plan.panels.columns {
            let search_label = if self.sudo_password_prompt {
                format!("[sudo] password: {}", "*".repeat(self.sudo_password_input.len()))
            } else {
                match self.search_mode {
                    SearchMode::None => "/ to search".to_string(),
                    SearchMode::Local => format!("/ {}", self.search_query),
                    SearchMode::Global => format!("Ctrl+F {}", self.search_query),
                }
            };
            let label = if self.input_mode == InputMode::None {
                search_label
            } else {
                match self.input_mode {
                    InputMode::NewFile => format!("new file: {}", self.input_buffer),
                    InputMode::NewDir => format!("new dir: {}", self.input_buffer),
                    InputMode::Rename => format!("rename: {}", self.input_buffer),
                    InputMode::None => search_label,
                }
            };
            frame.render_widget(
                Paragraph::new(label)
                    .style(Style::default().fg(if self.search_mode != SearchMode::None {
                        mode_color
                    } else if self.selection_mode {
                        parse_color(&self.effective.theme_colors.vars.secondary_fg)
                    } else {
                        top_bar_fg
                    }))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .padding(Padding::horizontal(1))
                            .border_style(Style::default().fg(top_bar_border)),
                    ),
                segments[seg_idx],
            );
            seg_idx += 1;
        }

        if self.runtime_plan.panels.preview {
            let preview_label = if self.sudo_mode {
                format!("SUDO @ {}", self.device_name)
            } else {
                format!("{} @ {}", self.user_name, self.device_name)
            };
            frame.render_widget(
                Paragraph::new(preview_label)
                    .style(Style::default().fg(top_bar_fg))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .padding(Padding::horizontal(1))
                            .border_style(Style::default().fg(top_bar_border)),
                    ),
                segments[seg_idx],
            );
        }
    }
}
