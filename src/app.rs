use std::{
    cmp::Ordering,
    collections::HashMap,
    collections::HashSet,
    fs,
    io,
    path::{Path, PathBuf},
    process::Command,
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
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use ratatui::layout::Alignment;
use serde::Deserialize;

pub fn run_app() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    // Ensure Ctrl+S/Ctrl+Q are delivered to the app instead of terminal flow control.
    let _ = Command::new("stty").arg("-ixon").status();

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
    #[serde(default)]
    sidebar: SidebarConfig,
    #[serde(default)]
    ui: UiConfig,
    #[serde(default)]
    navigation: NavigationConfig,
    #[serde(default)]
    features: FeaturesConfig,
    #[serde(default = "default_search")]
    search: SearchSettings,
    #[serde(default)]
    profiles: Vec<Profile>,
    #[serde(skip)]
    themes: ThemeRegistry,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SidebarConfig {
    #[serde(default)]
    bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct UiConfig {
    #[serde(default = "default_top_bar_height")]
    top_bar_height: u16,
    #[serde(default = "default_panels")]
    panels: Panels,
    #[serde(default = "default_panel_sizes")]
    panel_ratios: PanelSizes,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct NavigationConfig {
    #[serde(default = "default_start_dir")]
    start_dir: String,
    #[serde(default)]
    show_hidden: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_max_columns")]
    max_columns: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct FeaturesConfig {
    #[serde(default = "default_features")]
    enabled: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Profile {
    name: String,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    show_hidden: Option<bool>,
    #[serde(default)]
    start_dir: Option<String>,
    #[serde(default)]
    features: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
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
struct PanelSizes {
    #[serde(default = "default_size_sidebar")]
    sidebar: u16,
    #[serde(default = "default_size_dir")]
    dir: u16,
    #[serde(default = "default_size_preview")]
    preview: u16,
}

#[derive(Debug, Clone, Deserialize)]
struct SearchSettings {
    #[serde(default)]
    ignored_dirs: Vec<String>,
    #[serde(default = "default_search_max_depth")]
    max_depth: usize,
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
    #[serde(default = "default_color_selection_entry")]
    selection_entry: String,
    #[serde(default = "default_color_inactive_border")]
    inactive_border: String,
    #[serde(default = "default_color_dimmed_border")]
    dimmed_border: String,
    #[serde(default = "default_color_sidebar_header")]
    sidebar_header: String,
    #[serde(default = "default_color_sidebar_drive")]
    sidebar_drive: String,
    #[serde(default = "default_color_sidebar_recent")]
    sidebar_recent: String,
    #[serde(default = "default_color_column_header")]
    column_header: String,
    #[serde(default = "default_color_column_dir")]
    column_dir: String,
    #[serde(default = "default_color_column_file")]
    column_file: String,
    #[serde(default = "default_color_column_symlink")]
    column_symlink: String,
    #[serde(default = "default_color_column_exec")]
    column_exec: String,
    #[serde(default = "default_color_status_meta")]
    status_meta: String,
    #[serde(default = "default_color_status_profile")]
    status_profile: String,
    #[serde(default = "default_color_status_selection_mode")]
    status_selection_mode: String,
    #[serde(default = "default_color_keymap_label")]
    keymap_label: String,
    #[serde(default = "default_color_keymap_key")]
    keymap_key: String,
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

#[derive(Debug, Clone, Deserialize)]
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
    max_columns: usize,
    search: SearchSettings,
    panels: Panels,
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
        ensure_default_themes()?;
        let cfg = if config_path().exists() {
            toml::from_str::<AppConfig>(&fs::read_to_string(config_path())?)?
        } else {
            toml::from_str::<AppConfig>(default_config_contents())?
        };
        let mut config = cfg;
        config.themes = load_themes_registry()?;
        if config.profiles.is_empty() {
            config.profiles.push(Profile {
                name: "default".to_string(),
                theme: Some("dark".to_string()),
                show_hidden: Some(false),
                start_dir: Some("~/".to_string()),
                features: Some(vec!["preview".to_string(), "git_status".to_string()]),
            });
        }
        Ok(config)
    }

    fn effective_for_profile(&self, profile: Option<&Profile>) -> EffectiveSettings {
        let theme = profile
            .and_then(|p| p.theme.clone())
            .unwrap_or_else(|| "dark".to_string());
        let show_hidden = profile
            .and_then(|p| p.show_hidden)
            .unwrap_or(self.navigation.show_hidden);
        let start_dir = profile
            .and_then(|p| p.start_dir.clone())
            .unwrap_or_else(|| self.navigation.start_dir.clone());
        let features = profile
            .and_then(|p| p.features.clone())
            .unwrap_or_else(|| self.features.enabled.clone());
        let ui_colors = self
            .themes
            .by_name
            .get(&theme)
            .cloned()
            .unwrap_or_else(default_ui_colors);
        EffectiveSettings {
            theme,
            show_hidden,
            sort: self.navigation.sort.clone(),
            start_dir: normalize_start_dir(&start_dir),
            top_bar_height: self.ui.top_bar_height.max(1),
            features,
            ui_colors,
            panel_sizes: self.ui.panel_ratios.clone(),
            max_columns: self.navigation.max_columns.max(1),
            search: self.search.clone(),
            panels: self.ui.panels.clone(),
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
    selection_mode: bool,
    mode_selected_paths: HashSet<PathBuf>,
    mode_range_anchor: Option<usize>,
    mode_last_range: HashSet<PathBuf>,
    locked_column_path: Option<PathBuf>,
    simple_selected_paths: HashSet<PathBuf>,
    simple_range_anchor: Option<usize>,
    simple_last_range: HashSet<PathBuf>,
    simple_column_path: Option<PathBuf>,
    dialog: Option<DialogState>,
    keymap: KeymapConfig,
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
    paths: Vec<PathBuf>,
    cut: bool,
}

#[derive(Debug, Clone)]
enum DialogState {
    ConfirmDelete { paths: Vec<PathBuf>, yes_selected: bool },
}

#[derive(Debug, Clone, Deserialize)]
struct KeymapConfig {
    navigation: KeymapNavigation,
    selection: KeymapSelection,
    search: KeymapSearch,
    file_ops: KeymapFileOps,
    app: KeymapApp,
}

#[derive(Debug, Clone, Deserialize)]
struct KeymapNavigation {
    up: Vec<String>,
    down: Vec<String>,
    open: Vec<String>,
    parent: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct KeymapSelection {
    range_up: Vec<String>,
    range_down: Vec<String>,
    mode: String,
    toggle: String,
    exit: String,
}

#[derive(Debug, Clone, Deserialize)]
struct KeymapSearch {
    local: String,
    global: String,
}

#[derive(Debug, Clone, Deserialize)]
struct KeymapFileOps {
    new_file: String,
    new_dir: String,
    copy: String,
    cut: String,
    paste: String,
    trash: String,
}

#[derive(Debug, Clone, Deserialize)]
struct KeymapApp {
    quit: String,
    next_profile: String,
    prev_profile: String,
}

enum PreviewData {
    Empty,
    Details(Vec<Line<'static>>),
}

impl App {
    fn new(config: AppConfig) -> Result<Self> {
        let active_profile = 0;
        let effective = config.effective_for_profile(config.profiles.get(active_profile));
        let keymap = load_keymap_config();
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
            selection_mode: false,
            mode_selected_paths: HashSet::new(),
            mode_range_anchor: None,
            mode_last_range: HashSet::new(),
            locked_column_path: None,
            simple_selected_paths: HashSet::new(),
            simple_range_anchor: None,
            simple_last_range: HashSet::new(),
            simple_column_path: None,
            dialog: None,
            keymap,
        })
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.dialog.is_some() {
            self.handle_dialog_key(key);
            return;
        }
        if self.input_mode != InputMode::None {
            self.handle_input_mode(key);
            return;
        }
        if self.search_mode != SearchMode::None {
            self.handle_search_input(key);
            return;
        }
        if self.selection_mode {
            self.handle_selection_mode_key(key);
            return;
        }
        if key.code == KeyCode::Esc && self.selection_mode {
            self.selection_mode = false;
            self.mode_range_anchor = None;
            self.mode_last_range.clear();
            return;
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::Up {
            self.select_prev_range_simple();
            return;
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::Down {
            self.select_next_range_simple();
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
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.enter_selection_mode();
            }
            KeyCode::Char('n') if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                self.start_new_dir();
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => self.start_new_file(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.copy_selected(),
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => self.cut_selected(),
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => self.paste_clipboard(),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => self.request_delete_to_trash(),
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
        if self.dialog.is_some() {
            self.draw_dialog(frame);
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
                    Span::styled(
                        "DIRT // ",
                        Style::default()
                            .fg(parse_color(&self.effective.ui_colors.accent))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        profile_name.to_string(),
                        Style::default().fg(parse_color(&self.effective.ui_colors.foreground)),
                    ),
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
                SearchMode::None => "/ to search".to_string(),
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

        rows.push(ListItem::new(Line::from("Bookmarks").style(
            Style::default()
                .fg(parse_color(&self.effective.ui_colors.sidebar_header))
                .add_modifier(Modifier::BOLD),
        )));
        for b in &self.config.sidebar.bookmarks {
            rows.push(ListItem::new(Line::from(format!("  {} -> {}", b.name, b.path)).style(
                Style::default().fg(parse_color(&self.effective.ui_colors.foreground)),
            )));
        }

        rows.push(ListItem::new(""));
        rows.push(ListItem::new(Line::from("Drives").style(
            Style::default()
                .fg(parse_color(&self.effective.ui_colors.sidebar_header))
                .add_modifier(Modifier::BOLD),
        )));
        for drive in &self.drives {
            rows.push(ListItem::new(Line::from(format!("  {}", drive)).style(
                Style::default().fg(parse_color(&self.effective.ui_colors.sidebar_drive)),
            )));
        }

        rows.push(ListItem::new(""));
        rows.push(ListItem::new(Line::from("Recent Dirs").style(
            Style::default()
                .fg(parse_color(&self.effective.ui_colors.sidebar_header))
                .add_modifier(Modifier::BOLD),
        )));
        for recent in &self.recents {
            rows.push(ListItem::new(Line::from(format!("  {}", recent.display())).style(
                Style::default().fg(parse_color(&self.effective.ui_colors.sidebar_recent)),
            )));
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
        let max_columns = self.effective.max_columns.max(2);
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
            let is_locked_column = self
                .locked_column_path
                .as_ref()
                .map(|p| p == &col.path)
                .unwrap_or(true);
            let is_dimmed = self.selection_mode && !is_locked_column;
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
                let base_fg = if entry.is_dir {
                    parse_color(&self.effective.ui_colors.column_dir)
                } else if entry.is_symlink {
                    parse_color(&self.effective.ui_colors.column_symlink)
                } else if entry.is_executable {
                    parse_color(&self.effective.ui_colors.column_exec)
                } else {
                    parse_color(&self.effective.ui_colors.column_file)
                };
                let mut item = ListItem::new(format!(" {}{}", entry.name, kind))
                    .style(Style::default().fg(base_fg));
                let is_marked_selected = self.selection_set().contains(&entry.path);
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
                } else if is_marked_selected {
                    item = item.style(
                        Style::default()
                            .bg(parse_color(&self.effective.ui_colors.selection_entry))
                            .fg(parse_color(&self.effective.ui_colors.foreground)),
                    );
                } else if is_dimmed {
                    item = item.style(
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::DIM),
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
                    .title(Line::from(title).style(Style::default().fg(parse_color(
                        &self.effective.ui_colors.column_header,
                    ))))
                    .borders(Borders::ALL)
                    .style(
                        Style::default()
                            .fg(parse_color(if is_focused_column {
                                &self.effective.ui_colors.accent
                            } else if is_dimmed {
                                &self.effective.ui_colors.dimmed_border
                            } else {
                                &self.effective.ui_colors.inactive_border
                            }))
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
        self.simple_range_anchor = None;
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
        self.simple_range_anchor = None;
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
        self.clear_simple_selection();
    }

    fn go_parent(&mut self) {
        if self.columns.len() <= 1 {
            return;
        }
        self.columns.pop();
        if let Some(col) = self.columns.last() {
            self.current_dir = col.path.clone();
        }
        self.clear_simple_selection();
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
            PreviewData::Empty => vec![Line::from("No selection")],
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
        let sep = Span::styled(" · ", Style::default().fg(parse_color(&self.effective.ui_colors.status_meta)));
        let mut spans = vec![
            Span::styled(
                self.current_dir.display().to_string(),
                Style::default().fg(parse_color(&self.effective.ui_colors.status_bar)),
            ),
            sep.clone(),
            Span::styled(
                format!("files: {}", self.current_file_count()),
                Style::default().fg(parse_color(&self.effective.ui_colors.status_meta)),
            ),
            sep.clone(),
            Span::styled(
                format!("selected: {}", self.selected_name()),
                Style::default().fg(parse_color(&self.effective.ui_colors.status_meta)),
            ),
            sep.clone(),
            Span::styled(
                if self.selection_mode {
                    "SELECTION MODE".to_string()
                } else {
                    "select_mode: off".to_string()
                },
                Style::default().fg(parse_color(if self.selection_mode {
                    &self.effective.ui_colors.status_selection_mode
                } else {
                    &self.effective.ui_colors.status_meta
                })),
            ),
            sep.clone(),
            Span::styled(
                format!("git: {}", self.git_status_text()),
                Style::default().fg(parse_color(&self.effective.ui_colors.accent)),
            ),
            sep.clone(),
            Span::styled(
                format!("profile: {}", profile_name),
                Style::default().fg(parse_color(&self.effective.ui_colors.status_profile)),
            ),
            sep,
            Span::styled(
                if self.status_message.is_empty() {
                    "ready".to_string()
                } else {
                    self.status_message.clone()
                },
                Style::default().fg(parse_color(&self.effective.ui_colors.status_meta)),
            ),
        ];
        frame.render_widget(Paragraph::new(Line::from(std::mem::take(&mut spans))), area);
    }

    fn draw_keymap_bar(&self, frame: &mut Frame, area: Rect) {
        let key = |t: &str| Span::styled(t.to_string(), Style::default().fg(parse_color(&self.effective.ui_colors.keymap_key)));
        let label = |t: &str| Span::styled(t.to_string(), Style::default().fg(parse_color(&self.effective.ui_colors.keymap_label)));
        let sep = Span::styled(" · ".to_string(), Style::default().fg(parse_color(&self.effective.ui_colors.keymap_label)));
        let line = if self.selection_mode {
            Line::from(vec![
                key(&self.keymap.selection.toggle), label(" select/deselect"), sep.clone(),
                key(&self.keymap.selection.range_up.join("/")), label(" range"), sep.clone(),
                key(&self.keymap.selection.exit), label(" exit"), sep.clone(),
                key(&self.keymap.file_ops.copy), label(" copy"), sep.clone(),
                key(&self.keymap.file_ops.cut), label(" cut"), sep.clone(),
                key(&self.keymap.file_ops.paste), label(" paste"), sep.clone(),
                key(&self.keymap.file_ops.trash), label(" trash"),
            ])
        } else {
            Line::from(vec![
                key(&self.keymap.navigation.up.join("/")), label(" navigate"), sep.clone(),
                key(&self.keymap.navigation.down.join("/")), label(" navigate"), sep.clone(),
                key(&self.keymap.navigation.parent.join("/")), label(" navigate"), sep.clone(),
                key(&self.keymap.navigation.open.join("/")), label(" navigate"), sep.clone(),
                key(&self.keymap.selection.mode), label(" select mode"), sep.clone(),
                key(&self.keymap.app.quit), label(" quit"),
            ])
        };
        frame.render_widget(Paragraph::new(line), area);
    }

    fn draw_dialog(&self, frame: &mut Frame) {
        let Some(dialog) = &self.dialog else {
            return;
        };

        frame.render_widget(
            Block::default().style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Gray)
                    .add_modifier(Modifier::DIM),
            ),
            frame.area(),
        );

        let popup_area = centered_rect(50, 30, frame.area());
        frame.render_widget(Clear, popup_area);

        match dialog {
            DialogState::ConfirmDelete {
                paths,
                yes_selected,
            } => {
                let yes = if *yes_selected { "[ YES ]" } else { "  YES  " };
                let no = if !*yes_selected { "[ NO ]" } else { "  NO  " };
                let text = format!(
                    "Move {} item(s) to trash?\n\n{}    {}\n\nEnter confirm · Esc cancel",
                    paths.len(),
                    yes,
                    no
                );
                frame.render_widget(
                    Paragraph::new(text)
                        .alignment(Alignment::Center)
                        .block(
                            Block::default()
                                .title("Confirm Delete")
                                .borders(Borders::ALL)
                                .style(
                                    Style::default()
                                        .bg(parse_color(&self.effective.ui_colors.background))
                                        .fg(parse_color(&self.effective.ui_colors.foreground)),
                                ),
                        ),
                    popup_area,
                );
            }
        }
    }

    fn next_profile(&mut self) {
        if self.config.profiles.is_empty() {
            return;
        }
        self.active_profile = (self.active_profile + 1) % self.config.profiles.len();
        self.effective = self
            .config
            .effective_for_profile(self.config.profiles.get(self.active_profile));
        self.reinitialize_columns_preserve_current();
        self.clear_simple_selection();
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
            .effective_for_profile(self.config.profiles.get(self.active_profile));
        self.reinitialize_columns_preserve_current();
        self.clear_simple_selection();
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
                .map(|p| preview_for_path(p, &self.effective.ui_colors))
                .unwrap_or(PreviewData::Empty);
        }
        let Some(entry) = self.selected_entry() else {
            return PreviewData::Empty;
        };
        preview_for_path(&entry.path, &self.effective.ui_colors)
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
                    let cmd = self.search_query.trim();
                    if cmd == "config init" || cmd == "/config init" {
                        self.status_message = match init_config_file() {
                            Ok(msg) => msg,
                            Err(e) => format!("config init failed: {e}"),
                        };
                    } else if cmd == "keymap init" || cmd == "/keymap init" {
                        self.status_message = match init_keymap_file() {
                            Ok(msg) => msg,
                            Err(e) => format!("keymap init failed: {e}"),
                        };
                    }
                    self.search_mode = SearchMode::None;
                    self.search_query.clear();
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
        let paths = self.operation_paths();
        if paths.is_empty() {
            self.status_message = "nothing selected".to_string();
            return;
        }
        self.clipboard = Some(ClipboardItem { paths: paths.clone(), cut: false });
        self.status_message = format!("copied {} item(s)", paths.len());
    }

    fn cut_selected(&mut self) {
        let paths = self.operation_paths();
        if paths.is_empty() {
            self.status_message = "nothing selected".to_string();
            return;
        }
        self.clipboard = Some(ClipboardItem { paths: paths.clone(), cut: true });
        self.status_message = format!("cut {} item(s)", paths.len());
    }

    fn paste_clipboard(&mut self) {
        let Some(item) = self.clipboard.clone() else {
            self.status_message = "clipboard empty".to_string();
            return;
        };
        let mut ok_count = 0usize;
        let mut fail_count = 0usize;
        for source in &item.paths {
            let Some(name) = source.file_name() else {
                fail_count += 1;
                continue;
            };
            let mut destination = self.current_dir.join(name);
            if destination == *source {
                destination = self.current_dir.join(format!("{}_copy", name.to_string_lossy()));
            }

            let result = if item.cut {
                move_path(source, &destination)
            } else {
                copy_path(source, &destination)
            };
            if result.is_ok() {
                ok_count += 1;
            } else {
                fail_count += 1;
            }
        }
        if item.cut && fail_count == 0 {
            self.clipboard = None;
            self.clear_all_selection();
        }
        self.status_message = if fail_count == 0 {
            if item.cut {
                format!("moved {} item(s)", ok_count)
            } else {
                format!("copied {} item(s)", ok_count)
            }
        } else {
            format!("paste: {} ok, {} failed", ok_count, fail_count)
        };
        self.refresh_active_column();
    }

    fn delete_selected_to_trash(&mut self) {
        let paths = self.operation_paths();
        if paths.is_empty() {
            self.status_message = "nothing selected".to_string();
            return;
        };
        let mut ok_count = 0usize;
        let mut fail_count = 0usize;
        for path in &paths {
            match trash::delete(path) {
                Ok(_) => ok_count += 1,
                Err(_) => fail_count += 1,
            }
        }
        self.clear_all_selection();
        self.status_message = if fail_count == 0 {
            format!("trashed {} item(s)", ok_count)
        } else {
            format!("trash: {} ok, {} failed", ok_count, fail_count)
        };
        self.refresh_active_column();
    }

    fn request_delete_to_trash(&mut self) {
        let paths = self.operation_paths();
        if paths.is_empty() {
            self.status_message = "nothing selected".to_string();
            return;
        }
        self.dialog = Some(DialogState::ConfirmDelete {
            paths,
            yes_selected: true,
        });
    }

    fn handle_dialog_key(&mut self, key: crossterm::event::KeyEvent) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        match dialog {
            DialogState::ConfirmDelete { yes_selected, .. } => match key.code {
                KeyCode::Left | KeyCode::Char('h') => *yes_selected = true,
                KeyCode::Right | KeyCode::Char('l') => *yes_selected = false,
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    *yes_selected = true;
                    self.confirm_dialog();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.dialog = None;
                    self.status_message = "delete canceled".to_string();
                }
                KeyCode::Enter => self.confirm_dialog(),
                _ => {}
            },
        }
    }

    fn confirm_dialog(&mut self) {
        let Some(DialogState::ConfirmDelete {
            paths,
            yes_selected,
        }) = self.dialog.take()
        else {
            return;
        };
        if !yes_selected {
            self.status_message = "delete canceled".to_string();
            return;
        }
        let mut ok_count = 0usize;
        let mut fail_count = 0usize;
        for path in &paths {
            match trash::delete(path) {
                Ok(_) => ok_count += 1,
                Err(_) => fail_count += 1,
            }
        }
        self.clear_all_selection();
        self.status_message = if fail_count == 0 {
            format!("trashed {} item(s)", ok_count)
        } else {
            format!("trash: {} ok, {} failed", ok_count, fail_count)
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

    fn toggle_focused_selection(&mut self) {
        let Some(path) = self.selected_entry_path() else {
            return;
        };
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        if !self.mode_selected_paths.insert(path.clone()) {
            self.mode_selected_paths.remove(&path);
        }
    }

    fn select_prev_range_simple(&mut self) {
        self.select_range_step_simple(-1);
    }

    fn select_next_range_simple(&mut self) {
        self.select_range_step_simple(1);
    }

    fn select_range_step_simple(&mut self, delta: isize) {
        let Some(current_path) = self.columns.last().map(|c| c.path.clone()) else {
            return;
        };
        self.ensure_simple_column(current_path);
        let Some(current) = self.columns.last_mut() else {
            return;
        };
        if current.entries.is_empty() {
            return;
        }
        let previous = current.selected;
        if delta < 0 {
            if current.selected == 0 {
                return;
            }
            current.selected -= 1;
        } else if current.selected + 1 < current.entries.len() {
            current.selected += 1;
        } else {
            return;
        }
        let anchor = self.simple_range_anchor.unwrap_or(previous);
        self.simple_range_anchor = Some(anchor);
        let lo = anchor.min(current.selected);
        let hi = anchor.max(current.selected);
        let new_range: HashSet<PathBuf> = (lo..=hi)
            .map(|idx| current.entries[idx].path.clone())
            .collect();
        for p in new_range.symmetric_difference(&self.simple_last_range) {
            if self.simple_selected_paths.contains(p) {
                self.simple_selected_paths.remove(p);
            } else {
                self.simple_selected_paths.insert(p.clone());
            }
        }
        self.simple_last_range = new_range;
    }

    fn operation_paths(&self) -> Vec<PathBuf> {
        let set = self.selection_set();
        if !set.is_empty() {
            return set.iter().cloned().collect();
        }
        self.selected_entry_path().into_iter().collect()
    }

    fn selection_set(&self) -> &HashSet<PathBuf> {
        if self.selection_mode {
            &self.mode_selected_paths
        } else {
            &self.simple_selected_paths
        }
    }

    fn clear_simple_selection(&mut self) {
        self.simple_selected_paths.clear();
        self.simple_range_anchor = None;
        self.simple_last_range.clear();
        self.simple_column_path = None;
    }

    fn clear_all_selection(&mut self) {
        self.clear_simple_selection();
        self.mode_selected_paths.clear();
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.locked_column_path = None;
        self.selection_mode = false;
    }

    fn ensure_simple_column(&mut self, path: PathBuf) {
        if self.simple_column_path.as_ref() != Some(&path) {
            self.simple_selected_paths.clear();
            self.simple_range_anchor = None;
            self.simple_last_range.clear();
            self.simple_column_path = Some(path);
        }
    }

    fn enter_selection_mode(&mut self) {
        if self.selection_mode {
            return;
        }
        self.selection_mode = true;
        self.mode_selected_paths.clear();
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.locked_column_path = self.columns.last().map(|c| c.path.clone());
        self.clear_simple_selection();
    }

    fn exit_selection_mode(&mut self) {
        self.selection_mode = false;
        self.mode_selected_paths.clear();
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.locked_column_path = None;
    }

    fn handle_selection_mode_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.exit_selection_mode(),
            KeyCode::Char(' ') if key.modifiers.is_empty() => self.toggle_focused_selection(),
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => self.select_prev_range_mode(),
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => self.select_next_range_mode(),
            KeyCode::Up | KeyCode::Char('k') => self.select_prev_mode(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_mode(),
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {}
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.copy_selected(),
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => self.cut_selected(),
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => self.paste_clipboard(),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => self.request_delete_to_trash(),
            KeyCode::Char('n') if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                self.start_new_dir()
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => self.start_new_file(),
            _ => {}
        }
    }

    fn select_prev_mode(&mut self) {
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.select_prev();
    }

    fn select_next_mode(&mut self) {
        self.mode_range_anchor = None;
        self.mode_last_range.clear();
        self.select_next();
    }

    fn select_prev_range_mode(&mut self) {
        self.select_range_step_mode(-1);
    }

    fn select_next_range_mode(&mut self) {
        self.select_range_step_mode(1);
    }

    fn select_range_step_mode(&mut self, delta: isize) {
        let Some(current) = self.columns.last_mut() else {
            return;
        };
        if current.entries.is_empty() {
            return;
        }
        let previous = current.selected;
        if delta < 0 {
            if current.selected == 0 {
                return;
            }
            current.selected -= 1;
        } else if current.selected + 1 < current.entries.len() {
            current.selected += 1;
        } else {
            return;
        }
        let anchor = self.mode_range_anchor.unwrap_or(previous);
        self.mode_range_anchor = Some(anchor);
        let lo = anchor.min(current.selected);
        let hi = anchor.max(current.selected);
        let new_range: HashSet<PathBuf> = (lo..=hi)
            .map(|idx| current.entries[idx].path.clone())
            .collect();
        for p in new_range.symmetric_difference(&self.mode_last_range) {
            if self.mode_selected_paths.contains(p) {
                self.mode_selected_paths.remove(p);
            } else {
                self.mode_selected_paths.insert(p.clone());
            }
        }
        self.mode_last_range = new_range;
    }
}

#[derive(Debug, Clone)]
struct DirEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    is_symlink: bool,
    is_executable: bool,
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

fn keymap_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("dirt");
    path.push("keymap.toml");
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
    #[cfg(target_os = "linux")]
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

    #[cfg(target_os = "macos")]
    {
        let mut mounts = vec!["/".to_string(), "/Volumes".to_string()];
        mounts.sort();
        mounts.dedup();
        mounts
    }

    #[cfg(target_os = "windows")]
    {
        vec!["C:\\".to_string()]
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
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
        selection_entry: default_color_selection_entry(),
        inactive_border: default_color_inactive_border(),
        dimmed_border: default_color_dimmed_border(),
        sidebar_header: default_color_sidebar_header(),
        sidebar_drive: default_color_sidebar_drive(),
        sidebar_recent: default_color_sidebar_recent(),
        column_header: default_color_column_header(),
        column_dir: default_color_column_dir(),
        column_file: default_color_column_file(),
        column_symlink: default_color_column_symlink(),
        column_exec: default_color_column_exec(),
        status_meta: default_color_status_meta(),
        status_profile: default_color_status_profile(),
        status_selection_mode: default_color_status_selection_mode(),
        keymap_label: default_color_keymap_label(),
        keymap_key: default_color_keymap_key(),
    }
}

fn default_panel_sizes() -> PanelSizes {
    PanelSizes {
        sidebar: default_size_sidebar(),
        dir: default_size_dir(),
        preview: default_size_preview(),
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

fn default_start_dir() -> String {
    "~/".to_string()
}

fn default_sort() -> String {
    "name".to_string()
}

fn default_features() -> Vec<String> {
    vec!["preview".to_string(), "git_status".to_string()]
}

fn default_max_columns() -> usize {
    4
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

fn default_color_selection_entry() -> String {
    "darkgray".to_string()
}

fn default_color_inactive_border() -> String {
    "#2A2A2A".to_string()
}

fn default_color_dimmed_border() -> String {
    "#1C1C1C".to_string()
}

fn default_color_sidebar_header() -> String {
    default_color_accent()
}

fn default_color_sidebar_drive() -> String {
    default_color_selection_entry()
}

fn default_color_sidebar_recent() -> String {
    default_color_selection_entry()
}

fn default_color_column_header() -> String {
    default_color_selection_entry()
}

fn default_color_column_dir() -> String {
    default_color_accent()
}

fn default_color_column_file() -> String {
    default_color_fg()
}

fn default_color_column_symlink() -> String {
    "#94E0E0".to_string()
}

fn default_color_column_exec() -> String {
    "#84E052".to_string()
}

fn default_color_status_meta() -> String {
    default_color_selection_entry()
}

fn default_color_status_profile() -> String {
    default_color_selection_entry()
}

fn default_color_status_selection_mode() -> String {
    "#EB8D47".to_string()
}

fn default_color_keymap_label() -> String {
    default_color_selection_entry()
}

fn default_color_keymap_key() -> String {
    default_color_accent()
}

fn parse_color(name: &str) -> Color {
    if let Some(hex) = name.strip_prefix('#')
        && hex.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        )
    {
        return Color::Rgb(r, g, b);
    }
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
        let is_symlink = fs::symlink_metadata(&entry_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let is_executable = fs::metadata(&entry_path)
            .map(|m| {
                use std::os::unix::fs::PermissionsExt;
                m.permissions().mode() & 0o111 != 0
            })
            .unwrap_or(false);
        #[cfg(target_os = "windows")]
        let is_executable = entry_path
            .extension()
            .and_then(|x| x.to_str())
            .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "exe" | "bat" | "cmd" | "com"))
            .unwrap_or(false);
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        let is_executable = false;
        entries.push(DirEntry {
            name: file_name,
            path: entry_path,
            is_dir,
            is_symlink,
            is_executable,
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

fn preview_for_path(path: &Path, colors: &UiColors) -> PreviewData {
    let label = |k: &str| Span::styled(format!("{k}: "), Style::default().fg(parse_color(&colors.status_meta)));
    let val = |v: String| Span::styled(v, Style::default().fg(parse_color(&colors.foreground)));
    let bool_span = |b: bool| {
        if b {
            Span::styled("true", Style::default().fg(parse_color("#84E052")))
        } else {
            Span::styled("false", Style::default().fg(parse_color(&colors.status_meta)))
        }
    };
    let mut lines = Vec::new();
    lines.push(Line::from(vec![label("name"), val(path_last_segment(path))]));
    lines.push(Line::from(vec![label("path"), val(path.display().to_string())]));
    lines.push(Line::from(vec![
        label("canonical_path"),
        val(
            fs::canonicalize(path)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "-".to_string()),
        ),
    ]));

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
    lines.push(Line::from(vec![label("is_dir"), bool_span(file_type.is_dir())]));
    lines.push(Line::from(vec![label("is_file"), bool_span(file_type.is_file())]));
    lines.push(Line::from(vec![label("is_symlink"), bool_span(link_type.is_symlink())]));
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

fn scroll_start(total_rows: usize, viewport_rows: usize, anchor: usize) -> usize {
    if viewport_rows == 0 || total_rows <= viewport_rows {
        return 0;
    }
    let half = viewport_rows / 2;
    let centered = anchor.saturating_sub(half);
    centered.min(total_rows - viewport_rows)
}

fn format_system_time(time: Option<std::time::SystemTime>) -> String {
    match time {
        Some(t) => {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M").to_string()
        }
        None => "-".to_string(),
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut idx = 0usize;
    while size >= 1024.0 && idx < UNITS.len() - 1 {
        size /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{} {}", bytes, UNITS[idx])
    } else {
        format!("{size:.1} {}", UNITS[idx])
    }
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
            r##"[vars]
primary_bg   = "#0D0D0D"
secondary_bg = "#1A1A1A"
accent       = "#13B5B7"
active       = "#FCFCFC"
primary_fg   = "#E0E0E0"
secondary_fg = "#888888"

[borders]
active   = "$accent"
inactive = "#2A2A2A"
dimmed   = "#1C1C1C"

[sidebar]
background     = "$secondary_bg"
section_header = "$accent"
bookmark_fg    = "$primary_fg"
drive_fg       = "$secondary_fg"
recent_fg      = "$secondary_fg"
selected_bg    = "#2A2A2A"
selected_fg    = "$active"

[columns]
background    = "$primary_bg"
column_header = "$secondary_fg"
focused_bg    = "#141414"
dimmed_fg     = "#333333"
[columns.file]
default    = "$primary_fg"
dir        = "$accent"
symlink    = "#94E0E0"
executable = "#84E052"
[columns.selected]
bg = "#1F1F1F"
fg = "$active"

[status_bar]
path_fg           = "$active"
meta_fg           = "$secondary_fg"
git_fg            = "$accent"
profile_fg        = "$secondary_fg"
selection_mode_fg = "#EB8D47"

[keymap_bar]
key_fg     = "$accent"
label_fg   = "$secondary_fg"

[search_bar]
text_fg     = "$primary_fg"

[selection]
bg       = "#1F3A3A"
fg       = "$active"
"##,
        )?;
    }

    let mut bark_red = dir;
    bark_red.push("bark-red.toml");
    if !bark_red.exists() {
        fs::write(
            bark_red,
            r##"[ui_colors]
background = "black"
foreground = "white"
accent = "red"
status_bar = "green"
keymap_bar = "yellow"
search_bar = "magenta"
selection_bg = "red"
selection_fg = "black"
selection_entry = "darkgray"
"##,
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
        if let Ok(value) = toml::from_str::<toml::Value>(&raw) {
            by_name.insert(stem.to_string(), ui_colors_from_theme_value(&value));
        }
    }
    Ok(ThemeRegistry { by_name })
}

fn ui_colors_from_theme_value(value: &toml::Value) -> UiColors {
    let mut out = default_ui_colors();
    let vars = theme_vars(value);
    out.background = theme_color(value, &vars, "vars.primary_bg", &out.background);
    out.foreground = theme_color(value, &vars, "vars.primary_fg", &out.foreground);
    out.accent = theme_color(value, &vars, "borders.active", &out.accent);
    out.inactive_border = theme_color(value, &vars, "borders.inactive", &out.inactive_border);
    out.dimmed_border = theme_color(value, &vars, "borders.dimmed", &out.dimmed_border);
    out.search_bar = theme_color(value, &vars, "search_bar.text_fg", &out.search_bar);
    out.status_bar = theme_color(value, &vars, "status_bar.path_fg", &out.status_bar);
    out.keymap_bar = theme_color(value, &vars, "keymap_bar.key_fg", &out.keymap_bar);
    out.selection_bg = theme_color(value, &vars, "columns.selected.bg", &out.selection_bg);
    out.selection_fg = theme_color(value, &vars, "columns.selected.fg", &out.selection_fg);
    out.selection_entry = theme_color(value, &vars, "selection.bg", &out.selection_entry);
    out.sidebar_header = theme_color(value, &vars, "sidebar.section_header", &out.sidebar_header);
    out.sidebar_drive = theme_color(value, &vars, "sidebar.drive_fg", &out.sidebar_drive);
    out.sidebar_recent = theme_color(value, &vars, "sidebar.recent_fg", &out.sidebar_recent);
    out.column_header = theme_color(value, &vars, "columns.column_header", &out.column_header);
    out.column_dir = theme_color(value, &vars, "columns.file.dir", &out.column_dir);
    out.column_file = theme_color(value, &vars, "columns.file.default", &out.column_file);
    out.column_symlink = theme_color(value, &vars, "columns.file.symlink", &out.column_symlink);
    out.column_exec = theme_color(value, &vars, "columns.file.executable", &out.column_exec);
    out.status_meta = theme_color(value, &vars, "status_bar.meta_fg", &out.status_meta);
    out.status_profile = theme_color(value, &vars, "status_bar.profile_fg", &out.status_profile);
    out.status_selection_mode = theme_color(
        value,
        &vars,
        "status_bar.selection_mode_fg",
        &out.status_selection_mode,
    );
    out.keymap_key = theme_color(value, &vars, "keymap_bar.key_fg", &out.keymap_key);
    out.keymap_label = theme_color(value, &vars, "keymap_bar.label_fg", &out.keymap_label);
    out
}

fn theme_vars(value: &toml::Value) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    if let Some(tbl) = value.get("vars").and_then(|v| v.as_table()) {
        for (k, v) in tbl {
            if let Some(s) = v.as_str() {
                vars.insert(k.clone(), s.to_string());
            }
        }
    }
    vars
}

fn theme_color(value: &toml::Value, vars: &HashMap<String, String>, path: &str, fallback: &str) -> String {
    let Some(raw) = theme_lookup(value, path).and_then(|v| v.as_str()) else {
        return fallback.to_string();
    };
    if let Some(var_name) = raw.strip_prefix('$') {
        if let Some(v) = vars.get(var_name) {
            return v.clone();
        }
        eprintln!("warning: missing theme var ${var_name}, using fallback {fallback}");
        return fallback.to_string();
    }
    raw.to_string()
}

fn theme_lookup<'a>(value: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    let mut cur = value;
    for seg in path.split('.') {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

fn write_default_config(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_config_contents())
}

fn init_config_file() -> io::Result<String> {
    let path = config_path();
    if path.exists() {
        return Ok("Config already exists at ~/.config/dirt/dirt.toml".to_string());
    }
    write_default_config(&path)?;
    Ok("Config created at ~/.config/dirt/dirt.toml".to_string())
}

fn init_keymap_file() -> io::Result<String> {
    let path = keymap_path();
    if path.exists() {
        return Ok(format!("Keymap already exists at {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, default_keymap_contents())?;
    Ok(format!("Keymap created at {}", path.display()))
}

fn load_keymap_config() -> KeymapConfig {
    let path = keymap_path();
    if path.exists()
        && let Ok(raw) = fs::read_to_string(path)
        && let Ok(k) = toml::from_str::<KeymapConfig>(&raw)
    {
        return k;
    }
    toml::from_str(default_keymap_contents()).unwrap_or_else(|_| KeymapConfig {
        navigation: KeymapNavigation {
            up: vec!["Up".to_string(), "k".to_string()],
            down: vec!["Down".to_string(), "j".to_string()],
            open: vec!["Right".to_string(), "l".to_string(), "Enter".to_string()],
            parent: vec!["Left".to_string(), "h".to_string(), "Backspace".to_string()],
        },
        selection: KeymapSelection {
            range_up: vec!["Shift+Up".to_string()],
            range_down: vec!["Shift+Down".to_string()],
            mode: "Ctrl+S".to_string(),
            toggle: "Space".to_string(),
            exit: "Esc".to_string(),
        },
        search: KeymapSearch {
            local: "/".to_string(),
            global: "Ctrl+F".to_string(),
        },
        file_ops: KeymapFileOps {
            new_file: "Ctrl+N".to_string(),
            new_dir: "Ctrl+Shift+N".to_string(),
            copy: "Ctrl+C".to_string(),
            cut: "Ctrl+X".to_string(),
            paste: "Ctrl+V".to_string(),
            trash: "Ctrl+D".to_string(),
        },
        app: KeymapApp {
            quit: "q".to_string(),
            next_profile: "p".to_string(),
            prev_profile: "P".to_string(),
        },
    })
}

fn default_config_contents() -> &'static str {
    r#"# DIRT config
# Location: ~/.config/dirt/dirt.toml
# Notes:
# - Theme is profile-only (`[[profiles]].theme`)
# - Paths support `~` and `~/...`
# - This file is intended to be user-edited

[sidebar]
# Sidebar bookmarks shown in the left panel
bookmarks = [{ name = "home", path = "~" }]

[ui]
# Top bar height in terminal rows
top_bar_height = 3

[ui.panels]
# Toggle panel visibility
sidebar = true
preview = true
keymap_bar = true
search_bar = true
status_bar = true
columns = true

[ui.panel_ratios]
# Width ratios (relative weights)
# Example: 1/4/2 means sidebar=1 part, dir=4 parts, preview=2 parts
sidebar = 1
dir = 4
preview = 2

[navigation]
# Base navigation defaults used unless profile overrides
start_dir = "~/"
show_hidden = false
sort = "name"
max_columns = 4

[features]
# Feature flags (string identifiers)
enabled = ["preview", "git_status"]

[search]
# Search behavior (`/` local, Ctrl+F global)
ignored_dirs = [".git", "node_modules"]
max_depth = 6

[[profiles]]
# Profile shown in status bar and switchable in-app
name = "default"
theme = "dark"
start_dir = "~/"
show_hidden = false
features = ["preview", "git_status"]

[[profiles]]
name = "dev"
theme = "dark"
start_dir = "~/"
show_hidden = true
features = ["preview", "git_status", "thumbnails"]

[[profiles]]
name = "clean"
theme = "dark"
start_dir = "~/"
show_hidden = false
features = ["preview"]
"#
}

fn default_keymap_contents() -> &'static str {
    r#"[navigation]
up     = ["Up", "k"]
down   = ["Down", "j"]
open   = ["Right", "l", "Enter"]
parent = ["Left", "h", "Backspace"]

[selection]
range_up   = ["Shift+Up"]
range_down = ["Shift+Down"]
mode   = "Ctrl+S"
toggle = "Space"
exit   = "Esc"

[search]
local  = "/"
global = "Ctrl+F"

[file_ops]
new_file = "Ctrl+N"
new_dir  = "Ctrl+Shift+N"
copy     = "Ctrl+C"
cut      = "Ctrl+X"
paste    = "Ctrl+V"
trash    = "Ctrl+D"

[app]
quit         = "q"
next_profile = "p"
prev_profile = "P"
"#
}

fn normalize_start_dir(input: &str) -> PathBuf {
    let path = expand_tilde(input);
    if path.as_os_str().is_empty() {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    path
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
