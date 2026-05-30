use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
};

use ratatui::style::Color;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayButton {
    Yes,
    No,
    Ok,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayDialogKind {
    Confirm,
    Input { password: bool, placeholder: Option<String> },
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayDialog {
    pub title: String,
    pub message: String,
    pub kind: OverlayDialogKind,
    pub input: String,
    pub selected_is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayFrame {
    pub title: String,
    pub lines: Vec<(String, String, bool)>,
    pub scroll: u16,
}

pub fn draw_dim(frame: &mut Frame, theme: &Theme) {
    frame.render_widget(
        Block::default().style(Style::default().bg(parse_color(&theme.overlay.dim_bg))),
        frame.area(),
    );
}

pub fn draw_dialog(frame: &mut Frame, theme: &Theme, dialog: &OverlayDialog) {
    let area = fit_centered_dialog(frame.area(), &dialog.message);
    frame.render_widget(Clear, area);

    let border = Style::default().fg(parse_color(&theme.overlay.border)).bg(parse_color(&theme.overlay.background));
    let body = Style::default().fg(parse_color(&theme.overlay.text_fg)).bg(parse_color(&theme.overlay.background));
    let title = Line::from(Span::styled(
        format!(" {} ", dialog.title),
        Style::default()
            .fg(parse_color(&theme.overlay.title_fg))
            .bg(parse_color(&theme.overlay.background))
            .add_modifier(Modifier::BOLD),
    ));

    let lines = dialog.message.lines().count() as u16;
    let button_row = match dialog.kind {
        OverlayDialogKind::Message => message_buttons(theme),
        OverlayDialogKind::Confirm => yes_no_buttons(theme, dialog.selected_is_primary),
        OverlayDialogKind::Input { .. } => ok_cancel_buttons(theme, dialog.selected_is_primary),
    };

    let mut content = vec![Line::from(dialog.message.clone()), Line::from("")];
    if let OverlayDialogKind::Input { password, placeholder } = &dialog.kind {
        let shown = if dialog.input.is_empty() {
            placeholder.clone().unwrap_or_default()
        } else if *password {
            "*".repeat(dialog.input.chars().count())
        } else {
            dialog.input.clone()
        };
        let fg = if dialog.input.is_empty() {
            parse_color(&theme.overlay.secondary_fg)
        } else {
            parse_color(&theme.overlay.input_fg)
        };
        content.push(Line::from(Span::styled(
            format!(" {} ", shown),
            Style::default()
                .fg(fg)
                .bg(parse_color(&theme.overlay.input_bg)),
        )));
        content.push(Line::from(Span::styled(
            " ".repeat(shown.chars().count() + 2),
            Style::default().bg(parse_color(&theme.overlay.input_bg)),
        )));
        content.push(Line::from(""));
    }
    content.push(button_row);
    content.push(Line::from(Span::styled(
        "Enter confirm  Esc cancel",
        Style::default().fg(parse_color(&theme.overlay.secondary_fg)),
    )));

    frame.render_widget(
        Paragraph::new(content)
            .alignment(Alignment::Center)
            .style(body)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(border)
                    .padding(Padding::new(6, 6, 2, 2))
                    .style(body),
            ),
        area,
    );

    let _ = lines;
}

pub fn draw_frame(frame: &mut Frame, theme: &Theme, overlay: &OverlayFrame) {
    let area = centered_rect(90, 90, frame.area());
    frame.render_widget(Clear, area);

    let header = Line::from(Span::styled(
        format!(" {} ", overlay.title),
        Style::default().fg(parse_color(&theme.overlay.title_fg)).add_modifier(Modifier::BOLD),
    ));
    let block = Block::default()
        .title(header)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(parse_color(&theme.overlay.border)))
        .style(Style::default().bg(parse_color(&theme.overlay.background)))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut rows = Vec::with_capacity(overlay.lines.len() + 8);
    for (key, action, header_row) in &overlay.lines {
        if *header_row {
            rows.push(Line::from(Span::styled(
                key.clone(),
                Style::default().fg(parse_color(&theme.overlay.title_fg)).add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        rows.push(Line::from(vec![
            Span::styled(
                format!("{: <24}", key),
                Style::default().fg(parse_color(&theme.overlay.border)),
            ),
            Span::styled(action.clone(), Style::default().fg(parse_color(&theme.overlay.text_fg))),
        ]));
    }
    let max_scroll = rows.len().saturating_sub(inner.height as usize) as u16;
    let scroll = overlay.scroll.min(max_scroll);
    frame.render_widget(
        Paragraph::new(rows)
            .scroll((scroll, 0))
            .style(Style::default().bg(parse_color(&theme.overlay.background))),
        inner,
    );
}

fn fit_centered_dialog(area: Rect, text: &str) -> Rect {
    let max_w = (area.width.saturating_mul(60) / 100).max(28);
    let line_w = text.lines().map(|l| l.chars().count() as u16).max().unwrap_or(18);
    let w = (line_w + 16).min(max_w).max(28);
    let h = (text.lines().count() as u16 + 10).min(area.height.saturating_sub(2)).max(10);
    centered_rect_exact(w, h, area)
}

fn yes_no_buttons(theme: &Theme, yes_selected: bool) -> Line<'static> {
    Line::from(vec![
        button_span("▐ Yes ▌", yes_selected, parse_color(&theme.overlay.bool_yes), parse_color(&theme.overlay.secondary_fg)),
        Span::raw("   "),
        button_span("▐ No ▌", !yes_selected, parse_color(&theme.overlay.bool_no), parse_color(&theme.overlay.secondary_fg)),
    ])
}

fn ok_cancel_buttons(theme: &Theme, ok_selected: bool) -> Line<'static> {
    Line::from(vec![
        button_span("▐ OK ▌", ok_selected, parse_color(&theme.overlay.bool_yes), parse_color(&theme.overlay.secondary_fg)),
        Span::raw("   "),
        button_span("▐ Cancel ▌", !ok_selected, parse_color(&theme.overlay.bool_no), parse_color(&theme.overlay.secondary_fg)),
    ])
}

fn message_buttons(theme: &Theme) -> Line<'static> {
    Line::from(button_span(
        "▐ OK ▌",
        true,
        parse_color(&theme.overlay.bool_yes),
        parse_color(&theme.overlay.secondary_fg),
    ))
}

fn button_span(text: &str, selected: bool, selected_fg: ratatui::style::Color, unselected_fg: ratatui::style::Color) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default().fg(if selected { selected_fg } else { unselected_fg }),
    )
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn centered_rect_exact(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect { x, y, width: width.min(r.width), height: height.min(r.height) }
}

fn parse_color(name: &str) -> Color {
    let s = name.trim().trim_start_matches('#');
    if s.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        )
    {
        return Color::Rgb(r, g, b);
    }
    Color::White
}
