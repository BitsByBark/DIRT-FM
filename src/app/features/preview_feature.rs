use std::{fs, path::Path};

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::{
    app::{PreviewData, format_size, format_system_time, image_metadata_lines, parse_color, path_last_segment},
    theme::Theme,
    ui::preview::{self, ImageProtocol, PreviewImageConfig},
};

pub(crate) fn preview_for_path(
    path: &Path,
    colors: &Theme,
    selection_mode: bool,
    search_mode: bool,
    image_cfg: PreviewImageConfig,
    image_protocol: Option<ImageProtocol>,
) -> PreviewData {
    let text_color = if search_mode {
        parse_color(&colors.vars.search_mode)
    } else if selection_mode {
        parse_color(&colors.vars.selection_mode)
    } else {
        parse_color(&colors.preview.value)
    };
    let label_color = if search_mode {
        parse_color(&colors.vars.search_mode)
    } else if selection_mode {
        parse_color(&colors.vars.selection_mode)
    } else {
        parse_color(&colors.preview.label)
    };
    let label = |k: &str| Span::styled(format!("{k}: "), Style::default().fg(label_color));
    let val = |v: String| Span::styled(v, Style::default().fg(text_color));
    let bool_span = |b: bool| {
        if b {
            Span::styled("true", Style::default().fg(parse_color(&colors.vars.bool_yes)))
        } else {
            Span::styled("false", Style::default().fg(parse_color(&colors.vars.bool_no)))
        }
    };
    if preview::is_supported_image(path) {
        let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let max_bytes = image_cfg.max_image_size_mb * 1024 * 1024;
        if file_size > max_bytes {
            return PreviewData::Details(image_metadata_lines(path));
        }
        if image_cfg.enabled && image_protocol.is_none() {
            let size = format_size(file_size);
            return PreviewData::UnsupportedImageMascot {
                filename: path_last_segment(path),
                size,
            };
        }
    }

    let mut lines = Vec::new();
    lines.push(Line::from(vec![label("name"), val(path_last_segment(path))]));
    lines.push(Line::from(vec![label("path"), val(path.display().to_string())]));

    let Ok(meta) = fs::metadata(path) else {
        lines.push(Line::from(vec![label("metadata"), val("unavailable".to_string())]));
        return PreviewData::Details(lines);
    };
    let Ok(link_meta) = fs::symlink_metadata(path) else {
        lines.push(Line::from(vec![
            label("symlink_metadata"),
            val("unavailable".to_string()),
        ]));
        return PreviewData::Details(lines);
    };

    let file_type = meta.file_type();
    let link_type = link_meta.file_type();
    lines.push(Line::from(vec![label("is dir"), bool_span(file_type.is_dir())]));
    lines.push(Line::from(vec![label("is symlink"), bool_span(link_type.is_symlink())]));
    lines.push(Line::from(vec![label("size"), val(format_size(meta.len()))]));
    lines.push(Line::from(vec![
        label("readonly"),
        bool_span(meta.permissions().readonly()),
    ]));
    lines.push(Line::from(vec![
        label("created"),
        val(format_system_time(meta.created().ok())),
    ]));
    lines.push(Line::from(vec![
        label("modified"),
        val(format_system_time(meta.modified().ok())),
    ]));
    lines.push(Line::from(vec![
        label("accessed"),
        val(format_system_time(meta.accessed().ok())),
    ]));

    if file_type.is_dir() {
        let mut dirs = 0usize;
        let mut files = 0usize;
        let mut entries = 0usize;
        if let Ok(rd) = fs::read_dir(path) {
            for e in rd.flatten().take(5000) {
                entries += 1;
                if e.path().is_dir() {
                    dirs += 1;
                } else {
                    files += 1;
                }
            }
        }
        lines.push(Line::from(vec![label("dir_entries"), val(entries.to_string())]));
        lines.push(Line::from(vec![label("dir_count"), val(dirs.to_string())]));
        lines.push(Line::from(vec![label("file_count"), val(files.to_string())]));
    }

    PreviewData::Details(lines)
}
