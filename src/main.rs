mod app;
mod config;
mod input;
mod theme;
mod ui;
mod fs;

fn main() -> color_eyre::Result<()> {
    app::run_app()
}
