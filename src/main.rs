extern crate image;
extern crate itertools;
extern crate nalgebra;
extern crate num;
extern crate rayon;
extern crate serde;
extern crate termion;
extern crate mandelbrot;

use itertools::Itertools;
use num::complex::Complex64;
use rayon::prelude::*;
use serde::Serialize;
use std::fs::File;
use std::io::{self, Write};
use std::time::{Instant, SystemTime};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::*;
use mandelbrot::*;

/// The bounds for a given image, in column major order.
type Bounds = (u16, u16);

/// A location, scalar, and rendering context for a position in the complex plane.
#[derive(Clone, Debug, Serialize)]
struct Loc {
    /// The imaginary/Y axis origin.
    pub im0: f64,

    /// The real/X axis origin.
    pub re0: f64,

    /// Dimensional scaling factors in case the canvas is not square.
    ///
    /// This compensates for terminal cells having a 2:1 ratio.
    pub comp: (f64, f64),

    /// Magnification/zoom factor.
    pub scalar: f64,

    /// The maximum iterations before declaring a complex does not converge.
    pub max_iter: u32,
}

impl Loc {
    /// Determine the complex value at a given offset of the origin with respect to the provided
    /// bounds.
    fn complex_at(&self, bounds: Bounds, pos: (u16, u16)) -> Complex64 {
        let origin: (i32, i32) = (i32::from(bounds.0 / 2), i32::from(bounds.1 / 2));
        let offset: (i32, i32) = (i32::from(pos.0) - origin.0, i32::from(pos.1) - origin.1);

        Complex64 {
            re: self.comp.0 * f64::from(offset.0) * self.scalar + self.re0,
            im: self.comp.1 * f64::from(offset.1) * self.scalar + self.im0,
        }
    }

    /// Given a current bounds and a new bounds, a location that's scaled such that the original
    /// location and new location describe equivalent spaces with different resolutions.
    ///
    /// This acts to downscale/upscale a loc.
    pub fn scale(&self, old: Bounds, new: Bounds) -> Self {
        let re_scalar = f64::from(new.0) / f64::from(old.0);
        let im_scalar = f64::from(new.1) / f64::from(old.1);
        let avg = (re_scalar + im_scalar) / 2.;
        let scalar = self.scalar / avg;

        Self { scalar, .. *self }
    }
}

impl Default for Loc {
    fn default() -> Self {
        Self {
            im0: 0.0,
            re0: 0.0,
            comp: (1., 2.),
            scalar: 0.1,
            max_iter: 100,
        }
    }
}

#[derive(Clone, Debug)]
struct AppContext {
    /// The current loc.
    pub loc: Loc,
    /// The active holomorphic function.
    pub holomorphic: Holomorphic,
}

impl Default for AppContext {
    fn default() -> Self {
        Self {
            loc: Loc::default(),
            holomorphic: Holomorphic::default()
        }
    }
}

impl AppContext {
    /// Generate an escape matrix from the current application context.
    ///
    /// # Performance
    ///
    /// This fn is the most expensive operation in the application.
    ///
    fn to_ematrix(&self, bounds: Bounds) -> EMatrix {
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

fn draw_frame<W: Write>(screen: &mut W, app: &AppContext, bounds: Bounds) -> Result<(), crate::Error> {
    let render_start: Instant = Instant::now();
    let mat = app.to_ematrix(bounds);
    let buffer = ematrix_to_frame(&mat, bounds);
    let render_stop: Instant = Instant::now();

    let draw_start = Instant::now();
    write!(screen, "{}", buffer).unwrap();
    screen.flush()?;
    let draw_stop = Instant::now();

    let render_delta = render_stop - render_start;
    let draw_delta = draw_stop - draw_start;

    let labels = vec![
        format!("holo   = {:?}", &app.holomorphic),
        format!("re     = {:.4e}", app.loc.re0),
        format!("im     = {:.4e}", app.loc.im0),
        format!("iter   = {}", app.loc.max_iter),
        format!("scalar = {:.4e}", app.loc.scalar),
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

fn screenshot(app: &AppContext, bounds: Bounds) -> ! {
    // TODO: handle write errors without panicking.
    let imgen_bounds = (4000, 4000);

    let mut imgen_loc = app.loc.scale(bounds, imgen_bounds);
    imgen_loc.comp.1 = 1.;

    let imgen_app = AppContext { loc: imgen_loc, holomorphic: app.holomorphic.clone() };
    let mat = imgen_app.to_ematrix(imgen_bounds);

    let _v = write_loc(&imgen_app);
    eprintln!("loc: {:?}", &_v);
    let _e = write_ematrix(&mat);
    eprintln!("ematrix: {:?}", &_e);
    unimplemented!()
}

fn write_ematrix(ematrix: &EMatrix) -> io::Result<()> {
    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let path = format!("mb-{}.png", unix_secs.as_secs() as u64);
    let img = ematrix.clone().into_img();
    img.save(path)
}

fn write_loc(app: &AppContext) -> std::io::Result<()> {
    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let path = format!("mb-{}.json", unix_secs.as_secs() as u64);

    let mut f = File::create(path)?;
    let buf = serde_json::to_string(&app.loc).unwrap();
    f.write_all(&buf.as_bytes())
}

fn main() -> std::result::Result<(), crate::Error> {
    // Terminal initialization
    let mut stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode().unwrap();
    let mut screen = AlternateScreen::from(stdout);

    let mut app = AppContext::default();
    write!(screen, "{}", ToAlternateScreen).unwrap();
    write!(screen, "{}", termion::cursor::Hide).unwrap();

    loop {
        let bounds = termion::terminal_size()?;
        draw_frame(&mut screen, &app, bounds)?;
        match (&mut stdin).keys().next() {
            Some(Ok(Key::Char('q'))) => break,

            // Zoom in keys - shift key is optional.
            Some(Ok(Key::Char('+'))) => app.loc.scalar /= 2.0,
            Some(Ok(Key::Char('='))) => app.loc.scalar /= 2.0,

            // Zoom out keys - shift key is optional.
            Some(Ok(Key::Char('-'))) => app.loc.scalar *= 2.0,
            Some(Ok(Key::Char('_'))) => app.loc.scalar *= 2.0,

            // Move left/right along the real axis.
            Some(Ok(Key::Char('a'))) => app.loc.re0 -= app.loc.scalar * 10.0,
            Some(Ok(Key::Char('d'))) => app.loc.re0 += app.loc.scalar * 10.0,

            // Move up and down on the imaginary axis.
            Some(Ok(Key::Char('w'))) => app.loc.im0 -= app.loc.scalar * 10.0,
            Some(Ok(Key::Char('s'))) => app.loc.im0 += app.loc.scalar * 10.0,

            // Increase the limit on iterations to escape.
            Some(Ok(Key::Char('t'))) => app.loc.max_iter += 25,
            Some(Ok(Key::Char('g'))) => app.loc.max_iter -= 25,

            // Reset to default.
            Some(Ok(Key::Char('m'))) => {
                std::mem::replace(&mut app.loc, Loc::default());
            }

            // Write the loc state to a JSON file.
            Some(Ok(Key::Char('p'))) => {
                screenshot(&app, bounds);
            }

            // Toggle between the Julia sets and the Mandelbrot sets.
            Some(Ok(Key::Char('x'))) => {
                let new_holo: Holomorphic;
                match app.holomorphic {
                    Holomorphic::Julia(ref j) => {
                        new_holo = Holomorphic::Mandelbrot(Mandelbrot::from(j));
                    }
                    Holomorphic::Mandelbrot(ref m) => {
                        let c = Complex64 {
                            re: app.loc.re0,
                            im: app.loc.im0,
                        };
                        new_holo = Holomorphic::Julia(Julia::from_c(m, c))
                    }
                }
                app.holomorphic = new_holo;
            }

            _ => {}
        }
    }

    write!(screen, "{}", ToMainScreen).unwrap();
    write!(screen, "{}", termion::cursor::Show).unwrap();
    screen.flush()?;

    Ok(())
}
