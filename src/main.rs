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
    source: Option<Box<std::error::Error>>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Errmagerrd {:?}", self.source)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        unimplemented!()
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self {
            source: Some(Box::new(err)),
        }
    }
}

/// Try to determine whether the complex number `c` is in the Mandelbrot set.
///
/// A number `c` is in the set if, starting with zero, repeatedly squaring and
/// adding `c` never causes the number to leave the circle of radius 2 centered
/// on the origin; the number instead orbits near the origin forever. (If the
/// number does leave the circle, it eventually flies away to infinity.)
///
/// If after `limit` iterations our number has still not left the circle, return
/// `None`; this is as close as we come to knowing that `c` is in the set.
///
/// If the number does leave the circle before we give up, return `Some(i)`, where
/// `i` is the number of iterations it took.
///
/// This function was copied from https://github.com/ProgrammingRust/mandelbrot/blob/3b5d168b8746ecde18d17e39e01cd6d879ee61c4/src/main.rs#L67
fn escapes(c: Complex64, limit: u32) -> Option<u32> {
    let mut z = Complex64 { re: 0.0, im: 0.0 };
    for i in 0..limit {
        z = z * z + c;
        if z.norm_sqr() > 4.0 {
            return Some(i);
        }
    }

    return None;
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
    pub max_iter: u32
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            im0: 0.0,
            re0: 0.0,
            scalar: 0.1,
            max_iter: 100
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
fn rgb(iterations: Option<u32>) -> termion::color::Rgb {
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
fn cell_ansi(pos: (u16, u16), iterations: Option<u32>) -> String {
    format!(
        "{}{}{}",
        termion::cursor::Goto(pos.0 + 1, pos.1 + 1),
        termion::color::Bg(rgb(iterations)),
        " "
    )
}

type Escape = Option<u32>;
type EMatrix = nalgebra::DMatrix<Escape>;

fn escape_matrix(viewport: &Viewport, bounds: (u16, u16)) -> EMatrix {
    let y_iter = 0..bounds.0;
    let x_iter = 0..bounds.1;

    let escapes: Vec<Escape> = y_iter
        .cartesian_product(x_iter)
        .collect::<Vec<(u16, u16)>>()
        .par_iter()
        .map(|pos| complex_at(&viewport, bounds, pos.clone()))
        .map(|c| escapes(c, viewport.max_iter))
        .collect();


    EMatrix::from_vec(usize::from(bounds.0), usize::from(bounds.1), escapes)
}

fn ematrix_to_frame(viewport: &Viewport, bounds: (u16, u16)) -> String {
    let mat = escape_matrix(viewport, bounds);

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
    ematrix_to_frame(&viewport, bounds)
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

        write!(screen, "{}re = {:e}", termion::cursor::Goto(1, 1), viewport.re0).unwrap();
        write!(screen, "{}im = {:e}", termion::cursor::Goto(1, 2), viewport.im0).unwrap();
        write!(screen, "{}max_iter = {}", termion::cursor::Goto(1, 3), viewport.max_iter).unwrap();
        write!(screen, "{}scalar = {:e}", termion::cursor::Goto(1, 4), viewport.scalar).unwrap();
        write!(screen, "{}render = {}ms", termion::cursor::Goto(1, 5), render_delta.as_millis()).unwrap();
        write!(screen, "{}draw = {}ms", termion::cursor::Goto(1, 6), draw_delta.as_millis()).unwrap();
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
