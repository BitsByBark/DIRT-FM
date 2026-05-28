use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Theme {
    pub vars: VarsTheme,
    pub mascot: MascotTheme,
    pub top_bar: TopBarTheme,
    pub search_bar: SearchBarTheme,
    pub sidebar: SidebarTheme,
    pub col_1: ColumnTheme,
    pub col_2: ColumnTheme,
    pub col_3: ColumnTheme,
    pub col_4: ColumnTheme,
    pub preview: PreviewTheme,
    pub status_bar: StatusBarTheme,
    pub keymap_bar: KeymapBarTheme,
    pub git: GitTheme,
    pub selection: SelectionTheme,
}

#[derive(Debug, Clone)]
pub struct VarsTheme {
    pub primary_bg: String,
    pub secondary_bg: String,
    pub defult_panel_border: String,
    pub accent: String,
    pub active: String,
    pub active_dir_bg: String,
    pub active_dir_text: String,
    pub focused_dir_bg: String,
    pub focused_dir_text: String,
    pub primary_fg: String,
    pub secondary_fg: String,
    pub defult_panel_label: String,
    pub defult_text: String,
    pub bool_yes: String,
    pub bool_no: String,
    pub error_color: String,
    pub selection_mode: String,
    pub search_mode: String,
    pub bookmark_mode: String,
    pub mascot_empty_body: String,
}

#[derive(Debug, Clone)]
pub struct MascotTheme {
    pub empty_dir_body: String,
    pub text: String,
    pub shadow: String,
}

#[derive(Debug, Clone)]
pub struct TopBarTheme {
    pub border: String,
    pub dirt_label: String,
    pub separator: String,
    pub profile_name: String,
    pub selection_mode_label: String,
}

#[derive(Debug, Clone)]
pub struct SearchBarTheme {
    pub border: String,
    pub placeholder: String,
    pub text: String,
    pub cursor: String,
    pub match_color: String,
    pub no_match: String,
}

#[derive(Debug, Clone)]
pub struct SidebarTheme {
    pub border: String,
    pub section_header: String,
    pub bookmark_name: String,
    pub bookmark_path: String,
    pub drive_name: String,
    pub drive_path: String,
    pub recent_name: String,
    pub selected_bg: String,
    pub selected_fg: String,
    pub scrollbar: String,
}

#[derive(Debug, Clone)]
pub struct ColumnTheme {
    pub border: String,
    pub header: String,
    pub dir: String,
    pub file: String,
    pub symlink: String,
    pub executable: String,
    pub hidden: String,
    pub focused_bg: String,
    pub focused_fg: String,
    pub selected_bg: String,
    pub selected_fg: String,
    pub scrollbar: String,
}

#[derive(Debug, Clone)]
pub struct PreviewTheme {
    pub border: String,
    pub label: String,
    pub value: String,
    pub bool_true: String,
    pub bool_false: String,
    pub size: String,
    pub date: String,
    pub path: String,
    pub match_color: String,
    pub line_number: String,
    pub scrollbar: String,
}

#[derive(Debug, Clone)]
pub struct StatusBarTheme {
    pub border: String,
    pub path: String,
    pub separator: String,
    pub file_count: String,
    pub selected_name: String,
    pub git: String,
    pub git_none: String,
    pub profile: String,
    pub selection_mode: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct KeymapBarTheme {
    pub border: String,
    pub key: String,
    pub label: String,
    pub separator: String,
}

#[derive(Debug, Clone)]
pub struct GitTheme {
    pub modified: String,
    pub untracked: String,
    pub staged: String,
    pub ignored: String,
    pub conflicted: String,
}

#[derive(Debug, Clone)]
pub struct SelectionTheme {
    pub bg: String,
    pub fg: String,
    pub range_bg: String,
    pub dim_fg: String,
}

impl Theme {
    pub fn hardcoded_defaults() -> Self {
        Self::from_toml_value(&toml::Value::Table(toml::map::Map::new()))
    }

    pub fn from_toml_value(value: &toml::Value) -> Self {
        let vars = vars_table(value);
        let get = |path: &str, fallback: &str| resolve_color(value, &vars, path, fallback);
        Self {
            vars: VarsTheme {
                primary_bg: get("vars.primary_bg", "#FFFFFF"),
                secondary_bg: get("vars.secondary_bg", "#FFFFFF"),
                defult_panel_border: get("vars.defult_panel_border", &get("vars.secondary_bg", "#FFFFFF")),
                accent: get("vars.accent", "#FFFFFF"),
                active: get("vars.active", "#FFFFFF"),
                active_dir_bg: get("vars.active_dir_bg", "#FFFFFF"),
                active_dir_text: get("vars.active_dir_text", "#FFFFFF"),
                focused_dir_bg: get("vars.focused_dir_bg", "#FFFFFF"),
                focused_dir_text: get("vars.focused_dir_text", "#FFFFFF"),
                primary_fg: get("vars.primary_fg", "#FFFFFF"),
                secondary_fg: get("vars.secondary_fg", "#FFFFFF"),
                defult_panel_label: get("vars.defult_panel_label", &get("vars.secondary_fg", "#FFFFFF")),
                defult_text: get("vars.defult_text", &get("vars.primary_fg", "#FFFFFF")),
                bool_yes: get("vars.bool_yes", "#84E052"),
                bool_no: get("vars.bool_no", "#F64646"),
                error_color: get("vars.error_color", "#F64646"),
                selection_mode: get("vars.selection_mode", &get("status_bar.selection_mode", "#EB8D47")),
                search_mode: get("vars.search_mode", "#4CB2F6"),
                bookmark_mode: get("vars.bookmark_mode", "#EE9C9C"),
                mascot_empty_body: get("vars.mascot_empty_body", "#F64646"),
            },
            mascot: MascotTheme {
                empty_dir_body: get("mascot.empty_dir_body", &get("vars.mascot_empty_body", "#F64646")),
                text: get("mascot.text", "#FFFFFF"),
                shadow: get("mascot.shadow", "#FFFFFF"),
            },
            top_bar: TopBarTheme {
                border: get("top_bar.border", "#FFFFFF"),
                dirt_label: get("top_bar.dirt_label", "#FFFFFF"),
                separator: get("top_bar.separator", "#FFFFFF"),
                profile_name: get("top_bar.profile_name", "#FFFFFF"),
                selection_mode_label: get("top_bar.selection_mode_label", "#FFFFFF"),
            },
            search_bar: SearchBarTheme {
                border: get("search_bar.border", "#FFFFFF"),
                placeholder: get("search_bar.placeholder", "#FFFFFF"),
                text: get("search_bar.text", "#FFFFFF"),
                cursor: get("search_bar.cursor", "#FFFFFF"),
                match_color: get("search_bar.match", "#FFFFFF"),
                no_match: get("search_bar.no_match", "#FFFFFF"),
            },
            sidebar: SidebarTheme {
                border: get("sidebar.border", "#FFFFFF"),
                section_header: get("sidebar.section_header", "#FFFFFF"),
                bookmark_name: get("sidebar.bookmark_name", "#FFFFFF"),
                bookmark_path: get("sidebar.bookmark_path", "#FFFFFF"),
                drive_name: get("sidebar.drive_name", "#FFFFFF"),
                drive_path: get("sidebar.drive_path", "#FFFFFF"),
                recent_name: get("sidebar.recent_name", "#FFFFFF"),
                selected_bg: get("sidebar.selected_bg", "#FFFFFF"),
                selected_fg: get("sidebar.selected_fg", "#FFFFFF"),
                scrollbar: get("sidebar.scrollbar", "#FFFFFF"),
            },
            col_1: build_col(value, &vars, "col_1"),
            col_2: build_col(value, &vars, "col_2"),
            col_3: build_col(value, &vars, "col_3"),
            col_4: build_col(value, &vars, "col_4"),
            preview: PreviewTheme {
                border: get("preview.border", "#FFFFFF"),
                label: get("preview.label", "#FFFFFF"),
                value: get("preview.value", "#FFFFFF"),
                bool_true: get("preview.bool_true", "#FFFFFF"),
                bool_false: get("preview.bool_false", "#FFFFFF"),
                size: get("preview.size", "#FFFFFF"),
                date: get("preview.date", "#FFFFFF"),
                path: get("preview.path", "#FFFFFF"),
                match_color: get("preview.match", "#FFFFFF"),
                line_number: get("preview.line_number", "#FFFFFF"),
                scrollbar: get("preview.scrollbar", "#FFFFFF"),
            },
            status_bar: StatusBarTheme {
                border: get("status_bar.border", "#FFFFFF"),
                path: get("status_bar.path", "#FFFFFF"),
                separator: get("status_bar.separator", "#FFFFFF"),
                file_count: get("status_bar.file_count", "#FFFFFF"),
                selected_name: get("status_bar.selected_name", "#FFFFFF"),
                git: get("status_bar.git", "#FFFFFF"),
                git_none: get("status_bar.git_none", "#FFFFFF"),
                profile: get("status_bar.profile", "#FFFFFF"),
                selection_mode: get("status_bar.selection_mode", "#FFFFFF"),
                message: get("status_bar.message", "#FFFFFF"),
            },
            keymap_bar: KeymapBarTheme {
                border: get("keymap_bar.border", "#FFFFFF"),
                key: get("keymap_bar.key", "#FFFFFF"),
                label: get("keymap_bar.label", "#FFFFFF"),
                separator: get("keymap_bar.separator", "#FFFFFF"),
            },
            git: GitTheme {
                modified: get("git.modified", "#FFFFFF"),
                untracked: get("git.untracked", "#FFFFFF"),
                staged: get("git.staged", "#FFFFFF"),
                ignored: get("git.ignored", "#FFFFFF"),
                conflicted: get("git.conflicted", "#FFFFFF"),
            },
            selection: SelectionTheme {
                bg: get("selection.bg", "#FFFFFF"),
                fg: get("selection.fg", "#FFFFFF"),
                range_bg: get("selection.range_bg", "#FFFFFF"),
                dim_fg: get("selection.dim_fg", "#FFFFFF"),
            },
        }
    }
}

fn build_col(value: &toml::Value, vars: &HashMap<String, String>, key: &str) -> ColumnTheme {
    let get = |name: &str| resolve_color(value, vars, &format!("{key}.{name}"), "#FFFFFF");
    ColumnTheme {
        border: get("border"),
        header: get("header"),
        dir: get("dir"),
        file: get("file"),
        symlink: get("symlink"),
        executable: get("executable"),
        hidden: get("hidden"),
        focused_bg: get("focused_bg"),
        focused_fg: get("focused_fg"),
        selected_bg: get("selected_bg"),
        selected_fg: get("selected_fg"),
        scrollbar: get("scrollbar"),
    }
}

fn vars_table(value: &toml::Value) -> HashMap<String, String> {
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

fn resolve_color(value: &toml::Value, vars: &HashMap<String, String>, path: &str, fallback: &str) -> String {
    let Some(raw) = lookup(value, path).and_then(|v| v.as_str()) else {
        return fallback.to_string();
    };
    if let Some(var_name) = raw.strip_prefix('$') {
        return resolve_var(var_name, vars, fallback, 0);
    }
    raw.to_string()
}

fn resolve_var(name: &str, vars: &HashMap<String, String>, fallback: &str, depth: usize) -> String {
    if depth > 8 {
        eprintln!("warning: theme var recursion too deep at ${name}, using fallback {fallback}");
        return fallback.to_string();
    }
    let Some(value) = vars.get(name) else {
        eprintln!("warning: missing theme var ${name}, using fallback {fallback}");
        return fallback.to_string();
    };
    if let Some(next) = value.strip_prefix('$') {
        return resolve_var(next, vars, fallback, depth + 1);
    }
    value.clone()
}

fn lookup<'a>(value: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    let mut cur = value;
    for seg in path.split('.') {
        cur = cur.get(seg)?;
    }
    Some(cur)
}
