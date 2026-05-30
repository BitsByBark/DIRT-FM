use std::sync::atomic::Ordering as AtomicOrdering;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::{App, PERF_FRAMES, PERF_TOTAL_US, parse_color},
    ui::preview,
};

impl App {
    pub(crate) fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let profile_name = self
            .config
            .profiles
            .get(self.active_profile)
            .map(|p| p.name.as_str())
            .unwrap_or("default");
        let panel_fg = parse_color(&self.effective.theme_colors.vars.defult_panel_label);
        let sudo_color = parse_color(&self.effective.theme_colors.vars.sudo_mode);
        let mode_color = self.ui_mode_color();
        let sep = Span::styled(" · ", Style::default().fg(panel_fg));
        let mut spans = vec![
            Span::styled(self.current_dir.display().to_string(), Style::default().fg(panel_fg)),
            sep.clone(),
            Span::styled(format!("files: {}", self.current_file_count()), Style::default().fg(panel_fg)),
            sep.clone(),
            Span::styled(format!("selected: {}", self.selected_name()), Style::default().fg(panel_fg)),
            sep.clone(),
            Span::styled(
                if self.selection_mode {
                    "SELECTION MODE".to_string()
                } else {
                    "select_mode: off".to_string()
                },
                Style::default().fg(panel_fg),
            ),
            sep.clone(),
            Span::styled(format!("git: {}", self.git_status_text()), Style::default().fg(panel_fg)),
            sep.clone(),
            Span::styled(format!("profile: {}", profile_name), Style::default().fg(panel_fg)),
            sep.clone(),
            Span::styled(
                if self.status_message.is_empty() {
                    "ready".to_string()
                } else {
                    self.status_message.clone()
                },
                Style::default().fg(panel_fg),
            ),
        ];
        let frames = PERF_FRAMES.load(AtomicOrdering::Relaxed).max(1);
        let avg_ms = (PERF_TOTAL_US.load(AtomicOrdering::Relaxed) / frames) as f64 / 1000.0;
        spans.push(sep.clone());
        spans.push(Span::styled(format!("frame {:.2}ms", avg_ms), Style::default().fg(panel_fg)));
        if self.runtime_plan.features.preview_images {
            let ts = preview::thumb_stats_snapshot();
            spans.push(sep.clone());
            spans.push(Span::styled(
                format!(
                    "thumb q:{} run:{} ok:{} err:{} drop:{}",
                    ts.queued, ts.started, ts.completed, ts.failed, ts.dropped
                ),
                Style::default().fg(panel_fg),
            ));
        }
        if self.sudo_mode {
            spans.push(sep.clone());
            spans.push(Span::styled("SUDO", Style::default().fg(sudo_color).add_modifier(Modifier::BOLD)));
        }
        frame.render_widget(
            Paragraph::new(Line::from(std::mem::take(&mut spans))).block(
                Block::default().borders(Borders::TOP).border_style(Style::default().fg(
                    if self.sudo_mode || self.awaiting_bookmark_slot || self.selection_mode || self.search_mode != crate::app::SearchMode::None {
                        mode_color
                    } else {
                        parse_color(&self.effective.theme_colors.vars.defult_panel_border)
                    },
                )),
            ),
            area,
        );
    }
}
