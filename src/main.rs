extern crate image;
extern crate itertools;
extern crate nalgebra;
extern crate num;
extern crate rayon;
extern crate serde;
extern crate termion;
extern crate mandelbrot;

use itertools::Itertools;
use rayon::prelude::*;
use std::fs::File;
use std::io::{self, Write};
use std::time::{Instant, SystemTime};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::*;
use mandelbrot::*;

#[derive(Clone, Debug)]
struct RenderContext {
    /// The current loc.
    pub loc: Loc,
    /// The active holomorphic function.
    pub holomorphic: Holomorphic,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            loc: Loc::default(),
            holomorphic: Holomorphic::default()
        }
    }
}

impl RenderContext {
    /// Generate an escape matrix from the current application context.
    ///
    /// # Performance
    ///
    /// This fn is the most expensive operation in the application.
    ///
    fn render(&self, bounds: Bounds) -> EMatrix {
        let y_iter = 0..bounds.0;
        let x_iter = 0..bounds.1;

        let escapes: Vec<Escape> = y_iter
            .cartesian_product(x_iter)
            .collect::<Vec<(u16, u16)>>()
            .par_iter()
            .map(|pos| self.loc.complex_at(bounds, pos.clone()))
            .map(|c| self.holomorphic.render(c, self.loc.max_iter))
            .collect();

        EMatrix::from_vec(usize::from(bounds.0), usize::from(bounds.1), escapes)
    }

    /// Create a new application context with a pre-defined location.
    fn with_loc(loc: Loc) -> Self {
        Self { loc, holomorphic: Holomorphic::default() }
    }

    pub fn transform(&mut self, transform: &RctxTransform) {
        match *transform {
            RctxTransform::TranslateUp => self.loc.im0 -= self.loc.scalar * 10.0,
            RctxTransform::TranslateDown => self.loc.im0 += self.loc.scalar * 10.0,

            RctxTransform::TranslateLeft => self.loc.re0 -= self.loc.scalar * 10.0,
            RctxTransform::TranslateRight => self.loc.re0 += self.loc.scalar * 10.0,

            RctxTransform::IncIterations => self.loc.max_iter += 25,
            RctxTransform::DecIterations => self.loc.max_iter -= 25,

            RctxTransform::ScaleIn => self.loc.scalar /= 2.0,
            RctxTransform::ScaleOut => self.loc.scalar *= 2.0,

            RctxTransform::Reset => {
                // TODO: use `Loc::for_bounds()` for appropriate zoom selection
                std::mem::replace(&mut self.loc, Loc::default());
            }

            RctxTransform::ToggleHolo => {
                let new_holo: Holomorphic;
                match self.holomorphic {
                    Holomorphic::Julia(ref j) => {
                        new_holo = Holomorphic::Mandelbrot(Mandelbrot::from(j));
                    }
                    Holomorphic::Mandelbrot(ref m) => {
                        new_holo = Holomorphic::Julia(Julia::from_c(m, self.loc.origin()))
                    }
                }
                self.holomorphic = new_holo;
            }
        }
    }
}

#[derive(Debug,Clone,Copy)]
enum RctxTransform {
    TranslateUp,
    TranslateDown,
    TranslateLeft,
    TranslateRight,
    ScaleIn,
    ScaleOut,
    IncIterations,
    DecIterations,
    ToggleHolo,
    Reset,
}

#[derive(Debug,Clone,Copy)]
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

fn draw_frame<W: Write>(screen: &mut W, rctx: &RenderContext, bounds: Bounds) -> Result<(), crate::Error> {
    let render_start: Instant = Instant::now();
    let mat = rctx.render(bounds);
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

    let imgen_app = RenderContext { loc: imgen_loc, holomorphic: rctx.holomorphic.clone() };
    let mat = imgen_app.render(imgen_bounds);

    write_loc(&imgen_app)?;
    write_ematrix(&mat)?;
    Ok(())
}

fn write_ematrix(ematrix: &EMatrix) -> io::Result<()> {
    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let path = format!("mb-{}.png", unix_secs.as_secs() as u64);
    let img = ematrix.clone().into_img();
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

fn main() -> std::result::Result<(), crate::Error> {
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
                    AppCmd::Transform(t) => { rctx.transform(&t); }
                    AppCmd::Save => { screenshot(&rctx, bounds); }
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
