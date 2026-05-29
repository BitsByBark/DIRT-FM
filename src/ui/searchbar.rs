#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchbarCommand {
    ToggleSudo,
    ConfigInit,
    ConfigLayoutInit,
    ConfigThemeInit,
    KeymapInit,
    Keybinds,
}

pub fn parse_command(input: &str) -> Option<SearchbarCommand> {
    match input.trim() {
        "sudo" | "/sudo" => Some(SearchbarCommand::ToggleSudo),
        "config init" | "/config init" => Some(SearchbarCommand::ConfigInit),
        "config layout init" | "/config layout init" => Some(SearchbarCommand::ConfigLayoutInit),
        "config theme init" | "/config theme init" => Some(SearchbarCommand::ConfigThemeInit),
        "keymap init" | "/keymap init" => Some(SearchbarCommand::KeymapInit),
        "keybinds" | "/keybinds" => Some(SearchbarCommand::Keybinds),
        _ => None,
    }
}
