use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::Theme;
use crate::app::parse_color;

pub fn render_empty_dir_mascot(frame: &mut Frame, area: Rect, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let body = parse_color(&theme.vars.error_color);
    let text = body;
    let shadow = body;
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(vec![Span::styled("  ████████          ", Style::default().fg(body))]),
        Line::from(vec![Span::styled(" █████████████████", Style::default().fg(body))]),
        Line::from(vec![
            Span::styled(" ██", Style::default().fg(body)),
            Span::styled("//ERROR", Style::default().fg(text)),
            Span::styled("████████", Style::default().fg(body)),
            Span::styled("▒", Style::default().fg(shadow)),
        ]),
        Line::from(vec![
            Span::styled(" ██", Style::default().fg(body)),
            Span::styled("404", Style::default().fg(text)),
            Span::styled("████████████", Style::default().fg(body)),
            Span::styled("▒", Style::default().fg(shadow)),
        ]),
        Line::from(vec![
            Span::styled(" █████████████████", Style::default().fg(body)),
            Span::styled("▒", Style::default().fg(shadow)),
        ]),
        Line::from(vec![
            Span::styled(" █████████████████", Style::default().fg(body)),
            Span::styled("▒", Style::default().fg(shadow)),
        ]),
        Line::from(vec![Span::styled("  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒", Style::default().fg(shadow))]),
    ];
    let top_pad = area.height.saturating_sub(lines.len() as u16) as usize / 2;
    for _ in 0..top_pad {
        lines.insert(0, Line::from(""));
    }
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}
