use std::{
    env,
    io::{self, Write},
    path::Path,
    sync::{Mutex, OnceLock},
};

use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
    Frame,
};

use crate::{app::parse_color, theme::Theme};
#[path = "../fs/thumbs.rs"]
mod thumbs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    Kitty,
    #[cfg(feature = "sixel")]
    Sixel,
}

#[cfg(feature = "sixel")]
use std::process::Command;

#[derive(Debug, Clone, Copy)]
pub struct PreviewImageConfig {
    pub enabled: bool,
    pub max_image_size_mb: u64,
}

const DEFAULT_THUMBNAIL_SIZE: u32 = 256;
const DEFAULT_CACHE_LIFETIME_HRS: u64 = 24;
const MAX_PREVIEW_ROWS: u32 = 20;
const CELL_WIDTH_OVER_HEIGHT: f64 = 0.5;
static LAST_RENDERED_THUMB: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub fn detect_image_protocol() -> Option<ImageProtocol> {
    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default().to_ascii_lowercase();

    if term_program == "ghostty" || term == "xterm-kitty" {
        return Some(ImageProtocol::Kitty);
    }

    if kitty_probe().unwrap_or(false) {
        return Some(ImageProtocol::Kitty);
    }

    #[cfg(feature = "sixel")]
    {
        if term_program == "wezterm" || term == "foot" {
            return Some(ImageProtocol::Sixel);
        }

        if has_sixel_capability(&term) {
            return Some(ImageProtocol::Sixel);
        }
    }

    None
}

fn kitty_probe() -> io::Result<bool> {
    // Best-effort probe; lack of response is treated as unsupported.
    let mut out = io::stdout();
    out.write_all(b"\x1b_Ga=q\x1b\\")?;
    out.flush()?;
    Ok(false)
}

#[cfg(feature = "sixel")]
fn has_sixel_capability(term: &str) -> bool {
    if matches!(term, "xterm-256color" | "foot" | "wezterm") {
        return true;
    }
    if let Ok(output) = Command::new("infocmp").arg("-1").output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
        return text.contains("sixel");
    }
    false
}

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp"))
        .unwrap_or(false)
}

pub fn render_unsupported_mascot(frame: &mut Frame, area: Rect, theme: &Theme, filename: &str, size: &str) {
    let body = parse_color(&theme.mascot.incompatible_body);
    let text = parse_color(&theme.mascot.text);
    let shadow = parse_color(&theme.mascot.shadow);
    let secondary = parse_color(&theme.vars.secondary_fg);

    let mascot = [
        "   ██████",
        "   █████████████████",
        "   ██//ERROR████████▒",
        "   ██INCOMPATIBLE███▒",
        "   ██TERMINAL███████▒",
        "   █████████████████▒",
        "    ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒",
    ];

    let mut lines: Vec<Line<'static>> = Vec::new();
    let top_pad = area.height.saturating_sub((mascot.len() + 3) as u16) / 2;
    for _ in 0..top_pad {
        lines.push(Line::from(""));
    }

    for raw in mascot {
        let spans = raw.chars().map(|c| {
            let style = match c {
                '█' => Style::default().fg(body),
                '▒' => Style::default().fg(shadow),
                '/' | 'E' | 'R' | 'O' | 'N' | 'C' | 'M' | 'P' | 'A' | 'T' | 'I' | 'B' | 'L' => Style::default().fg(text),
                _ => Style::default().fg(body),
            };
            Span::styled(c.to_string(), style)
        }).collect::<Vec<_>>();
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(filename.to_string(), Style::default().fg(secondary))));
    lines.push(Line::from(Span::styled(size.to_string(), Style::default().fg(secondary))));

    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title("Details")
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .style(Style::default().fg(parse_color(&theme.vars.defult_panel_border))),
            ),
        area,
    );
}

pub fn render_image(path: &Path, area: Rect, protocol: ImageProtocol) -> io::Result<()> {
    thumbs::start_cleanup_once(thumbs::ThumbConfig {
        thumbnail_size: DEFAULT_THUMBNAIL_SIZE,
        cache_lifetime_hrs: DEFAULT_CACHE_LIFETIME_HRS,
    });

    let status = thumbs::thumbnail_status(
        path,
        thumbs::ThumbConfig {
            thumbnail_size: DEFAULT_THUMBNAIL_SIZE,
            cache_lifetime_hrs: DEFAULT_CACHE_LIFETIME_HRS,
        },
    );
    let target_path = match status {
        thumbs::ThumbStatus::Ready(p) => p,
        thumbs::ThumbStatus::Generating => {
            clear_preview_area(protocol)?;
            clear_last_rendered_state();
            render_generating(area)?;
            return Ok(());
        }
        thumbs::ThumbStatus::Unavailable => {
            clear_preview_area(protocol)?;
            clear_last_rendered_state();
            render_generating(area)?;
            return Ok(());
        }
    };

    let target_key = target_path.to_string_lossy().to_string();
    if is_same_as_last_render(&target_key) {
        return Ok(());
    }

    clear_preview_area(protocol)?;

    let avail_w = area.width.saturating_sub(2).max(1) as u32;
    let avail_h = (area.height.saturating_sub(2) as u32).min(MAX_PREVIEW_ROWS).max(1);
    let (img_w, img_h) = image::image_dimensions(&target_path).unwrap_or((avail_w, avail_h));
    let (render_w, render_h) = fit_preserving_ratio(img_w, img_h, avail_w, avail_h);
    let x_offset = ((avail_w.saturating_sub(render_w)) / 2) as u16;
    let y_offset = ((avail_h.saturating_sub(render_h)) / 2) as i16;

    let mut cfg = viuer::Config {
        x: area.x.saturating_add(1).saturating_add(x_offset),
        y: area.y.saturating_add(1) as i16 + y_offset,
        width: Some(render_w),
        height: Some(render_h),
        ..Default::default()
    };

    match protocol {
        ImageProtocol::Kitty => cfg.use_kitty = true,
        #[cfg(feature = "sixel")]
        ImageProtocol::Sixel => {
            cfg.use_sixel = true;
        }
    }

    viuer::print_from_file(target_path, &cfg)
        .map(|_| ())
        .map_err(|e| io::Error::other(e.to_string()))?;

    remember_last_render(target_key);
    Ok(())
}

fn clear_preview_area(protocol: ImageProtocol) -> io::Result<()> {
    let mut out = io::stdout();
    if matches!(protocol, ImageProtocol::Kitty) {
        // Delete visible kitty placements before drawing a new image.
        out.write_all(b"\x1b_Ga=d\x1b\\")?;
    }
    out.flush()
}

fn render_generating(area: Rect) -> io::Result<()> {
    let msg = "generating...";
    let x = area
        .x
        .saturating_add(area.width.saturating_sub(msg.len() as u16) / 2)
        .saturating_add(1);
    let y = area.y.saturating_add(area.height / 2).saturating_add(1);
    let mut out = io::stdout();
    let line = format!("\x1b[{};{}H\x1b[90m{}\x1b[0m", y, x, msg);
    out.write_all(line.as_bytes())?;
    out.flush()
}

fn is_same_as_last_render(target_key: &str) -> bool {
    let state = LAST_RENDERED_THUMB.get_or_init(|| Mutex::new(None));
    let Ok(guard) = state.lock() else {
        return false;
    };
    guard.as_deref() == Some(target_key)
}

fn remember_last_render(target_key: String) {
    let state = LAST_RENDERED_THUMB.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = state.lock() {
        *guard = Some(target_key);
    }
}

pub fn clear_last_image(protocol: ImageProtocol) -> io::Result<()> {
    clear_preview_area(protocol)?;
    clear_last_rendered_state();
    Ok(())
}

fn clear_last_rendered_state() {
    let state = LAST_RENDERED_THUMB.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = state.lock() {
        *guard = None;
    }
}

fn fit_preserving_ratio(img_w: u32, img_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if img_w == 0 || img_h == 0 {
        return (max_w.max(1), max_h.max(1));
    }
    // Convert pixel ratio into terminal cell-space (non-square cells).
    let rows_per_col = (img_h as f64 / img_w as f64) * CELL_WIDTH_OVER_HEIGHT;
    let by_width_h = (max_w as f64 * rows_per_col).floor().max(1.0);

    let (w, h) = if by_width_h <= max_h as f64 {
        (max_w as f64, by_width_h)
    } else {
        let w_by_height = ((max_h as f64) / rows_per_col).floor().max(1.0);
        (w_by_height, max_h as f64)
    };

    (
        w as u32,
        h as u32,
    )
}
