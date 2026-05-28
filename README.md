# DIRT

Terminal file explorer in Rust with a Miller-columns workflow, profile-based config, and theme-driven UI.

## Status

Active prototype with:
- three-pane main layout (sidebar, Miller columns, details)
- two bars (top + bottom keymap/status row)
- profile switching
- local/global search
- file operations (new file/dir, copy, cut, paste, trash with confirmation)
- selection mode + range selection
- bookmark mode with numbered slots
- theme-driven UI from TOML

## Stack

- `ratatui` + `crossterm` for TUI/input
- `serde` + `toml` for config/theme parsing
- `git2` for git branch/dirty status
- `trash` for delete-to-trash behavior

## Run

```bash
cargo run
```

## Installation

### Prerequisites (all platforms)

- Rust toolchain (`rustup`, `cargo`)
- Git (optional, for cloning)

### Linux

```bash
git clone <your-repo-url> dirt
cd dirt
cargo build --release
```

Binary:
- `target/release/DIRT`

Optional install to user bin:

```bash
mkdir -p ~/.local/bin
cp target/release/DIRT ~/.local/bin/dirt
```

### macOS

```bash
git clone <your-repo-url> dirt
cd dirt
cargo build --release
```

Binary:
- `target/release/DIRT`

Optional install to user bin:

```bash
mkdir -p ~/.local/bin
cp target/release/DIRT ~/.local/bin/dirt
```

### Windows (PowerShell)

```powershell
git clone <your-repo-url> dirt
cd dirt
cargo build --release
```

Binary:
- `target\release\DIRT.exe`

Optional install:
- Copy `target\release\DIRT.exe` to a folder on your `PATH`
- Or run directly from `target\release`

### First-time setup (inside DIRT)

- Run `/config init` in the search bar to create `dirt.toml`
- Run `/config layout init` in the search bar to create `layout.toml`
- Run `/config theme init` in the search bar to create `theme.toml`
- Run `/keymap init` in the search bar to create `keymap.toml`

## Cross-platform build notes

- Native macOS builds work with:
  - `cargo build --release`
- Linux → macOS cross-compilation requires an `osxcross` toolchain.
- Recommended targets:
  - `rustup target add x86_64-apple-darwin`
  - `rustup target add aarch64-apple-darwin`

## Config

Project defaults used by the app:

`defaults/layout.toml`

Theme defaults/fallback:

`defaults/theme.toml`

User-init config paths (created by init commands):

`~/.config/dirt/dirt.toml`  
`~/.config/dirt/layout.toml`  
`~/.config/dirt/theme.toml`

Keymap config path:

`~/.config/dirt/keymap.toml`

Current behavior:
- layout/theme defaults are sourced from `defaults/layout.toml` and `defaults/theme.toml`
- `/config init` writes `dirt.toml`
- `/config layout init` writes `layout.toml`
- `/config theme init` writes `theme.toml`
- `keymap.toml` is loaded on startup when present, otherwise hardcoded defaults are used
- `/keymap init` in the search bar writes a default keymap file

## Themes

Theme data is read from TOML and supports `$var` references from `[vars]` blocks.

## Default keybinds

### Navigation
- `↑/↓` or `j/k`: move
- `←/h/Backspace`: parent
- `→/l/Enter`: open directory

### Search
- `/`: local search
- `Ctrl+F`: global search
- In search mode: `Esc` cancel, `Backspace` delete char, `Enter` confirm

### Profiles / app
- `p`: next profile
- `P`: previous profile
- `q`: quit

### Selection
- Simple range: `Shift+↑` / `Shift+↓`
- Selection mode toggle: `Ctrl+S`
- In selection mode: `Space` toggle item, `Shift+↑/↓` range toggle, `Esc` exit and clear

### File operations
- `Ctrl+N`: new file
- `Ctrl+Shift+N`: new folder
- `Ctrl+C`: copy selected
- `Ctrl+X`: cut selected
- `Ctrl+V`: paste
- `Ctrl+D`: delete to trash (confirm dialog)

### Bookmarks
- `Ctrl+B`: enter bookmark mode
- In bookmark mode: `1..9` assign slot, `Esc` exit bookmark mode
- `Ctrl+1..9` or `1..9`: open bookmark slot

## Keymap Bar

- Normal mode: `↑↓←→ navigate | Ctrl+S select mode | q quit`
- Selection mode: `Space select/deselect | Shift+↑↓ range | Esc exit | Ctrl+C copy Ctrl+X cut Ctrl+V paste Ctrl+D trash`

## Project layout

```text
src/
  main.rs
  app.rs
  mascot.rs
  config.rs
  theme.rs
  input.rs
  ui/
    mod.rs
    sidebar.rs
    columns.rs
    preview.rs
    statusbar.rs
    searchbar.rs
  fs/
    mod.rs
    ops.rs
```

## Roadmap

- finish full logic extraction from `app.rs` into dedicated modules
- add richer git/file metadata indicators in columns
- improve preview/detail rendering depth
- plugin hooks and extension points
