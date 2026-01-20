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
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use std::io::stdout;

fn main() -> Result<()> {
    // Warn if running inside tmux - it may intercept keybindings
    if std::env::var("TMUX").is_ok() {
        eprintln!("Warning: Running inside tmux. Some keybindings (like Ctrl+K) may be");
        eprintln!("intercepted by tmux and won't reach this app. Consider running outside");
        eprintln!("tmux, or unbind conflicting keys with: tmux unbind-key C-k");
        eprintln!();
    }

    // Ignore SIGINT so Ctrl+C can be captured as a key event
    // This is needed because some terminals/systems still send SIGINT
    // even when raw mode is enabled
    #[cfg(unix)]
    unsafe {
        signal_hook::low_level::register(signal_hook::consts::SIGINT, || {})?;
    }

    let config = Config::load()?;
    let app = App::new(config)?;

    let mut terminal = ratatui::init();

    // Enable enhanced keyboard protocol (kitty protocol) if supported
    // This is the base mode for deck selection etc. The app will push/pop
    // different modes during study sessions based on deck settings.
    let enhanced_keyboard = execute!(
        stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    )
    .is_ok();

    let result = app.run(&mut terminal);

    // Restore keyboard mode if we enabled enhanced mode
    if enhanced_keyboard {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }

    ratatui::restore();

    result
}
