use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Padding, Paragraph},
};

use crate::{
    app::{App, PreviewData, SearchMode, format_size, image_metadata_lines, image_preview_panel_height, parse_color, path_last_segment},
    mascot::render_empty_dir_mascot,
    ui::preview::{self, ImageProtocol},
};

impl App {
    pub(crate) fn draw_details_panel(&self, frame: &mut Frame, area: Rect) {
        let mode_active = self.sudo_mode
            || self.awaiting_bookmark_slot
            || self.selection_mode
            || self.search_mode != SearchMode::None;
        let details_color = if mode_active {
            self.ui_mode_color()
        } else {
            parse_color(&self.effective.theme_colors.vars.defult_panel_border)
        };
        let details_title_color = if mode_active {
            self.ui_mode_color()
        } else {
            parse_color(&self.effective.theme_colors.vars.defult_panel_label)
        };
        if let Some(path) = self.selected_dir_path() {
            let read = super::super::read_dir_entries(
                path,
                &self.effective,
                self.sudo_mode,
                if self.sudo_password_input.is_empty() {
                    None
                } else {
                    Some(self.sudo_password_input.as_str())
                },
            );
            if read.permission_denied {
                let block = Block::default()
                    .title(Line::from("Details").style(Style::default().fg(details_title_color)))
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .style(Style::default().fg(details_color));
                let inner = block.inner(area);
                frame.render_widget(block, area);
                super::super::render_no_perms_mascot(
                    frame,
                    inner,
                    &self.effective.theme_colors,
                    path,
                    self.sudo_mode,
                );
                return;
            }
            if read.entries.is_empty() {
                if let Some(protocol) = self.image_protocol {
                    let _ = preview::clear_last_image(protocol);
                }
                let block = Block::default()
                    .title(Line::from("Details").style(Style::default().fg(details_title_color)))
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .style(Style::default().fg(details_color));
                let inner = block.inner(area);
                frame.render_widget(block, area);
                render_empty_dir_mascot(frame, inner, &self.effective.theme_colors);
                return;
            }
        }

        if let Some(path) = self.selected_entry_path()
            && preview::is_supported_image(&path)
            && self.runtime_plan.features.preview_images
        {
            let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let max_bytes = self.effective.preview.max_image_size_mb * 1024 * 1024;
            let metadata = image_metadata_lines(&path);
            if file_size > max_bytes {
                if let Some(protocol) = self.image_protocol {
                    let _ = preview::clear_last_image(protocol);
                }
                self.render_details_lines(frame, area, metadata, details_title_color, details_color);
                return;
            }

            let preview_height = image_preview_panel_height(area, &path);
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(preview_height), Constraint::Min(5)])
                .split(area);

            if let Some(protocol) = self.image_protocol {
                self.draw_image_preview(
                    frame,
                    split[0],
                    &path,
                    protocol,
                    details_title_color,
                    details_color,
                );
                self.render_details_lines(frame, split[1], metadata, details_title_color, details_color);
                return;
            }

            let size = format_size(file_size);
            preview::render_unsupported_mascot(
                frame,
                split[0],
                &self.effective.theme_colors,
                &path_last_segment(&path),
                &size,
            );
            if let Some(protocol) = self.image_protocol {
                let _ = preview::clear_last_image(protocol);
            }
            self.render_details_lines(frame, split[1], metadata, details_title_color, details_color);
            return;
        }

        if let Some(protocol) = self.image_protocol {
            let _ = preview::clear_last_image(protocol);
        }

        let details_text = match self.current_preview() {
            PreviewData::Empty => vec![Line::from("No selection")],
            PreviewData::Details(content) => content,
            PreviewData::UnsupportedImageMascot { filename, size } => {
                preview::render_unsupported_mascot(
                    frame,
                    area,
                    &self.effective.theme_colors,
                    &filename,
                    &size,
                );
                return;
            }
        };
        let p = Paragraph::new(details_text)
            .style(Style::default().fg(parse_color(&self.effective.theme_colors.vars.defult_panel_label)))
            .block(
                Block::default()
                    .title(Line::from("Details").style(Style::default().fg(details_title_color)))
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .style(Style::default().fg(details_color)),
            );
        frame.render_widget(p, area);
    }

    fn draw_image_preview(
        &self,
        frame: &mut Frame,
        area: Rect,
        path: &std::path::Path,
        protocol: ImageProtocol,
        title_color: Color,
        border_color: Color,
    ) {
        let block = Block::default()
            .title(Line::from("Preview").style(Style::default().fg(title_color)))
            .borders(Borders::ALL)
            .style(Style::default().fg(border_color));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let _ = preview::render_image(path, inner, protocol);
    }

    fn render_details_lines(
        &self,
        frame: &mut Frame,
        area: Rect,
        lines: Vec<Line<'static>>,
        title_color: Color,
        border_color: Color,
    ) {
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().fg(parse_color(&self.effective.theme_colors.vars.secondary_fg)))
                .block(
                    Block::default()
                        .title(Line::from("Details").style(Style::default().fg(title_color)))
                        .borders(Borders::ALL)
                        .padding(Padding::horizontal(1))
                        .style(Style::default().fg(border_color)),
                ),
            area,
        );
    }
}
