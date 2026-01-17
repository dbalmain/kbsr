mod app;
mod config;
mod deck;
mod keybind;
mod matcher;
mod scheduler;
mod storage;
mod ui;

use anyhow::Result;
use app::App;
use config::Config;

fn main() -> Result<()> {
    let config = Config::load()?;
    let app = App::new(config)?;

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();

    result
}
