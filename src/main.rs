extern crate nalgebra;
extern crate num;
extern crate serde;
extern crate structopt;
extern crate termion;
extern crate tui;

use mandelbrot::frontend::{self, AppCmd};
use mandelbrot::rctx::Rctx;
use mandelbrot::{loc::Loc, Bounds, Error};
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

fn draw_frame<W: Write>(screen: &mut W, rctx: &Rctx, bounds: Bounds) -> Result<(), crate::Error> {
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
fn screenshot(rctx: &Rctx, bounds: Bounds) -> Result<(), crate::Error> {
    let imgen_bounds = Bounds {
        width: 4000,
        height: 4000,
    };

    let mut imgen_loc = rctx
        .loc
        .scale(bounds, imgen_bounds, mandelbrot::loc::ScaleMethod::Min);

    let imgen_rctx = Rctx {
        loc: imgen_loc,
        ..rctx.clone()
    };

    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| u64::from(duration.as_secs()))
        .unwrap();

    let json_path = format!("mb-{}.json", unix_secs);
    File::create(json_path).and_then(|mut f| {
        let buf = serde_json::to_string(&imgen_rctx).unwrap();
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
fn handle_key(key: Key, rctx: &mut Rctx, bounds: &Bounds) -> Option<()> {
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

fn run_termion(mut rctx: Rctx) -> std::result::Result<(), crate::Error> {
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

fn run_tui(mut rctx: Rctx) -> std::result::Result<(), crate::Error> {
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
enum FrontendType {
    Termion,
    Tui,
}

#[derive(Debug)]
pub struct FrontendTypeParseError(String);

impl std::fmt::Display for FrontendTypeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Cannot parse {} to frontend", self.0)
    }
}

impl std::error::Error for FrontendTypeParseError {}

impl std::str::FromStr for FrontendType {
    type Err = FrontendTypeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tui" => Ok(FrontendType::Tui),
            "termion" => Ok(FrontendType::Termion),
            _ => Err(FrontendTypeParseError(s.to_string())),
        }
    }
}

fn read_rctx(path: &std::path::PathBuf) -> std::result::Result<Rctx, crate::Error> {
    let mut buf = String::new();
    File::open(&path)
        .map_err(|e| e.into())
        .and_then(|mut fh| {
            fh.read_to_string(&mut buf)
                .map_err(|e| crate::Error::from(e))
        })
        .and_then(|_| serde_json::from_str(&buf).map_err(|e| e.into()))
}

#[derive(Debug, StructOpt)]
#[structopt(name = "mandelbrot")]
struct AppOptions {
    #[structopt(long = "spec")]
    spec: Option<std::path::PathBuf>,
}

#[derive(Debug, StructOpt)]
enum Subcommand {
    #[structopt(name = "live")]
    Live {
        #[structopt(long = "frontend")]
        frontend_type: Option<FrontendType>,

        #[structopt(long = "spec")]
        spec: Option<std::path::PathBuf>,
    },

    #[structopt(name = "render")]
    Render {
        spec: std::path::PathBuf,

        #[structopt(long = "dest")]
        dest: Option<std::path::PathBuf>,

        #[structopt(long = "height", default_value = "4000")]
        height: u16,

        #[structopt(long = "width", default_value = "4000")]
        width: u16,
    },
}

#[derive(Debug, StructOpt)]
struct Command {
    #[structopt(subcommand)]
    subcommand: Subcommand,
}

/// Run an interactive mandelbrot explorer
fn live(
    frontend_type: Option<FrontendType>,
    spec: Option<std::path::PathBuf>,
) -> std::result::Result<(), crate::Error> {
    let mut rctx: Rctx;
    if let Some(ref path) = spec {
        rctx = read_rctx(&path)?;
    } else {
        rctx = Rctx::with_loc(Loc::for_bounds(termion::terminal_size()?.into()));
    }
    rctx.comp = (2.3, 1.0);

    let runtime = match frontend_type {
        None | Some(FrontendType::Termion) => run_termion,
        Some(FrontendType::Tui) => run_tui,
    };

    frontend::run_with_altscreen(move || runtime(rctx))
}

#[allow(unused)]
fn time_fn<T, U>(desc: &str, f: T) -> U
where
    T: FnOnce() -> U,
{
    let start = Instant::now();
    let result: U = f();
    println!("{} elapsed: {:?}", desc, start.elapsed());
    result
}

/// Render a fractal from the given spec/rctx
fn render(
    spec: std::path::PathBuf,
    height: u16,
    width: u16,
    dest: Option<std::path::PathBuf>,
) -> std::result::Result<(), crate::Error> {
    let mut rctx = read_rctx(&spec)?;
    rctx.comp = (1., 1.);
    let bound_rctx = rctx.bind(Bounds {
        height: height,
        width: width,
    });

    let output_path = dest.unwrap_or(spec.with_extension("png"));

    let ematrix = time_fn("ematrix", || bound_rctx.to_ematrix());
    let img = time_fn("coloring", || ematrix.to_img(&rctx.colorer));
    img.save(&output_path).map_err(|e| Error::from(e))
}

fn main() -> std::result::Result<(), crate::Error> {
    let cmd = Command::from_args();

    match cmd.subcommand {
        Subcommand::Live {
            frontend_type,
            spec,
        } => live(frontend_type, spec),
        Subcommand::Render {
            spec,
            height,
            width,
            dest,
        } => render(spec, height, width, dest),
    }
}
