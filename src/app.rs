use std::{
    collections::HashMap,
    collections::HashSet,
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    time::{Duration, Instant},
};

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use git2::{Repository, StatusOptions};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use serde::Deserialize;
use crate::theme::{ColumnTheme, Theme};
use crate::ui::preview::{self, ImageProtocol, PreviewImageConfig};
use crate::ui::overlay::{self, OverlayDialog, OverlayDialogKind, OverlayFrame};
mod features;
use self::features::preview_feature;
mod config;
mod fs_ops;
mod panels;
mod modes;
use self::config::{
    default_config_contents, discover_drives, ensure_local_defaults_files, init_config_file,
    init_keymap_file, init_layout_file, init_theme_file, load_default_theme_fallback,
    load_keymap_config, load_recents, load_themes_registry, save_recents,
};
use self::fs_ops::{build_columns_from_path, copy_path, move_path, read_dir_entries};

static PERF_FRAMES: AtomicU64 = AtomicU64::new(0);
static PERF_TOTAL_US: AtomicU64 = AtomicU64::new(0);
static PERF_MAIN_US: AtomicU64 = AtomicU64::new(0);
static PERF_TOP_US: AtomicU64 = AtomicU64::new(0);
static PERF_STATUS_US: AtomicU64 = AtomicU64::new(0);

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
    miller: MillerConfig,
    #[serde(default)]
    preview: PreviewConfig,
    #[serde(default)]
    profiles: Vec<Profile>,
    #[serde(skip)]
    themes: ThemeRegistry,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SidebarConfig {
    #[serde(default)]
    bookmarks: Vec<Bookmark>,
    #[serde(default = "default_recent_dirs_limit")]
    recent_dirs_limit: usize,
    #[serde(default = "default_drives_limit")]
    drives_limit: usize,
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
    #[serde(default)]
    sudo_mode: bool,
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
struct MillerConfig {
    #[serde(default = "default_max_columns")]
    column_count: usize,
    #[serde(default = "default_active_column")]
    active_column: usize,
    #[serde(default = "default_column_ratios")]
    column_ratios: Vec<u16>,
}

impl Default for MillerConfig {
    fn default() -> Self {
        Self {
            column_count: default_max_columns(),
            active_column: default_active_column(),
            column_ratios: default_column_ratios(),
        }
    }
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
    #[serde(default)]
    slot: Option<u8>,
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
    theme_colors: Theme,
    panel_sizes: PanelSizes,
    max_columns: usize,
    active_column: usize,
    column_ratios: Vec<u16>,
    search: SearchSettings,
    panels: Panels,
    preview: PreviewConfig,
}

#[derive(Debug, Clone, Copy)]
struct RuntimePlan {
    panels: EnabledPanels,
    features: EnabledFeatures,
}

#[derive(Debug, Clone, Copy)]
struct EnabledPanels {
    search_bar: bool,
    sidebar: bool,
    columns: bool,
    preview: bool,
    status_bar: bool,
    keymap_bar: bool,
}

#[derive(Debug, Clone, Copy)]
struct EnabledFeatures {
    preview_images: bool,
}

impl RuntimePlan {
    fn from_effective(effective: &EffectiveSettings) -> Self {
        let has_feature = |name: &str| effective.features.iter().any(|f| f == name);
        let panels = EnabledPanels {
            search_bar: effective.panels.search_bar,
            sidebar: effective.panels.sidebar,
            columns: effective.panels.columns,
            preview: effective.panels.preview,
            status_bar: effective.panels.status_bar,
            keymap_bar: effective.panels.keymap_bar,
        };
        let features = EnabledFeatures {
            preview_images: panels.preview && effective.preview.images && has_feature("preview"),
        };
        Self { panels, features }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PreviewConfig {
    #[serde(default = "default_true")]
    images: bool,
    #[serde(default = "default_max_image_size_mb")]
    max_image_size_mb: u64,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            images: true,
            max_image_size_mb: default_max_image_size_mb(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ThemeRegistry {
    by_name: HashMap<String, Theme>,
}

impl AppConfig {
    fn load() -> Result<Self> {
        ensure_local_defaults_files()?;
        let cfg = if let Ok(raw) = fs::read_to_string("defaults/layout.toml") {
            toml::from_str::<AppConfig>(&raw)
                .unwrap_or_else(|_| toml::from_str::<AppConfig>(default_config_contents()).unwrap())
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
        let theme_colors = self
            .themes
            .by_name
            .get(&theme)
            .cloned()
            .unwrap_or_else(load_default_theme_fallback);
        let max_columns = self.miller.column_count.max(2);
        let active_column = self.miller.active_column.min(max_columns.saturating_sub(2));
        let mut column_ratios = self.miller.column_ratios.clone();
        if column_ratios.is_empty() {
            column_ratios = vec![1; max_columns];
        }
        if column_ratios.len() < max_columns {
            column_ratios.resize(max_columns, 1);
        } else if column_ratios.len() > max_columns {
            column_ratios.truncate(max_columns);
        }
        EffectiveSettings {
            theme,
            show_hidden,
            sort: self.navigation.sort.clone(),
            start_dir: normalize_start_dir(&start_dir),
            top_bar_height: self.ui.top_bar_height.max(1),
            features,
            theme_colors,
            panel_sizes: self.ui.panel_ratios.clone(),
            max_columns,
            active_column,
            column_ratios,
            search: self.search.clone(),
            panels: self.ui.panels.clone(),
            preview: self.preview.clone(),
        }
    }
}

struct App {
    config: AppConfig,
    active_profile: usize,
    effective: EffectiveSettings,
    runtime_plan: RuntimePlan,
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
    rename_target: Option<PathBuf>,
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
    awaiting_bookmark_slot: bool,
    user_name: String,
    device_name: String,
    image_protocol: Option<ImageProtocol>,
    sudo_mode: bool,
    sudo_password_prompt: bool,
    sudo_password_input: String,
    keybinds_overlay: Option<OverlayFrame>,
    keybinds_overlay_cache: Option<OverlayFrame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    None,
    Local,
    Global,
}

#[derive(Debug, Clone, Copy)]
struct UiModeState {
    sudo: bool,
    select: bool,
    bookmark: bool,
    search: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    None,
    NewFile,
    NewDir,
    Rename,
}

#[derive(Debug, Clone)]
struct ClipboardItem {
    paths: Vec<PathBuf>,
    cut: bool,
}

#[derive(Debug, Clone)]
enum DialogState {
    ConfirmDelete { paths: Vec<PathBuf>, yes_selected: bool },
    ConfirmBookmarkOverwrite {
        slot: char,
        existing_path: String,
        new_path: String,
        yes_selected: bool,
    },
    Message { title: String, text: String },
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

pub(crate) enum PreviewData {
    Empty,
    Details(Vec<Line<'static>>),
    UnsupportedImageMascot { filename: String, size: String },
}

impl App {
    fn new(config: AppConfig) -> Result<Self> {
        let active_profile = 0;
        let effective = config.effective_for_profile(config.profiles.get(active_profile));
        let runtime_plan = RuntimePlan::from_effective(&effective);
        let keymap = load_keymap_config();
        let recents = if runtime_plan.panels.sidebar {
            load_recents()?
        } else {
            Vec::new()
        };
        let drives = if runtime_plan.panels.sidebar {
            discover_drives()
        } else {
            Vec::new()
        };
        let (user_name, device_name) = user_device_names();
        let image_protocol = if runtime_plan.features.preview_images {
            preview::detect_image_protocol()
        } else {
            None
        };
        let current_dir = effective.start_dir.clone();
        let columns = build_columns_from_path(&current_dir, &effective, false, None);
        Ok(Self {
            config,
            active_profile,
            effective,
            runtime_plan,
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
            rename_target: None,
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
            awaiting_bookmark_slot: false,
            user_name,
            device_name,
            image_protocol,
            sudo_mode: false,
            sudo_password_prompt: false,
            sudo_password_input: String::new(),
            keybinds_overlay: None,
            keybinds_overlay_cache: None,
        })
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.keybinds_overlay.is_some() {
            self.handle_keybinds_overlay_key(key);
            return;
        }
        if self.dialog.is_some() {
            self.handle_dialog_key(key);
            return;
        }
        if self.input_mode != InputMode::None {
            self.handle_input_mode(key);
            return;
        }
        if self.sudo_password_prompt {
            self.handle_sudo_password_input(key);
            return;
        }
        if self.search_mode != SearchMode::None {
            self.handle_search_input(key);
            return;
        }
        if key.code == KeyCode::Esc && self.awaiting_bookmark_slot {
            self.awaiting_bookmark_slot = false;
            self.status_message = "bookmark mode off".to_string();
            return;
        }
        if self.awaiting_bookmark_slot
            && let Some(slot) = bookmark_slot_from_key(key)
        {
            self.set_bookmark_slot(slot);
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
            KeyCode::Up => self.select_prev(),
            KeyCode::Down => self.select_next(),
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
            KeyCode::F(2) => self.start_rename(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.copy_selected(),
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => self.cut_selected(),
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => self.paste_clipboard(),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => self.request_delete_to_trash(),
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.awaiting_bookmark_slot = true;
                self.status_message = "bookmark set mode: press 1-9".to_string();
            }
            KeyCode::Char(c)
                if key.modifiers.contains(KeyModifiers::CONTROL) && ('1'..='9').contains(&c) =>
            {
                self.open_bookmark_slot(c);
            }
            KeyCode::Char(c) if key.modifiers.is_empty() && ('1'..='9').contains(&c) => {
                self.open_bookmark_slot(c);
            }
            _ => {}
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let t0 = Instant::now();
        let base_style = Style::default()
            .bg(parse_color(&self.effective.theme_colors.vars.primary_bg))
            .fg(parse_color(&self.effective.theme_colors.vars.primary_fg));
        frame.render_widget(Paragraph::new("").style(base_style), frame.area());

        let mut rows = Vec::new();
        if self.runtime_plan.panels.search_bar {
            rows.push(Constraint::Length(self.effective.top_bar_height));
        }
        rows.push(Constraint::Min(1));
        if self.runtime_plan.panels.status_bar || self.runtime_plan.panels.keymap_bar {
            rows.push(Constraint::Length(3));
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(rows)
            .split(frame.area());

        let mut row_index = 0;
        let top_start = Instant::now();
        if self.runtime_plan.panels.search_bar {
            self.draw_top_bar(frame, chunks[row_index]);
            row_index += 1;
        }
        let top_us = top_start.elapsed().as_micros() as u64;

        let main_area = chunks[row_index];
        row_index += 1;

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(self.main_panel_constraints())
            .split(main_area);

        let mut panel_index = 0;
        let main_start = Instant::now();
        if self.runtime_plan.panels.sidebar {
            self.draw_sidebar(frame, main_chunks[panel_index]);
            panel_index += 1;
        }
        if self.runtime_plan.panels.columns {
            self.draw_miller_columns(frame, main_chunks[panel_index]);
            panel_index += 1;
        }
        if self.runtime_plan.panels.preview {
            self.draw_details_panel(frame, main_chunks[panel_index]);
            panel_index += 1;
        }
        if panel_index == 0 {
            let p = Paragraph::new("All main panels are disabled in config.")
                .block(Block::default().title("DIRT").borders(Borders::ALL));
            frame.render_widget(p, main_area);
        }
        let main_us = main_start.elapsed().as_micros() as u64;

        let status_start = Instant::now();
        if self.runtime_plan.panels.status_bar || self.runtime_plan.panels.keymap_bar {
            if self.runtime_plan.panels.keymap_bar {
                self.draw_keymap_bar(frame, chunks[row_index]);
            } else {
                self.draw_status(frame, chunks[row_index]);
            }
        }
        let status_us = status_start.elapsed().as_micros() as u64;
        if self.dialog.is_some() {
            self.draw_dialog(frame);
        } else if self.input_mode != InputMode::None || self.sudo_password_prompt || self.keybinds_overlay.is_some() {
            self.draw_dialog(frame);
        }
        let total_us = t0.elapsed().as_micros() as u64;
        PERF_FRAMES.fetch_add(1, AtomicOrdering::Relaxed);
        PERF_TOTAL_US.fetch_add(total_us, AtomicOrdering::Relaxed);
        PERF_MAIN_US.fetch_add(main_us, AtomicOrdering::Relaxed);
        PERF_TOP_US.fetch_add(top_us, AtomicOrdering::Relaxed);
        PERF_STATUS_US.fetch_add(status_us, AtomicOrdering::Relaxed);
    }

    fn main_panel_constraints(&self) -> Vec<Constraint> {
        let mut weights: Vec<u16> = Vec::new();
        if self.runtime_plan.panels.sidebar {
            weights.push(self.effective.panel_sizes.sidebar.max(1));
        }
        if self.runtime_plan.panels.columns {
            weights.push(self.effective.panel_sizes.dir.max(1));
        }
        if self.runtime_plan.panels.preview {
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

    fn column_theme_for(&self, idx: usize) -> &ColumnTheme {
        match idx {
            0 => &self.effective.theme_colors.col_1,
            1 => &self.effective.theme_colors.col_2,
            2 => &self.effective.theme_colors.col_3,
            _ => &self.effective.theme_colors.col_4,
        }
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
        self.columns.push(DirColumn::from_path(
            path.clone(),
            &self.effective,
            self.sudo_mode,
            if self.sudo_password_input.is_empty() { None } else { Some(self.sudo_password_input.as_str()) },
        ));
        self.sudo_password_prompt = self.sudo_mode && self.columns.iter().any(|c| c.sudo_password_required);
        self.current_dir = path;
        self.track_recent_dir();
        self.clear_simple_selection();
    }

    fn go_parent(&mut self) {
        if self.columns.len() <= 1 {
            return;
        }
        self.columns.pop();
        if let Some(col) = self.columns.last() {
            self.current_dir = col.path.clone();
            self.track_recent_dir();
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
        self.columns = build_columns_from_path(&self.effective.start_dir, &self.effective, self.sudo_mode, if self.sudo_password_input.is_empty() { None } else { Some(self.sudo_password_input.as_str()) });
        self.sudo_password_prompt = self.sudo_mode && self.columns.iter().any(|c| c.sudo_password_required);
        self.current_dir = self.effective.start_dir.clone();
        self.track_recent_dir();
    }

    fn reinitialize_columns_preserve_current(&mut self) {
        let target = if self.current_dir.is_dir() {
            self.current_dir.clone()
        } else {
            self.effective.start_dir.clone()
        };
        self.columns = build_columns_from_path(&target, &self.effective, self.sudo_mode, if self.sudo_password_input.is_empty() { None } else { Some(self.sudo_password_input.as_str()) });
        self.sudo_password_prompt = self.sudo_mode && self.columns.iter().any(|c| c.sudo_password_required);
        self.current_dir = target;
        self.track_recent_dir();
    }

    fn draw_dialog(&self, frame: &mut Frame) {
        overlay::draw_dim(frame, &self.effective.theme_colors);
        if let Some(frame_state) = &self.keybinds_overlay {
            overlay::draw_frame(frame, &self.effective.theme_colors, frame_state);
            return;
        }
        if self.sudo_password_prompt {
            overlay::draw_dialog(
                frame,
                &self.effective.theme_colors,
                &OverlayDialog {
                    title: "SUDO PASSWORD".to_string(),
                    message: "[sudo] password:".to_string(),
                    kind: OverlayDialogKind::Input { password: true, placeholder: None },
                    input: self.sudo_password_input.clone(),
                    selected_is_primary: true,
                },
            );
            return;
        }
        if self.input_mode != InputMode::None {
            let title = match self.input_mode {
                InputMode::NewFile => "NEW FILE",
                InputMode::NewDir => "NEW FOLDER",
                InputMode::Rename => "RENAME",
                InputMode::None => "INPUT",
            };
            overlay::draw_dialog(
                frame,
                &self.effective.theme_colors,
                &OverlayDialog {
                    title: title.to_string(),
                    message: String::new(),
                    kind: OverlayDialogKind::Input { password: false, placeholder: None },
                    input: self.input_buffer.clone(),
                    selected_is_primary: true,
                },
            );
            return;
        }
        let Some(dialog) = &self.dialog else {
            return;
        };
        match dialog {
            DialogState::ConfirmDelete {
                paths,
                yes_selected,
            } => {
                let msg = if paths.len() == 1 {
                    let name = paths[0]
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "item".to_string());
                    format!("Delete {name}?")
                } else {
                    format!("Delete {} items?", paths.len())
                };
                overlay::draw_dialog(
                    frame,
                    &self.effective.theme_colors,
                    &OverlayDialog {
                        title: "CONFIRM".to_string(),
                        message: msg,
                        kind: OverlayDialogKind::Confirm,
                        input: String::new(),
                        selected_is_primary: *yes_selected,
                    },
                );
            }
            DialogState::ConfirmBookmarkOverwrite {
                slot,
                existing_path,
                new_path,
                yes_selected,
            } => {
                overlay::draw_dialog(
                    frame,
                    &self.effective.theme_colors,
                    &OverlayDialog {
                        title: "CONFIRM".to_string(),
                        message: format!("Overwrite bookmark {slot}?\nold: {existing_path}\nnew: {new_path}"),
                        kind: OverlayDialogKind::Confirm,
                        input: String::new(),
                        selected_is_primary: *yes_selected,
                    },
                );
            }
            DialogState::Message { title, text } => {
                overlay::draw_dialog(
                    frame,
                    &self.effective.theme_colors,
                    &OverlayDialog {
                        title: title.clone(),
                        message: text.clone(),
                        kind: OverlayDialogKind::Message,
                        input: String::new(),
                        selected_is_primary: true,
                    },
                );
            }
        }
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
                .map(|p| preview_feature::preview_for_path(
                    p,
                    &self.effective.theme_colors,
                    self.selection_mode,
                    self.search_mode != SearchMode::None,
                    self.image_preview_config(),
                    self.image_protocol,
                ))
                .unwrap_or(PreviewData::Empty);
        }
        let Some(entry) = self.selected_entry() else {
            return PreviewData::Empty;
        };
        preview_feature::preview_for_path(
            &entry.path,
            &self.effective.theme_colors,
            self.selection_mode,
            self.search_mode != SearchMode::None,
            self.image_preview_config(),
            self.image_protocol,
        )
    }

    fn image_preview_config(&self) -> PreviewImageConfig {
        PreviewImageConfig {
            enabled: self.runtime_plan.features.preview_images,
            max_image_size_mb: self.effective.preview.max_image_size_mb,
        }
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
                    .fg(parse_color(&self.effective.theme_colors.col_1.border)),
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
                        .bg(parse_color(&self.effective.theme_colors.col_1.selected_bg))
                        .fg(parse_color(&self.effective.theme_colors.col_1.selected_fg)),
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

fn bookmark_slot_from_key(key: crossterm::event::KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(c) if ('1'..='9').contains(&c) => Some(c),
        // Some terminals encode Ctrl+number as shifted symbols.
        KeyCode::Char('!') => Some('1'),
        KeyCode::Char('@') => Some('2'),
        KeyCode::Char('#') => Some('3'),
        KeyCode::Char('$') => Some('4'),
        KeyCode::Char('%') => Some('5'),
        KeyCode::Char('^') => Some('6'),
        KeyCode::Char('&') => Some('7'),
        KeyCode::Char('*') => Some('8'),
        KeyCode::Char('(') => Some('9'),
        _ => None,
    }
}

fn bookmark_matches_slot(bookmark: &Bookmark, slot: char) -> bool {
    if let Some(s) = bookmark.slot
        && let Some(sc) = char::from_digit(s as u32, 10)
    {
        return sc == slot;
    }
    bookmark.name == format!("slot-{slot}")
}

impl App {

    fn ui_mode_state(&self) -> UiModeState {
        UiModeState {
            sudo: self.sudo_mode,
            select: self.selection_mode,
            bookmark: self.awaiting_bookmark_slot,
            search: self.search_mode != SearchMode::None,
        }
    }

    // Mode precedence: normal -> sudo -> select/bookmark -> search.
    fn ui_mode_color_name(&self) -> &String {
        let mode = self.ui_mode_state();
        if mode.search {
            &self.effective.theme_colors.vars.search_mode
        } else if mode.bookmark {
            &self.effective.theme_colors.vars.bookmark_mode
        } else if mode.select {
            &self.effective.theme_colors.vars.selection_mode
        } else if mode.sudo {
            &self.effective.theme_colors.vars.sudo_mode
        } else {
            &self.effective.theme_colors.vars.defult_panel_label
        }
    }

    fn ui_mode_color(&self) -> Color {
        parse_color(self.ui_mode_color_name())
    }


    fn track_recent_dir(&mut self) {
        if !self.current_dir.is_dir() {
            return;
        }
        self.recents.retain(|p| p != &self.current_dir);
        self.recents.insert(0, self.current_dir.clone());
        let keep = self.config.sidebar.recent_dirs_limit.max(1);
        if self.recents.len() > keep {
            self.recents.truncate(keep);
        }
        let _ = save_recents(&self.recents);
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
    permission_denied: bool,
    sudo_password_required: bool,
}

impl DirColumn {
    fn from_path(path: PathBuf, settings: &EffectiveSettings, sudo_mode: bool, sudo_password: Option<&str>) -> Self {
        let read = read_dir_entries(&path, settings, sudo_mode, sudo_password);
        Self {
            path,
            entries: read.entries,
            selected: 0,
            permission_denied: read.permission_denied,
            sudo_password_required: read.sudo_password_required,
        }
    }
}

fn user_device_names() -> (String, String) {
    let user = env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string());
    let device = Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "device".to_string());
    (user, device)
}

fn default_true() -> bool {
    true
}

fn default_recent_dirs_limit() -> usize {
    10
}

fn default_drives_limit() -> usize {
    10
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

fn default_active_column() -> usize {
    2
}

fn default_column_ratios() -> Vec<u16> {
    vec![1, 1, 1, 1]
}

fn default_search_max_depth() -> usize {
    6
}

fn default_max_image_size_mb() -> u64 {
    20
}

pub(crate) fn parse_color(name: &str) -> Color {
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

pub(crate) fn path_last_segment(path: &Path) -> String {
    path.file_name()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn scroll_start(total_rows: usize, viewport_rows: usize, anchor: usize) -> usize {
    if viewport_rows == 0 || total_rows <= viewport_rows {
        return 0;
    }
    let half = viewport_rows / 2;
    let centered = anchor.saturating_sub(half);
    centered.min(total_rows - viewport_rows)
}

pub(crate) fn format_system_time(time: Option<std::time::SystemTime>) -> String {
    match time {
        Some(t) => {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M").to_string()
        }
        None => "-".to_string(),
    }
}

pub(crate) fn format_size(bytes: u64) -> String {
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

pub(crate) fn image_metadata_lines(path: &Path) -> Vec<Line<'static>> {
    let filename = path_last_segment(path);
    let meta = fs::metadata(path).ok();
    let size = meta
        .as_ref()
        .map(|m| format_size(m.len()))
        .unwrap_or_else(|| "-".to_string());
    let modified = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .map(|m| format_system_time(Some(m)))
        .unwrap_or_else(|| "-".to_string());
    let dim_text = image::image_dimensions(path)
        .map(|(w, h)| format!("{w}x{h} px"))
        .unwrap_or_else(|_| "-".to_string());
    vec![
        Line::from(filename),
        Line::from(dim_text),
        Line::from(size),
        Line::from(modified),
    ]
}

pub(crate) fn image_preview_panel_height(area: Rect, path: &Path) -> u16 {
    // Terminal cells are usually taller than they are wide.
    // width/height ~= 0.5 keeps visual image proportions in cell-space.
    const CELL_WIDTH_OVER_HEIGHT: f64 = 0.5;
    let min_panel_height = 6u16;
    let max_panel_height = area.height.saturating_sub(5).max(min_panel_height);
    let Ok((img_w, img_h)) = image::image_dimensions(path) else {
        return max_panel_height;
    };
    if img_w == 0 || img_h == 0 {
        return max_panel_height;
    }

    let inner_width = area.width.saturating_sub(2).max(1) as f64;
    let desired_inner_height =
        (inner_width * (img_h as f64 / img_w as f64) * CELL_WIDTH_OVER_HEIGHT).ceil() as u16;
    desired_inner_height
        .saturating_add(2)
        .clamp(min_panel_height, max_panel_height)
}

fn render_no_perms_mascot(frame: &mut Frame, area: Rect, theme: &Theme, path: &Path, sudo_mode: bool) {
    let body_color = if sudo_mode {
        parse_color(&theme.vars.sudo_mode)
    } else {
        parse_color(&theme.mascot.no_perms_body)
    };
    let text_color = parse_color(&theme.mascot.text);
    let shadow_color = parse_color(&theme.mascot.shadow);
    let secondary = parse_color(&theme.vars.secondary_fg);
    let mascot = [
        "  ██████",
        " █████████████████",
        " ██// NO █████████▒",
        " ██PERMSSIONS ████▒",
        " ██/SUDO █████████▒",
        " █████████████████▒",
        "  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒",
    ];
    let mut lines: Vec<Line<'static>> = Vec::new();
    let top_pad = area.height.saturating_sub((mascot.len() + 2) as u16) / 2;
    for _ in 0..top_pad {
        lines.push(Line::from(""));
    }
    for raw in mascot {
        let spans = raw
            .chars()
            .map(|c| {
                let style = match c {
                    '█' => Style::default().fg(body_color),
                    '▒' => Style::default().fg(shadow_color),
                    '/' | 'N' | 'O' | 'P' | 'E' | 'R' | 'M' | 'S' | 'I' | 'U' | 'D' => {
                        Style::default().fg(text_color)
                    }
                    _ => Style::default().fg(body_color),
                };
                Span::styled(c.to_string(), style)
            })
            .collect::<Vec<_>>();
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(display_home_relative(path), Style::default().fg(secondary))));
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
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
