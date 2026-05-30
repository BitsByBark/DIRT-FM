use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding},
};

use crate::app::{App, SearchMode, bookmark_matches_slot, parse_color};

impl App {
    pub(crate) fn draw_sidebar(&self, frame: &mut Frame, area: Rect) {
        let panel_fg = parse_color(&self.effective.theme_colors.vars.defult_panel_label);
        let bookmark_fg = if self.awaiting_bookmark_slot {
            self.ui_mode_color()
        } else {
            panel_fg
        };
        let sidebar_border = if self.sudo_mode
            || self.awaiting_bookmark_slot
            || self.selection_mode
            || self.search_mode != SearchMode::None
        {
            self.ui_mode_color()
        } else {
            parse_color(&self.effective.theme_colors.vars.defult_panel_border)
        };
        let mut rows = Vec::new();

        rows.push(ListItem::new(Line::from("Bookmarks").style(
            Style::default().fg(bookmark_fg).add_modifier(Modifier::BOLD),
        )));
        let mut bookmark_slots: Vec<(char, bool, String)> = Vec::new();
        for slot in 1..=9 {
            let slot_ch = char::from_digit(slot, 10).unwrap_or('1');
            let exists = self
                .config
                .sidebar
                .bookmarks
                .iter()
                .any(|b| bookmark_matches_slot(b, slot_ch));
            if slot > 3 && !exists {
                continue;
            }
            let title = self
                .config
                .sidebar
                .bookmarks
                .iter()
                .find(|b| bookmark_matches_slot(b, slot_ch))
                .map(|b| b.name.clone())
                .unwrap_or_default();
            bookmark_slots.push((slot_ch, exists, title));
        }
        let inner_h = area.height.saturating_sub(2) as usize;
        let reserve_for_drives = 4usize;
        let max_bookmark_rows = inner_h.saturating_sub(reserve_for_drives);
        for (slot_ch, exists, title) in bookmark_slots.into_iter().take(max_bookmark_rows) {
            let line = if exists {
                let filled_color = if self.awaiting_bookmark_slot {
                    self.ui_mode_color()
                } else {
                    parse_color(&self.effective.theme_colors.vars.primary_fg)
                };
                Line::from(vec![Span::styled(
                    format!("  {title}"),
                    Style::default().fg(filled_color),
                )])
            } else if self.awaiting_bookmark_slot {
                Line::from(vec![Span::styled(
                    format!("  {slot_ch}"),
                    Style::default().fg(self.ui_mode_color()),
                )])
            } else {
                Line::from(vec![Span::styled(
                    format!("  ctrl+b+{slot_ch}"),
                    Style::default().fg(parse_color(&self.effective.theme_colors.vars.secondary_fg)),
                )])
            };
            rows.push(ListItem::new(line));
        }

        rows.push(ListItem::new(""));
        rows.push(ListItem::new(
            Line::from("Drives").style(Style::default().fg(panel_fg).add_modifier(Modifier::BOLD)),
        ));
        for drive in self.drives.iter().take(self.config.sidebar.drives_limit.max(1)) {
            rows.push(ListItem::new(
                Line::from(format!("  {}", drive)).style(Style::default().fg(panel_fg)),
            ));
        }

        rows.push(ListItem::new(""));
        rows.push(ListItem::new(
            Line::from("Recent Dirs").style(Style::default().fg(panel_fg).add_modifier(Modifier::BOLD)),
        ));
        for recent in self
            .recents
            .iter()
            .take(self.config.sidebar.recent_dirs_limit.max(1))
        {
            rows.push(ListItem::new(
                Line::from(format!("  {}", recent.display())).style(Style::default().fg(panel_fg)),
            ));
        }

        let list = List::new(rows).block(
            Block::default()
                .title(Line::from("Sidebar").style(Style::default().fg(bookmark_fg)))
                .borders(Borders::ALL)
                .padding(Padding::horizontal(1))
                .style(Style::default().fg(sidebar_border)),
        );
        frame.render_widget(list, area);
    }
}
