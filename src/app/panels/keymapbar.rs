use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, parse_color};

impl App {
    pub(crate) fn draw_keymap_bar(&self, frame: &mut Frame, area: Rect) {
        let panel_fg = parse_color(&self.effective.theme_colors.vars.defult_panel_label);
        let key = |t: &str| Span::styled(t.to_string(), Style::default().fg(panel_fg));
        let label = |t: &str| Span::styled(t.to_string(), Style::default().fg(panel_fg));
        let sep = Span::styled(" · ".to_string(), Style::default().fg(panel_fg));
        let line = if self.selection_mode {
            Line::from(vec![
                key(&self.keymap.selection.toggle),
                label(" select/deselect"),
                sep.clone(),
                key(&self.keymap.selection.range_up.join("/")),
                label(" range"),
                sep.clone(),
                key(&self.keymap.selection.exit),
                label(" exit"),
                sep.clone(),
                key(&self.keymap.file_ops.copy),
                label(" copy"),
                sep.clone(),
                key(&self.keymap.file_ops.cut),
                label(" cut"),
                sep.clone(),
                key(&self.keymap.file_ops.paste),
                label(" paste"),
                sep.clone(),
                key(&self.keymap.file_ops.trash),
                label(" trash"),
            ])
        } else {
            Line::from(vec![
                key(&self.keymap.navigation.up.join("/")),
                label(" navigate"),
                sep.clone(),
                key(&self.keymap.navigation.down.join("/")),
                label(" navigate"),
                sep.clone(),
                key(&self.keymap.navigation.parent.join("/")),
                label(" navigate"),
                sep.clone(),
                key(&self.keymap.navigation.open.join("/")),
                label(" navigate"),
                sep.clone(),
                key(&self.keymap.selection.mode),
                label(" select mode"),
                sep.clone(),
                key(&self.keymap.app.quit),
                label(" quit"),
            ])
        };
        frame.render_widget(
            Paragraph::new(line).block(
                Block::default().borders(Borders::TOP).border_style(Style::default().fg(parse_color(
                    &self.effective.theme_colors.vars.defult_panel_border,
                ))),
            ),
            area,
        );
    }
}
