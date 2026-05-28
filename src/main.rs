use std::{
    cmp::Ordering,
    collections::HashMap,
    fs,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use git2::{Repository, StatusOptions};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use ratatui::layout::Alignment;
use serde::Deserialize;
use toml::Value;

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    let config = AppConfig::load()?;
    let mut app = App::new(config)?;

    loop {
        terminal.draw(|frame| app.draw(frame))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
                app.handle_key(key);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct AppConfig {
    defaults: Settings,
    #[serde(default)]
    profiles: Vec<Profile>,
    #[serde(default)]
    bookmarks: Vec<Bookmark>,
    #[serde(skip)]
    themes: ThemeRegistry,
}

#[derive(Debug, Clone, Deserialize)]
struct Settings {
    theme: String,
    show_hidden: bool,
    sort: String,
    start_dir: String,
    #[serde(default = "default_top_bar_height")]
    top_bar_height: u16,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default = "default_panel_sizes")]
    panel_sizes: PanelSizes,
    #[serde(default = "default_miller")]
    miller: MillerSettings,
    #[serde(default = "default_search")]
    search: SearchSettings,
    #[serde(default = "default_panels")]
    panels: Panels,
    #[serde(default)]
    hooks: Hooks,
}

#[derive(Debug, Clone, Deserialize)]
struct Profile {
    name: String,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    show_hidden: Option<bool>,
    #[serde(default)]
    sort: Option<String>,
    #[serde(default)]
    start_dir: Option<String>,
    #[serde(default)]
    top_bar_height: Option<u16>,
    #[serde(default)]
    features: Option<Vec<String>>,
    #[serde(default)]
    panel_sizes: Option<PanelSizesOverrides>,
    #[serde(default)]
    miller: Option<MillerSettingsOverrides>,
    #[serde(default)]
    search: Option<SearchOverrides>,
    #[serde(default)]
    panels: Option<PanelOverrides>,
    #[serde(default)]
    hooks: Option<Hooks>,
}

#[derive(Debug, Clone, Deserialize)]
struct Panels {
    #[serde(default = "default_true")]
    sidebar: bool,
    #[serde(default = "default_true")]
    columns: bool,
    #[serde(default = "default_true")]
    preview: bool,
    #[serde(default = "default_true")]
    search_bar: bool,
    #[serde(default = "default_true")]
    status_bar: bool,
    #[serde(default = "default_true")]
    keymap_bar: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PanelOverrides {
    #[serde(default)]
    sidebar: Option<bool>,
    #[serde(default)]
    columns: Option<bool>,
    #[serde(default)]
    preview: Option<bool>,
    #[serde(default)]
    search_bar: Option<bool>,
    #[serde(default)]
    status_bar: Option<bool>,
    #[serde(default)]
    keymap_bar: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct PanelSizes {
    #[serde(default = "default_size_sidebar")]
    sidebar: u16,
    #[serde(default = "default_size_dir")]
    dir: u16,
    #[serde(default = "default_size_preview")]
    preview: u16,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PanelSizesOverrides {
    #[serde(default)]
    sidebar: Option<u16>,
    #[serde(default)]
    dir: Option<u16>,
    #[serde(default)]
    preview: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
struct MillerSettings {
    #[serde(default = "default_max_columns")]
    max_columns: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct MillerSettingsOverrides {
    #[serde(default)]
    max_columns: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct SearchSettings {
    #[serde(default)]
    ignored_dirs: Vec<String>,
    #[serde(default = "default_search_max_depth")]
    max_depth: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SearchOverrides {
    #[serde(default)]
    ignored_dirs: Option<Vec<String>>,
    #[serde(default)]
    max_depth: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct UiColors {
    #[serde(default = "default_color_bg")]
    background: String,
    #[serde(default = "default_color_fg")]
    foreground: String,
    #[serde(default = "default_color_accent")]
    accent: String,
    #[serde(default = "default_color_status")]
    status_bar: String,
    #[serde(default = "default_color_keymap")]
    keymap_bar: String,
    #[serde(default = "default_color_search")]
    search_bar: String,
    #[serde(default = "default_color_selection_bg")]
    selection_bg: String,
    #[serde(default = "default_color_selection_fg")]
    selection_fg: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Hooks {
    #[serde(default)]
    on_open: Option<String>,
    #[serde(default)]
    on_select: Option<String>,
    #[serde(default)]
    on_rename: Option<String>,
    #[serde(default)]
    on_delete: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Bookmark {
    name: String,
    path: String,
}

#[derive(Debug, Clone)]
struct EffectiveSettings {
    theme: String,
    show_hidden: bool,
    sort: String,
    start_dir: PathBuf,
    top_bar_height: u16,
    features: Vec<String>,
    ui_colors: UiColors,
    panel_sizes: PanelSizes,
    miller: MillerSettings,
    search: SearchSettings,
    panels: Panels,
    hooks: Hooks,
}

#[derive(Debug, Clone, Default)]
struct ThemeRegistry {
    by_name: HashMap<String, UiColors>,
}

#[derive(Debug, Clone, Deserialize)]
struct ThemeFile {
    #[serde(default = "default_ui_colors")]
    ui_colors: UiColors,
}

impl AppConfig {
    fn load() -> Result<Self> {
        let config_path = config_path();
        if !config_path.exists() {
            write_default_config(&config_path)?;
        }
        let raw = fs::read_to_string(&config_path)?;
        ensure_default_themes()?;
        let default_value: Value = toml::from_str(default_config_contents())?;
        let (mut healed_value, mut changed) = match toml::from_str::<Value>(&raw) {
            Ok(mut user_value) => {
                let changed = heal_value(&mut user_value, &default_value);
                (user_value, changed)
            }
            Err(_) => (default_value.clone(), true),
        };
        if strip_color_blocks(&mut healed_value) {
            changed = true;
        }

        if changed {
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&config_path, toml::to_string_pretty(&healed_value)?)?;
        }

        let mut config: AppConfig = healed_value.try_into()?;
        config.themes = load_themes_registry()?;
        Ok(config)
    }
}

impl Settings {
    fn merge_with_profile(&self, profile: Option<&Profile>, themes: &ThemeRegistry) -> EffectiveSettings {
        let theme = profile
            .and_then(|p| p.theme.clone())
            .unwrap_or_else(|| self.theme.clone());
        let show_hidden = profile
            .and_then(|p| p.show_hidden)
            .unwrap_or(self.show_hidden);
        let sort = profile
            .and_then(|p| p.sort.clone())
            .unwrap_or_else(|| self.sort.clone());
        let start_dir = profile
            .and_then(|p| p.start_dir.clone())
            .unwrap_or_else(|| self.start_dir.clone());
        let top_bar_height = profile
            .and_then(|p| p.top_bar_height)
            .unwrap_or(self.top_bar_height)
            .max(1);
        let features = profile
            .and_then(|p| p.features.clone())
            .unwrap_or_else(|| self.features.clone());
        let theme_colors = themes
            .by_name
            .get(&theme)
            .cloned()
            .unwrap_or_else(default_ui_colors);
        let panel_sizes = merge_panel_sizes(
            &self.panel_sizes,
            profile.and_then(|p| p.panel_sizes.as_ref()),
        );
        let miller = merge_miller(
            &self.miller,
            profile.and_then(|p| p.miller.as_ref()),
        );
        let search = merge_search(
            &self.search,
            profile.and_then(|p| p.search.as_ref()),
        );
        let panels = merge_panels(
            &self.panels,
            profile.and_then(|p| p.panels.as_ref()),
        );
        let hooks = profile
            .and_then(|p| p.hooks.clone())
            .unwrap_or_else(|| self.hooks.clone());

        EffectiveSettings {
            theme,
            show_hidden,
            sort,
            start_dir: normalize_start_dir(&start_dir),
            top_bar_height,
            features,
            ui_colors: theme_colors,
            panel_sizes,
            miller,
            search,
            panels,
            hooks,
        }
    }
}

struct App {
    config: AppConfig,
    active_profile: usize,
    effective: EffectiveSettings,
    recents: Vec<PathBuf>,
    drives: Vec<String>,
    current_dir: PathBuf,
    columns: Vec<DirColumn>,
    search_mode: SearchMode,
    search_query: String,
    global_results: Vec<PathBuf>,
    global_selected: usize,
    input_mode: InputMode,
    input_buffer: String,
    clipboard: Option<ClipboardItem>,
    status_message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    None,
    Local,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    None,
    NewFile,
    NewDir,
}

#[derive(Debug, Clone)]
struct ClipboardItem {
    path: PathBuf,
    cut: bool,
}

enum PreviewData {
    Empty,
    Details(String),
}

impl App {
    fn new(config: AppConfig) -> Result<Self> {
        let active_profile = 0;
        let effective = config
            .defaults
            .merge_with_profile(config.profiles.get(active_profile), &config.themes);
        let recents = load_recents()?;
        let drives = discover_drives();
        let current_dir = effective.start_dir.clone();
        let columns = build_columns_from_path(&current_dir, &effective);
        Ok(Self {
            config,
            active_profile,
            effective,
            recents,
            drives,
            current_dir,
            columns,
            search_mode: SearchMode::None,
            search_query: String::new(),
            global_results: Vec::new(),
            global_selected: 0,
            input_mode: InputMode::None,
            input_buffer: String::new(),
            clipboard: None,
            status_message: String::new(),
        })
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.input_mode != InputMode::None {
            self.handle_input_mode(key);
            return;
        }
        if self.search_mode != SearchMode::None {
            self.handle_search_input(key);
            return;
        }
        match key.code {
            KeyCode::Char('p') => self.next_profile(),
            KeyCode::Char('P') => self.prev_profile(),
            KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') => self.enter_selected_dir(),
            KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h') => self.go_parent(),
            KeyCode::Char('/') => self.start_local_search(),
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.start_global_search();
            }
            KeyCode::Char('a') => self.start_new_file(),
            KeyCode::Char('A') => self.start_new_dir(),
            KeyCode::Char('y') => self.copy_selected(),
            KeyCode::Char('x') => self.cut_selected(),
            KeyCode::Char('v') => self.paste_clipboard(),
            KeyCode::Char('d') => self.delete_selected_to_trash(),
            _ => {}
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let base_style = Style::default()
            .bg(parse_color(&self.effective.ui_colors.background))
            .fg(parse_color(&self.effective.ui_colors.foreground));
        frame.render_widget(Paragraph::new("").style(base_style), frame.area());

        let mut rows = Vec::new();
        if self.effective.panels.search_bar {
            rows.push(Constraint::Length(self.effective.top_bar_height));
        }
        rows.push(Constraint::Min(1));
        if self.effective.panels.status_bar {
            rows.push(Constraint::Length(1));
        }
        if self.effective.panels.keymap_bar {
            rows.push(Constraint::Length(1));
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(rows)
            .split(frame.area());

        let mut row_index = 0;
        if self.effective.panels.search_bar {
            self.draw_top_bar(frame, chunks[row_index]);
            row_index += 1;
        }

        let main_area = chunks[row_index];
        row_index += 1;

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(self.main_panel_constraints())
            .split(main_area);

        let mut panel_index = 0;
        if self.effective.panels.sidebar {
            self.draw_sidebar(frame, main_chunks[panel_index]);
            panel_index += 1;
        }
        if self.effective.panels.columns {
            self.draw_miller_columns(frame, main_chunks[panel_index]);
            panel_index += 1;
        }
        if self.effective.panels.preview {
            self.draw_details_panel(frame, main_chunks[panel_index]);
            panel_index += 1;
        }
        if panel_index == 0 {
            let p = Paragraph::new("All main panels are disabled in config.")
                .block(Block::default().title("DIRT").borders(Borders::ALL));
            frame.render_widget(p, main_area);
        }

        if self.effective.panels.status_bar {
            self.draw_status(frame, chunks[row_index]);
            row_index += 1;
        }
        if self.effective.panels.keymap_bar {
            self.draw_keymap_bar(frame, chunks[row_index]);
        }
    }

    fn main_panel_constraints(&self) -> Vec<Constraint> {
        let mut weights: Vec<u16> = Vec::new();
        if self.effective.panels.sidebar {
            weights.push(self.effective.panel_sizes.sidebar.max(1));
        }
        if self.effective.panels.columns {
            weights.push(self.effective.panel_sizes.dir.max(1));
        }
        if self.effective.panels.preview {
            weights.push(self.effective.panel_sizes.preview.max(1));
        }
        let total: u32 = weights.iter().map(|w| *w as u32).sum();
        if total == 0 {
            return vec![Constraint::Fill(1)];
        }
        weights
            .into_iter()
            .map(|w| Constraint::Ratio(w as u32, total))
            .collect()
    }

    fn draw_top_bar(&self, frame: &mut Frame, area: Rect) {
        let constraints = self.main_panel_constraints();
        let segments = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        let mut seg_idx = 0;
        if self.effective.panels.sidebar {
            let profile_name = self
                .config
                .profiles
                .get(self.active_profile)
                .map(|p| p.name.as_str())
                .unwrap_or("default");
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("DIRT // ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(profile_name.to_string()),
                ]))
                    .style(Style::default().fg(parse_color(&self.effective.ui_colors.search_bar)))
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(parse_color(&self.effective.ui_colors.search_bar))),
                segments[seg_idx],
            );
            seg_idx += 1;
        }

        if self.effective.panels.columns {
            let search_label = match self.search_mode {
                SearchMode::None => "fuzzy search [placeholder]".to_string(),
                SearchMode::Local => format!("/ {}", self.search_query),
                SearchMode::Global => format!("Ctrl+F {}", self.search_query),
            };
            let label = if self.input_mode == InputMode::None {
                search_label
            } else {
                match self.input_mode {
                    InputMode::NewFile => format!("new file: {}", self.input_buffer),
                    InputMode::NewDir => format!("new dir: {}", self.input_buffer),
                    InputMode::None => search_label,
                }
            };
            frame.render_widget(
                Paragraph::new(label)
                    .style(Style::default().fg(parse_color(&self.effective.ui_colors.search_bar)))
                    .block(Block::default().borders(Borders::ALL)),
                segments[seg_idx],
            );
            seg_idx += 1;
        }

        if self.effective.panels.preview {
            frame.render_widget(
                Paragraph::new("")
                    .style(Style::default().fg(parse_color(&self.effective.ui_colors.search_bar)))
                    .block(Block::default().borders(Borders::ALL)),
                segments[seg_idx],
            );
        }
    }

    fn draw_sidebar(&self, frame: &mut Frame, area: Rect) {
        let mut rows = Vec::new();

        rows.push(ListItem::new(Line::from("Bookmarks").style(Style::default().add_modifier(Modifier::BOLD))));
        for b in &self.config.bookmarks {
            rows.push(ListItem::new(format!("  {} -> {}", b.name, b.path)));
        }

        rows.push(ListItem::new(""));
        rows.push(ListItem::new(Line::from("Drives").style(Style::default().add_modifier(Modifier::BOLD))));
        for drive in &self.drives {
            rows.push(ListItem::new(format!("  {}", drive)));
        }

        rows.push(ListItem::new(""));
        rows.push(ListItem::new(Line::from("Recent Dirs").style(Style::default().add_modifier(Modifier::BOLD))));
        for recent in &self.recents {
            rows.push(ListItem::new(format!("  {}", recent.display())));
        }

        let list = List::new(rows).block(
            Block::default()
                .title("Sidebar")
                .borders(Borders::ALL)
                .style(
                    Style::default()
                        .fg(parse_color(&self.effective.ui_colors.accent))
                        .bg(parse_color(&self.effective.ui_colors.background)),
                ),
        );
        frame.render_widget(list, area);
    }

    fn draw_miller_columns(&self, frame: &mut Frame, area: Rect) {
        if self.search_mode == SearchMode::Global {
            self.draw_global_results(frame, area);
            return;
        }
        let max_columns = self.effective.miller.max_columns.max(2);
        let (path_cols, outer_label) = self.visible_path_columns_fixed(max_columns);
        let outer = Block::default()
            .title(outer_label)
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .fg(parse_color(&self.effective.ui_colors.accent))
                    .bg(parse_color(&self.effective.ui_colors.background)),
            );
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        if inner.width < 3 || inner.height < 3 {
            return;
        }

        if max_columns == 0 {
            return;
        }
        let constraints = (0..max_columns).map(|_| Constraint::Fill(1)).collect::<Vec<_>>();
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(inner);

        let active_render_idx = max_columns.saturating_sub(2);
        for idx in 0..max_columns {
            if idx == max_columns - 1 {
                if self.columns.len() <= 1 {
                    let p = Paragraph::new("YOU HAVE GONE TOO FAR!!!")
                        .alignment(Alignment::Center)
                        .block(
                            Block::default()
                                .title("Next")
                                .borders(Borders::ALL)
                                .style(
                                    Style::default()
                                        .fg(parse_color(&self.effective.ui_colors.accent))
                                        .bg(parse_color(&self.effective.ui_colors.background)),
                                ),
                        );
                    frame.render_widget(p, chunks[idx]);
                } else if let Some(selected_dir) = self.selected_dir_path() {
                    let preview_col = DirColumn::from_path(selected_dir.to_path_buf(), &self.effective);
                    if preview_col.entries.is_empty() {
                        let p = Paragraph::new("YOU HAVE GONE TOO FAR!!!")
                            .alignment(Alignment::Center)
                            .block(
                                Block::default()
                                    .title("Next")
                                    .borders(Borders::ALL)
                                    .style(
                                        Style::default()
                                            .fg(parse_color(&self.effective.ui_colors.accent))
                                            .bg(parse_color(&self.effective.ui_colors.background)),
                                    ),
                            );
                        frame.render_widget(p, chunks[idx]);
                    } else {
                        let rows = preview_col
                            .entries
                            .iter()
                            .map(|e| {
                                let kind = if e.is_dir { "/" } else { "" };
                                ListItem::new(format!(" {}{}", e.name, kind))
                            })
                            .collect::<Vec<_>>();
                        let list = List::new(rows).block(
                            Block::default()
                                .title(format!("/{}", path_last_segment(selected_dir)))
                                .borders(Borders::ALL)
                                .style(
                                    Style::default()
                                        .fg(parse_color(&self.effective.ui_colors.accent))
                                        .bg(parse_color(&self.effective.ui_colors.background)),
                                ),
                        );
                        frame.render_widget(list, chunks[idx]);
                    }
                } else {
                    let p = Paragraph::new("YOU HAVE GONE TOO FAR!!!")
                        .alignment(Alignment::Center)
                        .block(
                            Block::default()
                                .title("Next")
                                .borders(Borders::ALL)
                                .style(
                                    Style::default()
                                        .fg(parse_color(&self.effective.ui_colors.accent))
                                        .bg(parse_color(&self.effective.ui_colors.background)),
                                ),
                        );
                    frame.render_widget(p, chunks[idx]);
                }
                continue;
            }

            let maybe_col = path_cols.get(idx).and_then(|c| c.clone());
            let Some(col) = maybe_col else {
                let p = Paragraph::new("")
                    .block(
                        Block::default()
                            .title("/")
                            .borders(Borders::ALL)
                            .style(
                                Style::default()
                                    .fg(parse_color(&self.effective.ui_colors.accent))
                                    .bg(parse_color(&self.effective.ui_colors.background)),
                            ),
                    );
                frame.render_widget(p, chunks[idx]);
                continue;
            };

            let is_focused_column = idx == active_render_idx;
            let next_visible_path = if idx + 1 < path_cols.len() {
                path_cols[idx + 1].as_ref().map(|c| c.path.as_path())
            } else {
                self.selected_dir_path()
            };
            let filtered_indices = if is_focused_column && self.search_mode == SearchMode::Local {
                local_filtered_indices_with_query(&self.search_query, col)
            } else {
                (0..col.entries.len()).collect::<Vec<_>>()
            };
            let viewport_height = chunks[idx].height.saturating_sub(2) as usize;
            let anchor_idx = if is_focused_column {
                filtered_indices.iter().position(|&x| x == col.selected).unwrap_or(0)
            } else {
                next_visible_path
                    .and_then(|p| col.entries.iter().position(|e| e.path == p))
                    .and_then(|abs| filtered_indices.iter().position(|&x| x == abs))
                    .unwrap_or(0)
            };
            let start = scroll_start(filtered_indices.len(), viewport_height, anchor_idx);
            let end = (start + viewport_height).min(filtered_indices.len());
            let mut rows = Vec::new();
            for filtered_row_idx in start..end {
                let absolute_idx = filtered_indices[filtered_row_idx];
                let entry = &col.entries[absolute_idx];
                let kind = if entry.is_dir { "/" } else { "" };
                let mut item = ListItem::new(format!(" {}{}", entry.name, kind));
                let should_highlight = (is_focused_column && absolute_idx == col.selected)
                    || next_visible_path
                        .map(|p| entry.path == p)
                        .unwrap_or(false);
                if should_highlight {
                    item = item.style(
                        Style::default()
                            .bg(parse_color(&self.effective.ui_colors.selection_bg))
                            .fg(parse_color(&self.effective.ui_colors.selection_fg)),
                    );
                }
                rows.push(item);
            }
            if filtered_indices.is_empty() {
                rows.push(ListItem::new("  <empty>"));
            }
            let title = format!("/{}", path_last_segment(&col.path));
            let list = List::new(rows).block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .style(
                        Style::default()
                            .fg(parse_color(&self.effective.ui_colors.accent))
                            .bg(parse_color(&self.effective.ui_colors.background)),
                    ),
            );
            frame.render_widget(list, chunks[idx]);
        }
    }

    fn visible_path_columns_fixed(&self, max_columns: usize) -> (Vec<Option<&DirColumn>>, String) {
        let path_slots = max_columns.saturating_sub(1);
        let depth = self.columns.len();
        let visible_count = depth.min(path_slots);
        let start = depth.saturating_sub(visible_count);
        let mut cols = Vec::new();
        let pad = path_slots.saturating_sub(visible_count);
        for _ in 0..pad {
            cols.push(None);
        }
        for col in &self.columns[start..] {
            cols.push(Some(col));
        }

        let outer_label = if let Some(first_real) = self.columns.get(start) {
            first_real
                .path
                .parent()
                .map(display_home_relative)
                .unwrap_or_else(|| display_home_relative(&first_real.path))
        } else {
            display_home_relative(&self.current_dir)
        };
        (cols, outer_label)
    }

    fn selected_dir_path(&self) -> Option<&Path> {
        let current = self.columns.last()?;
        let selected = current.entries.get(current.selected)?;
        if selected.is_dir {
            Some(&selected.path)
        } else {
            None
        }
    }

    fn select_prev(&mut self) {
        if self.search_mode == SearchMode::Global {
            if self.global_selected > 0 {
                self.global_selected -= 1;
            }
            return;
        }
        let Some(current) = self.columns.last_mut() else {
            return;
        };
        if self.search_mode == SearchMode::Local {
            let filtered = local_filtered_indices_with_query(&self.search_query, current);
            if filtered.is_empty() {
                return;
            }
            if let Some(pos) = filtered.iter().position(|&i| i == current.selected) {
                if pos > 0 {
                    current.selected = filtered[pos - 1];
                }
            } else {
                current.selected = filtered[0];
            }
            return;
        }
        if current.selected > 0 {
            current.selected -= 1;
        }
    }

    fn select_next(&mut self) {
        if self.search_mode == SearchMode::Global {
            if self.global_selected + 1 < self.global_results.len() {
                self.global_selected += 1;
            }
            return;
        }
        let Some(current) = self.columns.last_mut() else {
            return;
        };
        if self.search_mode == SearchMode::Local {
            let filtered = local_filtered_indices_with_query(&self.search_query, current);
            if filtered.is_empty() {
                return;
            }
            if let Some(pos) = filtered.iter().position(|&i| i == current.selected) {
                if pos + 1 < filtered.len() {
                    current.selected = filtered[pos + 1];
                }
            } else {
                current.selected = filtered[0];
            }
            return;
        }
        if current.selected + 1 < current.entries.len() {
            current.selected += 1;
        }
    }

    fn enter_selected_dir(&mut self) {
        if self.search_mode == SearchMode::Global {
            self.open_global_selected();
            return;
        }
        let Some(current) = self.columns.last() else {
            return;
        };
        let Some(selected) = current.entries.get(current.selected) else {
            return;
        };
        if !selected.is_dir {
            return;
        };
        let path = selected.path.clone();
        self.columns.push(DirColumn::from_path(path.clone(), &self.effective));
        self.current_dir = path;
    }

    fn go_parent(&mut self) {
        if self.columns.len() <= 1 {
            return;
        }
        self.columns.pop();
        if let Some(col) = self.columns.last() {
            self.current_dir = col.path.clone();
        }
    }

    fn current_file_count(&self) -> usize {
        self.columns.last().map(|c| c.entries.len()).unwrap_or(0)
    }

    fn selected_name(&self) -> String {
        if self.search_mode == SearchMode::Global {
            return self
                .global_results
                .get(self.global_selected)
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| "-".to_string());
        }
        self.columns
            .last()
            .and_then(|c| c.entries.get(c.selected))
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "-".to_string())
    }

    fn reinitialize_columns(&mut self) {
        self.columns = build_columns_from_path(&self.effective.start_dir, &self.effective);
        self.current_dir = self.effective.start_dir.clone();
    }

    fn reinitialize_columns_preserve_current(&mut self) {
        let target = if self.current_dir.is_dir() {
            self.current_dir.clone()
        } else {
            self.effective.start_dir.clone()
        };
        self.columns = build_columns_from_path(&target, &self.effective);
        self.current_dir = target;
    }

    fn draw_details_panel(&self, frame: &mut Frame, area: Rect) {
        let details_text = match self.current_preview() {
            PreviewData::Empty => "No selection".to_string(),
            PreviewData::Details(content) => content,
        };
        let p = Paragraph::new(details_text)
            .block(
                Block::default()
                    .title("Details")
                    .borders(Borders::ALL)
                    .style(
                        Style::default()
                            .fg(parse_color(&self.effective.ui_colors.accent))
                            .bg(parse_color(&self.effective.ui_colors.background)),
                    ),
            );
        frame.render_widget(p, area);
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let profile_name = self
            .config
            .profiles
            .get(self.active_profile)
            .map(|p| p.name.as_str())
            .unwrap_or("default");
        let text = format!(
            "{} · files: {} · selected: {} · git: {} · profile: {} · {}",
            self.current_dir.display(),
            self.current_file_count(),
            self.selected_name(),
            self.git_status_text(),
            profile_name,
            if self.status_message.is_empty() {
                "ready"
            } else {
                self.status_message.as_str()
            }
        );
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(parse_color(&self.effective.ui_colors.status_bar))),
            area,
        );
    }

    fn draw_keymap_bar(&self, frame: &mut Frame, area: Rect) {
        let text = "↑/k ↓/j nav · →/Enter open · ← up · / local · Ctrl+F global · a/A new file/dir · y/x/v copy/cut/paste · d trash · p/P profile · q quit";
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(parse_color(&self.effective.ui_colors.keymap_bar))),
            area,
        );
    }

    fn next_profile(&mut self) {
        if self.config.profiles.is_empty() {
            return;
        }
        self.active_profile = (self.active_profile + 1) % self.config.profiles.len();
        self.effective = self
            .config
            .defaults
            .merge_with_profile(self.config.profiles.get(self.active_profile), &self.config.themes);
        self.reinitialize_columns_preserve_current();
    }

    fn prev_profile(&mut self) {
        if self.config.profiles.is_empty() {
            return;
        }
        self.active_profile = if self.active_profile == 0 {
            self.config.profiles.len() - 1
        } else {
            self.active_profile - 1
        };
        self.effective = self
            .config
            .defaults
            .merge_with_profile(self.config.profiles.get(self.active_profile), &self.config.themes);
        self.reinitialize_columns_preserve_current();
    }

    fn selected_entry(&self) -> Option<&DirEntry> {
        if self.search_mode == SearchMode::Global {
            return None;
        }
        let current = self.columns.last()?;
        current.entries.get(current.selected)
    }

    fn selected_entry_path(&self) -> Option<PathBuf> {
        self.selected_entry().map(|e| e.path.clone())
    }

    fn current_preview(&self) -> PreviewData {
        if self.search_mode == SearchMode::Global {
            return self
                .global_results
                .get(self.global_selected)
                .map(|p| preview_for_path(p))
                .unwrap_or(PreviewData::Empty);
        }
        let Some(entry) = self.selected_entry() else {
            return PreviewData::Empty;
        };
        preview_for_path(&entry.path)
    }

    fn git_status_text(&self) -> String {
        let Ok(repo) = Repository::discover(&self.current_dir) else {
            return "-".to_string();
        };
        let branch = repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(|s| s.to_string()))
            .unwrap_or_else(|| "detached".to_string());

        let mut opts = StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);
        let dirty = repo
            .statuses(Some(&mut opts))
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if dirty {
            format!("{branch}*")
        } else {
            branch
        }
    }

    fn draw_global_results(&self, frame: &mut Frame, area: Rect) {
        let outer = Block::default()
            .title(format!("Global Search ({})", self.global_results.len()))
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .fg(parse_color(&self.effective.ui_colors.accent))
                    .bg(parse_color(&self.effective.ui_colors.background)),
            );
        let inner = outer.inner(area);
        frame.render_widget(outer, area);
        if inner.height <= 2 {
            return;
        }

        let viewport = inner.height as usize;
        let start = scroll_start(self.global_results.len(), viewport, self.global_selected);
        let end = (start + viewport).min(self.global_results.len());
        let mut rows = Vec::new();
        for idx in start..end {
            let p = &self.global_results[idx];
            let mut item = ListItem::new(display_home_relative(p));
            if idx == self.global_selected {
                item = item.style(
                    Style::default()
                        .bg(parse_color(&self.effective.ui_colors.selection_bg))
                        .fg(parse_color(&self.effective.ui_colors.selection_fg)),
                );
            }
            rows.push(item);
        }
        if rows.is_empty() {
            rows.push(ListItem::new("No matches"));
        }
        frame.render_widget(List::new(rows), inner);
    }

    fn local_filtered_indices(&self, col: &DirColumn) -> Vec<usize> {
        local_filtered_indices_with_query(&self.search_query, col)
    }

    
}

fn local_filtered_indices_with_query(query: &str, col: &DirColumn) -> Vec<usize> {
        if query.is_empty() {
            return (0..col.entries.len()).collect();
        }
        let q = query.to_ascii_lowercase();
        col.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if fuzzy_contains(&e.name.to_ascii_lowercase(), &q) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
}

impl App {

    fn start_local_search(&mut self) {
        self.search_mode = SearchMode::Local;
        self.search_query.clear();
    }

    fn start_global_search(&mut self) {
        self.search_mode = SearchMode::Global;
        self.search_query.clear();
        self.run_global_search();
    }

    fn handle_search_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_mode = SearchMode::None;
                self.search_query.clear();
                self.global_results.clear();
                self.global_selected = 0;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.on_search_query_changed();
            }
            KeyCode::Enter => {
                if self.search_mode == SearchMode::Global {
                    self.open_global_selected();
                } else {
                    self.search_mode = SearchMode::None;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search_query.push(c);
                self.on_search_query_changed();
            }
            _ => {}
        }
    }

    fn on_search_query_changed(&mut self) {
        match self.search_mode {
            SearchMode::Local => {
                let filtered = self
                    .columns
                    .last()
                    .map(|c| self.local_filtered_indices(c))
                    .unwrap_or_default();
                if let Some(first) = filtered.first().copied()
                    && let Some(current) = self.columns.last_mut()
                {
                    current.selected = first;
                }
            }
            SearchMode::Global => self.run_global_search(),
            SearchMode::None => {}
        }
    }

    fn run_global_search(&mut self) {
        self.global_results = global_search_paths(
            &self.current_dir,
            &self.search_query,
            self.effective.search.max_depth,
            &self.effective.search.ignored_dirs,
        );
        self.global_selected = 0;
    }

    fn open_global_selected(&mut self) {
        let Some(path) = self.global_results.get(self.global_selected).cloned() else {
            return;
        };
        let target_dir = if path.is_dir() {
            path
        } else {
            path.parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.current_dir.clone())
        };
        self.current_dir = target_dir.clone();
        self.columns = build_columns_from_path(&target_dir, &self.effective);
        self.search_mode = SearchMode::None;
        self.search_query.clear();
        self.global_results.clear();
        self.global_selected = 0;
    }

    fn handle_input_mode(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::None;
                self.input_buffer.clear();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => {
                self.commit_input_mode();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    fn start_new_file(&mut self) {
        self.input_mode = InputMode::NewFile;
        self.input_buffer.clear();
    }

    fn start_new_dir(&mut self) {
        self.input_mode = InputMode::NewDir;
        self.input_buffer.clear();
    }

    fn commit_input_mode(&mut self) {
        let name = self.input_buffer.trim();
        if name.is_empty() {
            self.status_message = "name cannot be empty".to_string();
            self.input_mode = InputMode::None;
            self.input_buffer.clear();
            return;
        }
        let target = self.current_dir.join(name);
        let result = match self.input_mode {
            InputMode::NewFile => fs::write(&target, ""),
            InputMode::NewDir => fs::create_dir_all(&target),
            InputMode::None => Ok(()),
        };
        self.status_message = match result {
            Ok(_) => format!("created {}", target.display()),
            Err(e) => format!("create failed: {e}"),
        };
        self.input_mode = InputMode::None;
        self.input_buffer.clear();
        self.refresh_active_column();
    }

    fn copy_selected(&mut self) {
        let Some(path) = self.selected_entry_path() else {
            self.status_message = "nothing selected".to_string();
            return;
        };
        self.clipboard = Some(ClipboardItem { path, cut: false });
        self.status_message = "copied".to_string();
    }

    fn cut_selected(&mut self) {
        let Some(path) = self.selected_entry_path() else {
            self.status_message = "nothing selected".to_string();
            return;
        };
        self.clipboard = Some(ClipboardItem { path, cut: true });
        self.status_message = "cut".to_string();
    }

    fn paste_clipboard(&mut self) {
        let Some(item) = self.clipboard.clone() else {
            self.status_message = "clipboard empty".to_string();
            return;
        };
        let Some(name) = item.path.file_name() else {
            self.status_message = "invalid clipboard path".to_string();
            return;
        };
        let mut destination = self.current_dir.join(name);
        if destination == item.path {
            destination = self.current_dir.join(format!("{}_copy", name.to_string_lossy()));
        }

        let result = if item.cut {
            move_path(&item.path, &destination)
        } else {
            copy_path(&item.path, &destination)
        };

        self.status_message = match result {
            Ok(_) => {
                if item.cut {
                    self.clipboard = None;
                    format!("moved to {}", destination.display())
                } else {
                    format!("copied to {}", destination.display())
                }
            }
            Err(e) => format!("paste failed: {e}"),
        };
        self.refresh_active_column();
    }

    fn delete_selected_to_trash(&mut self) {
        let Some(path) = self.selected_entry_path() else {
            self.status_message = "nothing selected".to_string();
            return;
        };
        self.status_message = match trash::delete(&path) {
            Ok(_) => format!("trashed {}", path.display()),
            Err(e) => format!("trash failed: {e}"),
        };
        self.refresh_active_column();
    }

    fn refresh_active_column(&mut self) {
        if let Some(col) = self.columns.last_mut() {
            col.entries = read_dir_entries(&col.path, &self.effective);
            if col.entries.is_empty() {
                col.selected = 0;
            } else if col.selected >= col.entries.len() {
                col.selected = col.entries.len() - 1;
            }
        }
    }
}

#[derive(Debug, Clone)]
struct DirEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

#[derive(Debug, Clone)]
struct DirColumn {
    path: PathBuf,
    entries: Vec<DirEntry>,
    selected: usize,
}

impl DirColumn {
    fn from_path(path: PathBuf, settings: &EffectiveSettings) -> Self {
        let entries = read_dir_entries(&path, settings);
        Self {
            path,
            entries,
            selected: 0,
        }
    }
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path.push("dirt.toml");
    path
}

fn themes_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path.push("themes");
    path
}

fn state_dir() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path
}

fn load_recents() -> Result<Vec<PathBuf>> {
    let mut recents_path = state_dir();
    recents_path.push("recents.toml");
    if !recents_path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(recents_path)?;
    let recents: RecentsState = toml::from_str(&raw)?;
    Ok(recents
        .recent_dirs
        .into_iter()
        .map(|p| expand_tilde(&p))
        .collect())
}

#[derive(Debug, Deserialize)]
struct RecentsState {
    #[serde(default)]
    recent_dirs: Vec<String>,
}

fn discover_drives() -> Vec<String> {
    #[cfg(target_family = "unix")]
    {
        let mut mounts = vec!["/".to_string()];
        if let Ok(content) = fs::read_to_string("/proc/mounts") {
            for line in content.lines().take(20) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 1 {
                    let mountpoint = parts[1];
                    if mountpoint.starts_with("/mnt") || mountpoint.starts_with("/media") {
                        mounts.push(mountpoint.to_string());
                    }
                }
            }
        }
        mounts.sort();
        mounts.dedup();
        mounts
    }
    #[cfg(not(target_family = "unix"))]
    {
        vec!["/".to_string()]
    }
}

fn default_true() -> bool {
    true
}

fn default_panels() -> Panels {
    Panels {
        sidebar: true,
        columns: true,
        preview: true,
        search_bar: true,
        status_bar: true,
        keymap_bar: true,
    }
}

fn default_ui_colors() -> UiColors {
    UiColors {
        background: default_color_bg(),
        foreground: default_color_fg(),
        accent: default_color_accent(),
        status_bar: default_color_status(),
        keymap_bar: default_color_keymap(),
        search_bar: default_color_search(),
        selection_bg: default_color_selection_bg(),
        selection_fg: default_color_selection_fg(),
    }
}

fn default_panel_sizes() -> PanelSizes {
    PanelSizes {
        sidebar: default_size_sidebar(),
        dir: default_size_dir(),
        preview: default_size_preview(),
    }
}

fn default_miller() -> MillerSettings {
    MillerSettings {
        max_columns: default_max_columns(),
    }
}

fn default_search() -> SearchSettings {
    SearchSettings {
        ignored_dirs: vec![".git".to_string(), "node_modules".to_string()],
        max_depth: default_search_max_depth(),
    }
}

fn default_size_sidebar() -> u16 {
    1
}

fn default_size_dir() -> u16 {
    4
}

fn default_size_preview() -> u16 {
    1
}

fn default_top_bar_height() -> u16 {
    3
}

fn default_max_columns() -> usize {
    3
}

fn default_search_max_depth() -> usize {
    6
}

fn default_color_bg() -> String {
    "black".to_string()
}

fn default_color_fg() -> String {
    "white".to_string()
}

fn default_color_accent() -> String {
    "gray".to_string()
}

fn default_color_status() -> String {
    "white".to_string()
}

fn default_color_keymap() -> String {
    "gray".to_string()
}

fn default_color_search() -> String {
    "white".to_string()
}

fn default_color_selection_bg() -> String {
    "white".to_string()
}

fn default_color_selection_fg() -> String {
    "black".to_string()
}

fn merge_panels(base: &Panels, overrides: Option<&PanelOverrides>) -> Panels {
    let Some(overrides) = overrides else {
        return base.clone();
    };
    Panels {
        sidebar: overrides.sidebar.unwrap_or(base.sidebar),
        columns: overrides.columns.unwrap_or(base.columns),
        preview: overrides.preview.unwrap_or(base.preview),
        search_bar: overrides.search_bar.unwrap_or(base.search_bar),
        status_bar: overrides.status_bar.unwrap_or(base.status_bar),
        keymap_bar: overrides.keymap_bar.unwrap_or(base.keymap_bar),
    }
}

fn merge_panel_sizes(base: &PanelSizes, overrides: Option<&PanelSizesOverrides>) -> PanelSizes {
    let Some(overrides) = overrides else {
        return base.clone();
    };
    PanelSizes {
        sidebar: overrides.sidebar.unwrap_or(base.sidebar),
        dir: overrides.dir.unwrap_or(base.dir),
        preview: overrides.preview.unwrap_or(base.preview),
    }
}

fn merge_miller(base: &MillerSettings, overrides: Option<&MillerSettingsOverrides>) -> MillerSettings {
    let Some(overrides) = overrides else {
        return base.clone();
    };
    MillerSettings {
        max_columns: overrides.max_columns.unwrap_or(base.max_columns).max(1),
    }
}

fn merge_search(base: &SearchSettings, overrides: Option<&SearchOverrides>) -> SearchSettings {
    let Some(overrides) = overrides else {
        return base.clone();
    };
    SearchSettings {
        ignored_dirs: overrides
            .ignored_dirs
            .clone()
            .unwrap_or_else(|| base.ignored_dirs.clone()),
        max_depth: overrides.max_depth.unwrap_or(base.max_depth).max(1),
    }
}

fn parse_color(name: &str) -> Color {
    match name.to_ascii_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "white" => Color::White,
        "darkgray" | "dark_gray" => Color::DarkGray,
        _ => Color::Reset,
    }
}

fn display_home_relative(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(stripped) = path.strip_prefix(&home)
    {
        if stripped.as_os_str().is_empty() {
            return "~".to_string();
        }
        return format!("~/{}", stripped.display());
    }
    path.display().to_string()
}

fn path_last_segment(path: &Path) -> String {
    path.file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn build_columns_from_path(path: &Path, settings: &EffectiveSettings) -> Vec<DirColumn> {
    vec![DirColumn::from_path(path.to_path_buf(), settings)]
}

fn read_dir_entries(path: &Path, settings: &EffectiveSettings) -> Vec<DirEntry> {
    let Ok(read_dir) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for dir_entry in read_dir.flatten() {
        let file_name = dir_entry.file_name().to_string_lossy().to_string();
        if !settings.show_hidden && file_name.starts_with('.') {
            continue;
        }
        let entry_path = dir_entry.path();
        let is_dir = entry_path.is_dir();
        entries.push(DirEntry {
            name: file_name,
            path: entry_path,
            is_dir,
        });
    }
    entries.sort_by(|a, b| sort_entries(a, b, settings.sort.as_str()));
    entries
}

fn sort_entries(a: &DirEntry, b: &DirEntry, sort_mode: &str) -> Ordering {
    let dir_first = b.is_dir.cmp(&a.is_dir);
    if dir_first != Ordering::Equal {
        return dir_first;
    }

    match sort_mode {
        "type" => extension_of(&a.name)
            .cmp(&extension_of(&b.name))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "size" => fs::metadata(&a.path)
            .ok()
            .map(|m| m.len())
            .unwrap_or(0)
            .cmp(&fs::metadata(&b.path).ok().map(|m| m.len()).unwrap_or(0))
            .reverse()
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "modified" => fs::metadata(&a.path)
            .and_then(|m| m.modified())
            .ok()
            .cmp(&fs::metadata(&b.path).and_then(|m| m.modified()).ok())
            .reverse()
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    }
}

fn extension_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_lowercase())
        .unwrap_or_default()
}

fn preview_for_path(path: &Path) -> PreviewData {
    let mut out = String::new();
    out.push_str(&format!("name: {}\n", path_last_segment(path)));
    out.push_str(&format!("path: {}\n", path.display()));
    out.push_str(&format!(
        "canonical_path: {}\n",
        fs::canonicalize(path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "-".to_string())
    ));

    let Ok(meta) = fs::metadata(path) else {
        out.push_str("metadata: unavailable\n");
        return PreviewData::Details(out);
    };
    let Ok(link_meta) = fs::symlink_metadata(path) else {
        out.push_str("symlink_metadata: unavailable\n");
        return PreviewData::Details(out);
    };

    let file_type = meta.file_type();
    let link_type = link_meta.file_type();
    out.push_str(&format!("is_dir: {}\n", file_type.is_dir()));
    out.push_str(&format!("is_file: {}\n", file_type.is_file()));
    out.push_str(&format!("is_symlink: {}\n", link_type.is_symlink()));
    out.push_str(&format!("size_bytes: {}\n", meta.len()));
    out.push_str(&format!("readonly: {}\n", meta.permissions().readonly()));
    out.push_str(&format!("created: {}\n", format_system_time(meta.created().ok())));
    out.push_str(&format!("modified: {}\n", format_system_time(meta.modified().ok())));
    out.push_str(&format!("accessed: {}\n", format_system_time(meta.accessed().ok())));

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
        out.push_str(&format!("dir_entries: {}\n", entries));
        out.push_str(&format!("dir_count: {}\n", dirs));
        out.push_str(&format!("file_count: {}\n", files));
    }

    PreviewData::Details(out)
}

fn scroll_start(total_rows: usize, viewport_rows: usize, anchor: usize) -> usize {
    if viewport_rows == 0 || total_rows <= viewport_rows {
        return 0;
    }
    let half = viewport_rows / 2;
    let centered = anchor.saturating_sub(half);
    centered.min(total_rows - viewport_rows)
}

fn format_system_time(time: Option<std::time::SystemTime>) -> String {
    match time.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) {
        Some(d) => format!("{}s_since_epoch", d.as_secs()),
        None => "-".to_string(),
    }
}

fn fuzzy_contains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut chars = needle.chars();
    let mut current = chars.next();
    for c in haystack.chars() {
        if let Some(n) = current
            && c == n
        {
            current = chars.next();
            if current.is_none() {
                return true;
            }
        }
    }
    false
}

fn global_search_paths(root: &Path, query: &str, max_depth: usize, ignored_dirs: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let q = query.to_ascii_lowercase();
    walk_collect(root, &q, 0, max_depth, ignored_dirs, &mut out);
    out
}

fn walk_collect(
    dir: &Path,
    query: &str,
    depth: usize,
    max_depth: usize,
    ignored_dirs: &[String],
    out: &mut Vec<PathBuf>,
) {
    if depth > max_depth {
        return;
    }
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if query.is_empty() || fuzzy_contains(&name.to_ascii_lowercase(), query) {
            out.push(path.clone());
        }
        if path.is_dir() {
            if ignored_dirs.iter().any(|x| x == &name) {
                continue;
            }
            walk_collect(&path, query, depth + 1, max_depth, ignored_dirs, out);
        }
    }
}

fn copy_path(source: &Path, destination: &Path) -> io::Result<()> {
    if source.is_dir() {
        copy_dir_recursive(source, destination)
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination).map(|_| ())
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let src = entry.path();
        let dst = destination.join(entry.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn move_path(source: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(source, destination) {
        Ok(_) => Ok(()),
        Err(_) => {
            copy_path(source, destination)?;
            if source.is_dir() {
                fs::remove_dir_all(source)
            } else {
                fs::remove_file(source)
            }
        }
    }
}

fn ensure_default_themes() -> io::Result<()> {
    let dir = themes_dir();
    fs::create_dir_all(&dir)?;

    let mut dark = dir.clone();
    dark.push("dark.toml");
    if !dark.exists() {
        fs::write(
            dark,
            r#"[ui_colors]
background = "black"
foreground = "white"
accent = "gray"
status_bar = "white"
keymap_bar = "gray"
search_bar = "white"
selection_bg = "white"
selection_fg = "black"
"#,
        )?;
    }

    let mut bark_red = dir;
    bark_red.push("bark-red.toml");
    if !bark_red.exists() {
        fs::write(
            bark_red,
            r#"[ui_colors]
background = "black"
foreground = "white"
accent = "red"
status_bar = "green"
keymap_bar = "yellow"
search_bar = "magenta"
selection_bg = "red"
selection_fg = "black"
"#,
        )?;
    }
    Ok(())
}

fn load_themes_registry() -> Result<ThemeRegistry> {
    let dir = themes_dir();
    let mut by_name = HashMap::new();
    if !dir.exists() {
        return Ok(ThemeRegistry { by_name });
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|x| x.to_str()) else {
            continue;
        };
        let raw = fs::read_to_string(&path)?;
        if let Ok(theme) = toml::from_str::<ThemeFile>(&raw) {
            by_name.insert(stem.to_string(), theme.ui_colors);
        }
    }
    Ok(ThemeRegistry { by_name })
}

fn write_default_config(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_config_contents())
}

fn default_config_contents() -> &'static str {
    r#"[defaults]
theme = "dark"
show_hidden = false
sort = "name"
start_dir = "~"
top_bar_height = 3
features = ["preview", "git_status"]

[defaults.panel_sizes]
sidebar = 1
dir = 4
preview = 1

[defaults.miller]
max_columns = 3

[defaults.search]
ignored_dirs = [".git", "node_modules"]
max_depth = 6

[defaults.panels]
sidebar = true
columns = true
preview = true
search_bar = true
status_bar = true
keymap_bar = true

[[bookmarks]]
name = "home"
path = "~"

[[profiles]]
name = "dev"
theme = "bark-red"
show_hidden = true
start_dir = "~/projects"
features = ["preview", "git_status", "thumbnails"]

[profiles.panels]
preview = false

[profiles.panel_sizes]
dir = 5

[[profiles]]
name = "clean"
features = ["preview"]

[profiles.panels]
sidebar = true
keymap_bar = false

[profiles.panel_sizes]
preview = 2
"#
}

fn normalize_start_dir(input: &str) -> PathBuf {
    let path = expand_tilde(input);
    if path.as_os_str().is_empty() {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    path
}

fn heal_value(current: &mut Value, defaults: &Value) -> bool {
    match defaults {
        Value::Table(default_table) => {
            if !matches!(current, Value::Table(_)) {
                *current = defaults.clone();
                return true;
            }
            let Some(current_table) = current.as_table_mut() else {
                *current = defaults.clone();
                return true;
            };
            let mut changed = false;
            for (key, default_value) in default_table {
                match current_table.get_mut(key) {
                    Some(existing) => {
                        if heal_value(existing, default_value) {
                            changed = true;
                        }
                    }
                    None => {
                        current_table.insert(key.clone(), default_value.clone());
                        changed = true;
                    }
                }
            }
            changed
        }
        Value::Array(default_arr) => {
            if !matches!(current, Value::Array(_)) {
                *current = defaults.clone();
                return true;
            }
            let Some(current_arr) = current.as_array_mut() else {
                *current = defaults.clone();
                return true;
            };
            if default_arr.is_empty() {
                return false;
            }
            let template = &default_arr[0];
            let mut changed = false;
            for item in current_arr {
                if heal_value(item, template) {
                    changed = true;
                }
            }
            changed
        }
        _ => {
            if same_toml_type(current, defaults) {
                false
            } else {
                *current = defaults.clone();
                true
            }
        }
    }
}

fn same_toml_type(a: &Value, b: &Value) -> bool {
    matches!(
        (a, b),
        (Value::String(_), Value::String(_))
            | (Value::Integer(_), Value::Integer(_))
            | (Value::Float(_), Value::Float(_))
            | (Value::Boolean(_), Value::Boolean(_))
            | (Value::Datetime(_), Value::Datetime(_))
            | (Value::Array(_), Value::Array(_))
            | (Value::Table(_), Value::Table(_))
    )
}

fn strip_color_blocks(root: &mut Value) -> bool {
    let Some(root_table) = root.as_table_mut() else {
        return false;
    };
    let mut changed = false;
    if let Some(defaults) = root_table.get_mut("defaults").and_then(Value::as_table_mut) {
        if defaults.remove("ui_colors").is_some() {
            changed = true;
        }
    }
    if let Some(profiles) = root_table.get_mut("profiles").and_then(Value::as_array_mut) {
        for profile in profiles {
            if let Some(profile_table) = profile.as_table_mut() {
                if profile_table.remove("ui_colors").is_some() {
                    changed = true;
                }
            }
        }
    }
    changed
}

fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(input)
}
