#![allow(unused)]
// TODO: remove this

extern crate itertools;
extern crate num;
extern crate termion;
extern crate rayon;
extern crate nalgebra;

use itertools::Itertools;
use num::complex::Complex64;
use std::io::{self, Write};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::*;
use rayon::prelude::*;
use std::time::Instant;

#[derive(Debug)]
struct Error {
    source: Option<Box<dyn std::error::Error>>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Errmagerrd {:?}", self.source)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| &**e)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self {
            source: Some(Box::new(err)),
        }
    }
}

type Escape = Option<u32>;
type EMatrix = nalgebra::DMatrix<Escape>;

#[derive(Debug)]
struct Mandelbrot {
    pub exp: f64,
}

impl Default for Mandelbrot {
    fn default() -> Self {
        Mandelbrot { exp: 2. }
    }
}

impl From<Julia> for Mandelbrot {
    fn from(j: Julia) -> Self {
        Mandelbrot { exp: j.exp }
    }
}

impl Mandelbrot {
    pub fn render(&self, c: Complex64, limit: u32) -> Escape {
        let mut z = Complex64 { re: 0.0, im: 0.0 };
        for i in 0..limit {
            z *= z;
            z += c;
            if z.norm_sqr() > 4.0 {
                return Some(i);
            }
        }

        return None;
    }
}

#[derive(Debug)]
struct Julia {
    pub exp: f64,
    pub c: Complex64,
}

impl Default for Julia {
    fn default() -> Self {
        Julia { exp: 2., c: Complex64 { re: 0.6, im: 0.4 } }
    }
}

impl Julia {
    /// Create a Julia set with a given mandelbrot algorithm and
    /// re/im coordinates.
    pub fn from_mandelbrot(m: Mandelbrot, c: Complex64) -> Self {
        Julia { exp: m.exp, c }
    }

    fn render(&self, c: Complex64, limit: u32) -> Escape {
        let mut z = c.clone();
        for i in 0..limit {
            z *= z;
            z += Complex64 { re: -1.5, im: -0.2 };
            if z.norm_sqr() > 4.0 {
                return Some(i);
            }
        }

        return None;
    }
}

/// A complex-valued function that is locally differentiable.
///
/// In more reasonable terms, this is either a Julia set or a Mandelbrot set.
#[derive(Debug)]
enum Holomorphic {
    Julia(Julia),
    Mandelbrot(Mandelbrot)
}

impl Holomorphic {
    pub fn render(&self, c: Complex64, limit: u32) -> Escape {
        match self {
            Holomorphic::Julia(j) => j.render(c, limit),
            Holomorphic::Mandelbrot(m) => m.render(c, limit),

        }
    }
}

impl Default for Holomorphic {
    fn default() -> Self {
        Holomorphic::Mandelbrot(Mandelbrot::default())
    }
}

#[derive(Debug)]
enum Algorithm {
    Julia { exp: f64, c: Complex64 },
    Mandelbrot { exp: f64 },
}

/// The rendering context or view for a given position.
#[derive(Debug)]
struct Viewport {
    /// The x axis origin.
    pub im0: f64,

    /// The y axis origin.
    pub re0: f64,

    /// Magnification/zoom factor.
    pub scalar: f64,

    /// The maximum iterations before declaring a complex does not converge.
    pub max_iter: u32,

    pub holomorphic: Holomorphic,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            im0: 0.0,
            re0: 0.0,
            scalar: 0.1,
            max_iter: 100,
            holomorphic: Holomorphic::default()
        }
    }
}

/// Compute the complex number for a given terminal position, viewport, and bounds.
fn complex_at(viewport: &Viewport, bounds: (u16, u16), pos: (u16, u16)) -> Complex64 {
    let origin: (i32, i32) = (i32::from(bounds.0 / 2), i32::from(bounds.1 / 2));
    let offset: (i32, i32) = (i32::from(pos.0) - origin.0, i32::from(pos.1) - origin.1);

    Complex64 {
        re: f64::from(offset.0) * viewport.scalar + viewport.re0,
        // Hack - the doubling compensates for terminal cell x/y variation
        im: 2. * f64::from(offset.1) * viewport.scalar + viewport.im0,
    }
}

/// Convert Mandelbrot escape iterations to an RGB value.
///
/// Color is computed by representing (approximate) RGB values with 3 sine waves.
///
/// Note: To produce true RGB the sine waves need to be 120 degrees (2pi/3) apart.
/// Using a 60 degree phase offset produces some beautiful sunset colors, so this
/// isn't a true RGB conversion. It delights me to inform the reader that in this
/// case form trumps function, so deal with it.
fn rgb(iterations: Escape) -> termion::color::Rgb {
    match iterations.map(|i| f64::from(i)) {
        None => termion::color::Rgb(0, 0, 0),
        Some(i) => {
            let freq: f64 = 0.1;
            let coefficient: f64 = 127.;
            let offset: f64 = 127.;

            let rphase: f64 = 0.;
            let gphase: f64 = std::f64::consts::PI / 3.;
            let bphase: f64 = std::f64::consts::PI * 2. / 3.;

            let red = ((i * freq) + rphase).sin() * coefficient + offset;
            let green = ((i * freq) + gphase).sin() * coefficient + offset;
            let blue = ((i * freq) + bphase).sin() * coefficient + offset;

            termion::color::Rgb(red as u8, green as u8, blue as u8)
        }
    }
}

/// Given XY coordinates and computed mandelbrot iteration,
/// compute the necessary ANSI to move the cursor and paint the cell.
///
/// Note: generating strings for every element is highly inefficient; we
/// should really be appending to a string slice. :shrug:
fn cell_ansi(pos: (u16, u16), iterations: Escape) -> String {
    format!(
        "{}{}{}",
        termion::cursor::Goto(pos.0 + 1, pos.1 + 1),
        termion::color::Bg(rgb(iterations)),
        " "
    )
}

fn escape_matrix(viewport: &Viewport, bounds: (u16, u16)) -> EMatrix {
    let y_iter = 0..bounds.0;
    let x_iter = 0..bounds.1;

    let escapes: Vec<Escape> = y_iter
        .cartesian_product(x_iter)
        .collect::<Vec<(u16, u16)>>()
        .par_iter()
        .map(|pos| complex_at(&viewport, bounds, pos.clone()))
        .map(|c| viewport.holomorphic.render(c, viewport.max_iter))
        .collect();


    EMatrix::from_vec(usize::from(bounds.0), usize::from(bounds.1), escapes)
}

fn ematrix_to_frame(mat: EMatrix, bounds: (u16, u16)) -> String {
    let y_iter = 0..bounds.0;
    let x_iter = 0..bounds.1;

    y_iter
        .cartesian_product(x_iter)
        .zip(mat.iter())
        .map(move |(pos, escape)| cell_ansi(pos, *escape))
        .collect()
}

/// Given a viewport and bounds, render the ANSI sequences to draw the mandelbrot
/// fractal.
///
/// Note: This function performs too many heap allocations by casually using Strings
/// and Vectors. This would perform better by writing to a pre-allocated `&str`.
fn frame(viewport: &Viewport, bounds: (u16, u16)) -> String {
    ematrix_to_frame(escape_matrix(&viewport, bounds), bounds)
}

fn main() -> Result<(), Error> {
    // Terminal initialization
    let mut stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode().unwrap();
    let mut screen = AlternateScreen::from(stdout);

    let mut viewport = Viewport::default();
    write!(screen, "{}", ToAlternateScreen).unwrap();
    write!(screen, "{}", termion::cursor::Hide).unwrap();

    loop {
        let render_start: Instant = Instant::now();
        let buffer = frame(&viewport, termion::terminal_size().unwrap());
        let render_stop: Instant = Instant::now();
        let render_delta = render_stop - render_start;

        let draw_start = Instant::now();
        write!(screen, "{}", buffer).unwrap();
        screen.flush()?;
        let draw_stop = Instant::now();
        let draw_delta = draw_stop - draw_start;

        let labels = vec![
            format!("re     = {:e}", viewport.re0),
            format!("im     = {:e}", viewport.im0),
            format!("iter   = {}", viewport.max_iter),
            format!("scalar = {:e}", viewport.scalar),
            format!("render = {}ms", render_delta.as_millis()),
            format!("draw   = {}ms", draw_delta.as_millis())
        ];

        for (offset, label) in labels.iter().enumerate() {
            write!(screen, "{}{}{}",
                   termion::cursor::Goto(1, offset as u16 + 1),
                   termion::style::Reset,
                   label).unwrap();
        }

        screen.flush()?;

        match (&mut stdin).keys().next() {
            Some(Ok(Key::Char('q'))) => break,

            // Zoom in keys - shift key is optional.
            Some(Ok(Key::Char('+'))) => viewport.scalar /= 2.0,
            Some(Ok(Key::Char('='))) => viewport.scalar /= 2.0,

            // Zoom out keys - shift key is optional.
            Some(Ok(Key::Char('-'))) => viewport.scalar *= 2.0,
            Some(Ok(Key::Char('_'))) => viewport.scalar *= 2.0,

            Some(Ok(Key::Char('a'))) => viewport.re0 -= viewport.scalar * 10.0,
            Some(Ok(Key::Char('d'))) => viewport.re0 += viewport.scalar * 10.0,

            Some(Ok(Key::Char('w'))) => viewport.im0 -= viewport.scalar * 10.0,
            Some(Ok(Key::Char('s'))) => viewport.im0 += viewport.scalar * 10.0,

            Some(Ok(Key::Char('t'))) => viewport.max_iter += 25,
            Some(Ok(Key::Char('g'))) => viewport.max_iter -= 25,

            Some(Ok(Key::Char('m'))) => {
                std::mem::replace(&mut viewport, Viewport::default());
            },
            _ => {}
        }
    }

    write!(screen, "{}", ToMainScreen).unwrap();
    write!(screen, "{}", termion::cursor::Show).unwrap();
    screen.flush()?;

    Ok(())
}
