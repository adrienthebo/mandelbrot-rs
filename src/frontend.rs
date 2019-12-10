//! Application frontends
//!
//!

use crate::rctx::RctxTransform;
use std::io::Write;
use termion::event::Key;
use termion::raw::IntoRawMode;

#[derive(Debug, Clone, Copy)]
pub enum AppCmd {
    /// Transform the rendering context.
    Transform(RctxTransform),

    /// Generate a screenshot based on the current rendering context.
    Save,

    /// Gracefully shut down the app.
    Quit,

    /// An unhandled command.
    Unhandled(Key),
}

impl From<Key> for AppCmd {
    fn from(key: Key) -> AppCmd {
        match key {
            Key::Char('q') => AppCmd::Quit,

            // Zoom in/out - shift key is optional.
            Key::Char('+') | Key::Char('=') => AppCmd::Transform(RctxTransform::ScaleIn),
            Key::Char('-') | Key::Char('_') => AppCmd::Transform(RctxTransform::ScaleOut),

            // Move left/right along the real axis.
            Key::Char('a') => AppCmd::Transform(RctxTransform::TranslateLeft),
            Key::Char('d') => AppCmd::Transform(RctxTransform::TranslateRight),

            // Move up and down on the imaginary axis.
            Key::Char('w') => AppCmd::Transform(RctxTransform::TranslateUp),
            Key::Char('s') => AppCmd::Transform(RctxTransform::TranslateDown),

            // Increase/decrease the limit on iterations to escape.
            Key::Char('t') => AppCmd::Transform(RctxTransform::IncIterations),
            Key::Char('g') => AppCmd::Transform(RctxTransform::DecIterations),

            Key::Char('y') => AppCmd::Transform(RctxTransform::IncExp),
            Key::Char('h') => AppCmd::Transform(RctxTransform::DecExp),

            // Toggle between the Julia sets and the Mandelbrot sets.
            Key::Char('x') => AppCmd::Transform(RctxTransform::ToggleHolo),

            // Reset the zoom level to default.
            Key::Char('m') => AppCmd::Transform(RctxTransform::Reset),

            // Generate a state file and image for the current location.
            Key::Char('p') => AppCmd::Save,

            u => AppCmd::Unhandled(u),
        }
    }
}

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
