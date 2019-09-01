extern crate image;
extern crate itertools;
extern crate mandelbrot;
extern crate nalgebra;
extern crate num;
extern crate serde;
extern crate structopt;
extern crate termion;
extern crate tui;

use itertools::Itertools;
use mandelbrot::*;
use std::fs::File;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use structopt::StructOpt;
use termion::event::Key;
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::*;

use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::widgets::{Block, Borders, Widget};
use tui::Terminal;

#[derive(Debug, Clone, Copy)]
enum AppCmd {
    Transform(RctxTransform),
    Save,
    Quit,
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

            // Increase the limit on iterations to escape.
            Key::Char('t') => AppCmd::Transform(RctxTransform::IncIterations),
            Key::Char('g') => AppCmd::Transform(RctxTransform::DecIterations),

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

/// Given XY coordinates and computed mandelbrot iteration,
/// compute the necessary ANSI to move the cursor and paint the cell.
///
/// Note: generating strings for every element is highly inefficient; we
/// should really be appending to a string slice. :shrug:
fn cell_ansi(pos: (u16, u16), escape: Escape) -> String {
    // PERF: a coloring object should be passed instead of generated for each value.
    let sr = SineRGB::default();
    let rgb = sr.rgb(escape);
    let color = termion::color::Rgb(rgb.0, rgb.1, rgb.2);

    format!(
        "{}{}{}",
        termion::cursor::Goto(pos.0 + 1, pos.1 + 1),
        termion::color::Bg(color),
        " "
    )
}

fn ematrix_to_frame(mat: &EMatrix, bounds: Bounds) -> String {
    let y_iter = 0..bounds.0;
    let x_iter = 0..bounds.1;

    y_iter
        .cartesian_product(x_iter)
        .zip(mat.iter())
        .map(move |(pos, escape)| cell_ansi(pos, *escape))
        .collect()
}

fn draw_frame<W: Write>(
    screen: &mut W,
    rctx: &RenderContext,
    bounds: Bounds,
) -> Result<(), crate::Error> {
    let render_start: Instant = Instant::now();
    let mat = rctx.to_ematrix(bounds);
    let buffer = ematrix_to_frame(&mat, bounds);
    let render_stop: Instant = Instant::now();

    let draw_start = Instant::now();
    write!(screen, "{}", buffer).unwrap();
    screen.flush()?;
    let draw_stop = Instant::now();

    let render_delta = render_stop - render_start;
    let draw_delta = draw_stop - draw_start;

    let labels = vec![
        format!("holo   = {:?}", &rctx.holomorphic),
        format!("re     = {:.4e}", rctx.loc.re0),
        format!("im     = {:.4e}", rctx.loc.im0),
        format!("iter   = {}", rctx.loc.max_iter),
        format!("scalar = {:.4e}", rctx.loc.scalar),
        format!("render = {}ms", render_delta.as_millis()),
        format!("draw   = {}ms", draw_delta.as_millis()),
    ];

    for (offset, label) in labels.iter().enumerate() {
        write!(
            screen,
            "{}{}{}",
            termion::cursor::Goto(1, offset as u16 + 1),
            termion::style::Reset,
            label
        )
        .unwrap();
    }

    screen.flush()?;
    Ok(())
}

fn screenshot(rctx: &RenderContext, bounds: Bounds) -> Result<(), crate::Error> {
    // TODO: handle write errors without panicking.
    let imgen_bounds = (4000, 4000);

    let mut imgen_loc = rctx.loc.scale(bounds, imgen_bounds);
    imgen_loc.comp.1 = 1.;

    let imgen_app = RenderContext {
        loc: imgen_loc,
        holomorphic: rctx.holomorphic.clone(),
    };
    let mat = imgen_app.to_ematrix(imgen_bounds);

    write_loc(&imgen_app)?;
    write_ematrix(&mat)?;
    Ok(())
}

fn write_ematrix(ematrix: &EMatrix) -> io::Result<()> {
    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let path = format!("mb-{}.png", unix_secs.as_secs() as u64);
    let img = ematrix.clone().to_img();
    img.save(path)
}

fn write_loc(rctx: &RenderContext) -> std::io::Result<()> {
    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let path = format!("mb-{}.json", unix_secs.as_secs() as u64);

    let mut f = File::create(path)?;
    let buf = serde_json::to_string(&rctx.loc).unwrap();
    f.write_all(&buf.as_bytes())
}

fn run_termion() -> std::result::Result<(), crate::Error> {
    // Terminal initialization
    let mut stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode().unwrap();
    let mut screen = AlternateScreen::from(stdout);

    let mut rctx = RenderContext::with_loc(Loc::for_bounds(termion::terminal_size()?));
    write!(screen, "{}", ToAlternateScreen).unwrap();
    write!(screen, "{}", termion::cursor::Hide).unwrap();

    loop {
        let bounds = termion::terminal_size()?;
        draw_frame(&mut screen, &rctx, bounds)?;
        match (&mut stdin).keys().next() {
            None | Some(Err(_)) => break, // Stdin was closed or could not be read, shut down.
            Some(Ok(key)) => {
                match AppCmd::from(key) {
                    AppCmd::Transform(t) => {
                        rctx.transform(&t);
                    }
                    AppCmd::Save => {
                        // TODO: handle errors when generating screenshots.
                        let _ = screenshot(&rctx, bounds);
                    }
                    AppCmd::Quit => break,
                    AppCmd::Unhandled(_) => {}
                }
            }
        }
    }

    write!(screen, "{}", ToMainScreen).unwrap();
    write!(screen, "{}", termion::cursor::Show).unwrap();
    screen.flush()?;

    Ok(())
}

fn run_tui() -> std::result::Result<(), crate::Error> {
    // Terminal initialization
    let mut stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut rctx = RenderContext::with_loc(Loc::for_bounds(termion::terminal_size()?));

    loop {
        let bounds = termion::terminal_size()?;
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
                .split(f.size());

            Block::default()
                .title("Sidebar")
                .borders(Borders::ALL)
                .render(&mut f, chunks[0]);

            rctx.render(&mut f, chunks[1]);

            //Block::default()
            //    .borders(Borders::NONE)
            //    .render(&mut f, chunks[1]);
        })?;

        match (&mut stdin).keys().next() {
            None | Some(Err(_)) => {
                thread::sleep(Duration::from_millis(100));
            }
            Some(Ok(key)) => {
                match AppCmd::from(key) {
                    AppCmd::Transform(t) => {
                        rctx.transform(&t);
                    }
                    AppCmd::Save => {
                        // TODO: handle errors when generating screenshots.
                        let _ = screenshot(&rctx, bounds);
                    }
                    AppCmd::Quit => break,
                    AppCmd::Unhandled(_) => {}
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
enum TuiType {
    Termion,
    Tui,
}

#[derive(Debug)]
pub struct TuiTypeParseError(String);

impl std::fmt::Display for TuiTypeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Cannot parse {} to tui type", self.0)
    }
}

impl std::error::Error for TuiTypeParseError {}

impl std::str::FromStr for TuiType {
    type Err = TuiTypeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tui" => Ok(TuiType::Tui),
            "termion" => Ok(TuiType::Termion),
            _ => Err(TuiTypeParseError(s.to_string())),
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "mandelbrot")]
struct AppOptions {
    #[structopt(short = "t", long = "tui")]
    tui_type: Option<TuiType>,
}

fn main() -> std::result::Result<(), crate::Error> {
    let opts = AppOptions::from_args();
    match opts.tui_type {
        None | Some(TuiType::Termion) => run_termion(),
        Some(TuiType::Tui) => run_tui(),
    }
}
