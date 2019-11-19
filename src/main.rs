extern crate image;
extern crate mandelbrot;
extern crate nalgebra;
extern crate num;
extern crate serde;
extern crate structopt;
extern crate termion;
extern crate tui;

use mandelbrot::frontend::{self, AppCmd};
use mandelbrot::{loc::Loc, rctx::RenderContext, Bounds, Error};
use std::fs::File;
use std::io::{self, Read, Write};
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

/// Convert an RGB image to a series of ANSI escape sequences that set the cursor and paint the
/// background.
fn img_to_ansi(img: &image::RgbImage, bounds: Bounds) -> String {
    let mut buf = String::new();
    for yi in 0..bounds.height {
        for xi in 0..bounds.width {
            let pos = mandelbrot::Pos { x: xi, y: yi };
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

fn draw_frame<W: Write>(
    screen: &mut W,
    rctx: &RenderContext,
    bounds: Bounds,
) -> Result<(), crate::Error> {
    let render_start: Instant = Instant::now();
    let img = rctx.bind(bounds).to_ematrix().to_img(&rctx.colorer);
    let ansi = img_to_ansi(&img, bounds);
    let render_stop: Instant = Instant::now();

    let draw_start = Instant::now();
    write!(screen, "{}", ansi).unwrap();
    screen.flush()?;
    let draw_stop = Instant::now();

    let render_delta = render_stop - render_start;
    let draw_delta = draw_stop - draw_start;

    let labels = vec![
        format!("fn     = {:?}", &rctx.complexfn),
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
        )?
    }

    screen.flush()?;
    Ok(())
}

/// Generate an image and location data for a given render context and bounds.
///
/// TODO: handle write errors without panicking.
fn screenshot(rctx: &RenderContext, bounds: Bounds) -> Result<(), crate::Error> {
    let imgen_bounds = Bounds {
        width: 4000,
        height: 4000,
    };

    let mut imgen_loc = rctx.loc.scale(bounds, imgen_bounds);
    imgen_loc.comp = (1., 1.);

    let imgen_rctx = RenderContext {
        loc: imgen_loc,
        ..rctx.clone()
    };

    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| u64::from(duration.as_secs()))
        .unwrap();

    let json_path = format!("mb-{}.json", unix_secs);
    File::create(json_path).and_then(|mut f| {
        let buf = serde_json::to_string(&rctx).unwrap();
        f.write_all(&buf.as_bytes())
    })?;

    let png_path = format!("mb-{}.png", unix_secs);
    imgen_rctx
        .bind(imgen_bounds)
        .to_ematrix()
        .to_img(&imgen_rctx.colorer)
        .save(png_path)
        .map_err(|e| Error::from(e))
}

/// Accept a key input, act on that input, and indicate if the app should keep going.
fn handle_key(key: Key, rctx: &mut RenderContext, bounds: &Bounds) -> Option<()> {
    match AppCmd::from(key) {
        AppCmd::Transform(t) => {
            rctx.transform(&t);
            Some(())
        }
        AppCmd::Save => {
            // TODO: handle errors when generating screenshots.
            let _ = screenshot(&rctx, *bounds);
            Some(())
        }
        AppCmd::Unhandled(_) => Some(()),
        AppCmd::Quit => None,
    }
}

fn run_termion(mut rctx: RenderContext) -> std::result::Result<(), crate::Error> {
    // Terminal initialization
    let mut stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode().unwrap();
    let mut screen = AlternateScreen::from(stdout);

    write!(screen, "{}", ToAlternateScreen).unwrap();
    write!(screen, "{}", termion::cursor::Hide).unwrap();

    loop {
        let bounds: Bounds = termion::terminal_size()?.into();
        draw_frame(&mut screen, &rctx, bounds)?;
        match (&mut stdin).keys().next() {
            None | Some(Err(_)) => break, // Stdin was closed or could not be read, shut down.
            Some(Ok(key)) => {
                if let None = handle_key(key, &mut rctx, &bounds) {
                    break;
                }
            }
        }
    }

    write!(screen, "{}", ToMainScreen).unwrap();
    write!(screen, "{}", termion::cursor::Show).unwrap();
    screen.flush()?;

    Ok(())
}

fn run_tui(mut rctx: RenderContext) -> std::result::Result<(), crate::Error> {
    // Terminal initialization
    let mut stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    loop {
        let bounds: Bounds = termion::terminal_size()?.into();
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
        })?;

        match (&mut stdin).keys().next() {
            None | Some(Err(_)) => {
                thread::sleep(Duration::from_millis(100));
            }
            Some(Ok(key)) => {
                if let None = handle_key(key, &mut rctx, &bounds) {
                    break;
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

    #[structopt(short = "l", long = "load-file")]
    load_file: Option<std::path::PathBuf>,
}

fn main() -> std::result::Result<(), crate::Error> {
    let opts = AppOptions::from_args();

    let rctx: RenderContext;
    if let Some(load_file) = opts.load_file {
        let mut fh = File::open(load_file)?;
        let mut content = String::new();
        fh.read_to_string(&mut content)
            .expect("Unable to read rctx file");
        rctx = serde_json::from_str(&content).expect("Cannot deserialize rctx");
    } else {
        rctx = RenderContext::with_loc(Loc::for_bounds(termion::terminal_size()?.into()));
    }

    let runtime = match opts.tui_type {
        None | Some(TuiType::Termion) => run_termion,
        Some(TuiType::Tui) => run_tui,
    };

    frontend::run_with_altscreen(move || runtime(rctx))
}
