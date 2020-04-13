//! Application frontends
//!
//!

use crate::polycomplex::ComplexFn;
use crate::rctx::{Rctx, RctxTransform};
use crate::Bounds;
use std::fs::File;
use std::io::{self, Write};
use std::time::{Instant, SystemTime};
use termion::event::Key;
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::widgets::{Block, Borders, Widget};
use tui::Terminal;

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

/// Configuration for `run` subcommand
///
/// TODO: move to a more reasonable place
#[derive(Debug)]
pub struct RunOptions {
    pub img_dir: std::path::PathBuf,
}

impl RunOptions {
    pub fn new(img_dir: Option<std::path::PathBuf>) -> Self {
        Self {
            img_dir: img_dir.unwrap_or(std::path::PathBuf::from(".")),
        }
    }
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
            Key::Char('x') => AppCmd::Transform(RctxTransform::SwitchFn),

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
    std::io::stdout()
        .into_raw_mode()
        .map(|stdout| termion::screen::AlternateScreen::from(stdout))
        .and_then(|mut screen| {
            write!(
                screen,
                "{}{}",
                termion::screen::ToMainScreen,
                termion::cursor::Show
            )?;
            Ok(screen)
        })
        .and_then(|mut screen| screen.flush())
}

/// Generate an image and location data for a given render context and bounds.
///
/// TODO: handle write errors without panicking.
fn screenshot(
    rctx: &Rctx,
    old_bounds: &Bounds,
    img_dir: &std::path::Path,
) -> Result<(), crate::Error> {
    let new_bounds = Bounds {
        width: 4000,
        height: 4000,
    };

    let imgen_rctx = Rctx {
        loc: rctx
            .loc
            .scale(old_bounds, &new_bounds, crate::loc::ScaleMethod::Min),
        ..rctx.clone()
    };

    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| u64::from(duration.as_secs()))
        .unwrap();

    let mut json_path = std::path::PathBuf::from(img_dir);
    json_path.push(format!("mb-{}.json", unix_secs));
    File::create(json_path).and_then(|mut f| {
        let buf = serde_json::to_string(&imgen_rctx).unwrap();
        f.write_all(&buf.as_bytes())
    })?;

    let mut png_path = std::path::PathBuf::from(img_dir);
    png_path.push(format!("mb-{}.png", unix_secs));
    imgen_rctx
        .bind(new_bounds)
        .to_ematrix()
        .to_img(&imgen_rctx.colorer)
        .save(png_path)
        .map_err(|e| crate::Error::from(e))
}

/// Accept a key input, act on that input, and indicate if the app should keep going.
fn handle_key(key: Key, rctx: &mut Rctx, bounds: &Bounds, run_options: &RunOptions) -> Option<()> {
    let cmd = AppCmd::from(key);
    match &cmd {
        AppCmd::Transform(t) => {
            rctx.transform(&t);
            Some(())
        }
        AppCmd::Save => {
            // TODO: handle errors when generating screenshots.
            let _ = screenshot(&rctx, bounds, run_options.img_dir.as_path());
            Some(())
        }
        AppCmd::Unhandled(_) => Some(()),
        AppCmd::Quit => None,
    }
}

pub trait Frontend: Send + Sync + std::panic::UnwindSafe {
    fn run(
        &mut self,
        initial_rctx: Rctx,
        run_options: RunOptions,
    ) -> std::result::Result<(), crate::Error> {
        let mut rctx = initial_rctx;
        loop {
            let bounds: Bounds = termion::terminal_size()?.into();
            self.draw(&rctx, &bounds)?;

            match self.update(&mut rctx, &bounds, &run_options) {
                Ok(Some(())) => {}
                Ok(None) | Err(_) => break,
            }
        }

        Ok(())
    }

    /// Redraw the UI
    fn draw(&mut self, rctx: &Rctx, bounds: &Bounds) -> Result<(), crate::Error>;

    /// Read input and update the frontend state accordingly
    fn update(
        &mut self,
        rctx: &mut Rctx,
        bounds: &Bounds,
        run_options: &RunOptions,
    ) -> Result<Option<()>, crate::Error>;
}

pub struct Termion {
    stdin: std::io::Stdin,
    screen: termion::screen::AlternateScreen<termion::raw::RawTerminal<std::io::Stdout>>,
}

impl Termion {
    pub fn build() -> Result<Self, crate::Error> {
        // Terminal initialization
        let stdin = io::stdin();
        let stdout = io::stdout().into_raw_mode().unwrap();
        let mut screen = termion::screen::AlternateScreen::from(stdout);

        write!(
            screen,
            "{}{}",
            termion::screen::ToAlternateScreen,
            termion::cursor::Hide
        )?;

        Ok(Termion { stdin, screen })
    }

    /// Convert an RGB image to a series of ANSI escape sequences that set the cursor and paint the
    /// background.
    fn img_to_ansi(&self, img: &image::RgbImage, bounds: &Bounds) -> String {
        let mut buf = String::new();
        for yi in 0..bounds.height {
            for xi in 0..bounds.width {
                let pos = crate::Pos { x: xi, y: yi };
                let pixel = img.get_pixel(xi.into(), yi.into());
                buf.push_str(String::from(termion::cursor::Goto(pos.x + 1, pos.y + 1)).as_str());
                buf.push_str(
                    termion::color::Rgb(pixel[0], pixel[1], pixel[2])
                        .bg_string()
                        .as_str(),
                );
                buf.push(' ');
            }
        }
        buf
    }
}

impl Frontend for Termion {
    fn draw(&mut self, rctx: &Rctx, bounds: &Bounds) -> Result<(), crate::Error> {
        let render_start: Instant = Instant::now();
        let img = rctx.bind(*bounds).to_ematrix().to_img(&rctx.colorer);
        let ansi = self.img_to_ansi(&img, bounds);
        let render_stop: Instant = Instant::now();

        let draw_start = Instant::now();
        write!(self.screen, "{}", ansi).unwrap();
        self.screen.flush()?;
        let draw_stop = Instant::now();

        let render_delta = render_stop - render_start;
        let draw_delta = draw_stop - draw_start;

        let labels = vec![
            format!("exp    = {:.4e}", &rctx.complexfn.exp()),
            format!("re     = {:.4e}", rctx.loc.re0),
            format!("im     = {:.4e}", rctx.loc.im0),
            format!("iter   = {}", rctx.loc.max_iter),
            format!("scalar = {:.4e}", rctx.loc.scalar),
            format!("render = {}ms", render_delta.as_millis()),
            format!("draw   = {}ms", draw_delta.as_millis()),
        ];

        for (offset, label) in labels.iter().enumerate() {
            write!(
                self.screen,
                "{}{}{}",
                termion::cursor::Goto(1, offset as u16 + 1),
                termion::style::Reset,
                label
            )?
        }

        self.screen.flush()?;
        Ok(())
    }

    /// XXX this code looks pathological, refactor soon
    fn update(
        &mut self,
        rctx: &mut Rctx,
        bounds: &Bounds,
        run_options: &RunOptions,
    ) -> Result<Option<()>, crate::Error> {
        match (&mut self.stdin).keys().next() {
            None | Some(Err(_)) => Ok(None), // Stdin was closed or could not be read, shut down.
            Some(Ok(key)) => Ok(handle_key(key, rctx, &bounds, &run_options)),
        }
    }
}

impl Drop for Termion {
    fn drop(&mut self) {
        let _w = write!(
            self.screen,
            "{}{}",
            termion::screen::ToMainScreen,
            termion::cursor::Show
        );
        let _f = self.screen.flush();
    }
}

pub struct Tui {
    stdin: std::io::Stdin,
    terminal: tui::Terminal<
        tui::backend::TermionBackend<
            termion::screen::AlternateScreen<
                MouseTerminal<termion::raw::RawTerminal<std::io::Stdout>>,
            >,
        >,
    >,
}

impl Tui {
    pub fn build() -> Result<Self, crate::Error> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout().into_raw_mode()?;
        let stdout = MouseTerminal::from(stdout);
        let stdout = termion::screen::AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.hide_cursor()?;

        Ok(Self { stdin, terminal })
    }
}

impl Frontend for Tui {
    /// Redraw the UI with TUI
    ///
    /// TODO: clean up `_bounds` arg
    fn draw(&mut self, rctx: &Rctx, _bounds: &Bounds) -> Result<(), crate::Error> {
        self.terminal
            .draw(|mut frame| {
                let sections = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
                    .split(frame.size());

                Block::default()
                    .title("Sidebar")
                    .borders(Borders::ALL)
                    .render(&mut frame, sections[0]);

                // XXX bad clone, shouldn't be necessary
                rctx.clone().render(&mut frame, sections[1]);
            })
            .map_err(|e| e.into())
    }

    fn update(
        &mut self,
        rctx: &mut Rctx,
        bounds: &Bounds,
        run_options: &RunOptions,
    ) -> Result<Option<()>, crate::Error> {
        match (&mut self.stdin).keys().next() {
            None | Some(Err(_)) => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                Ok(Some(()))
            }
            Some(Ok(key)) => Ok(handle_key(key, rctx, &bounds, &run_options)),
        }
    }
}
