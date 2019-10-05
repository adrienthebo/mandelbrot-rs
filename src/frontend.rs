//! Application frontends
//!
//!

use std::io::Write;
use termion::raw::IntoRawMode;

/// Run a closure in an alternate screen, and disable the alternate screen before handling a
/// panic.
pub fn run_with_altscreen<F: FnOnce() -> T + std::panic::UnwindSafe, T>(f: F) -> T {
    let result = std::panic::catch_unwind(f);

    match result {
        Err(err) => {
            // XXX: It is unclear what should happen when the terminal cannot be reset to the main
            // screen.
            //
            // Cases:
            //  - A terminal is not attached: we can't shut down. Exit silently.
            //  - We encountered an error when resetting the terminal. Panic hard.
            //
            let _ = reset_terminal();
            std::panic::resume_unwind(err)
        }
        Ok(t) => t,
    }
}

fn reset_terminal() -> std::io::Result<()> {
    let stdout = std::io::stdout().into_raw_mode().unwrap();
    let mut screen = termion::screen::AlternateScreen::from(stdout);
    write!(screen, "{}", termion::screen::ToMainScreen).unwrap();
    write!(screen, "{}", termion::cursor::Show).unwrap();
    screen.flush()
}
