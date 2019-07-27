extern crate itertools;
extern crate num;
extern crate termion;
extern crate rayon;

use itertools::Itertools;
use num::complex::Complex64;
use std::io::{self, Write};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::*;
use rayon::prelude::*;
use std::time::{Duration, Instant};

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

/// A given X/Y position and Z offset/magnification.
#[derive(Debug)]
struct Viewport {
    /// The x axis origin.
    pub im0: f64,

    /// The y axis origin.
    pub re0: f64,

    pub scalar: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            im0: 0.0,
            re0: 0.0,
            scalar: 0.1,
        }
    }
}

/// Compute the complex number for a given terminal position, viewport, and bounds.
fn complex_at(viewport: &Viewport, bounds: (u16, u16), pos: (u16, u16)) -> Complex64 {
    let origin: (i32, i32) = (i32::from(bounds.0 / 2), i32::from(bounds.1 / 2));
    let offset: (i32, i32) = (i32::from(pos.0) - origin.0, i32::from(pos.1) - origin.1);

    Complex64 {
        re: f64::from(offset.0) * viewport.scalar + viewport.re0,
        im: f64::from(offset.1) * viewport.scalar + viewport.im0,
    }
}

fn rgb(iterations: Option<u32>) -> termion::color::Rgb {
    let i = iterations.map(f64::from).unwrap_or(0_f64);

    let freq: f64 = 0.01;
    let coefficient: f64 = 255.;
    let offset: f64 = 129.;

    let rphase: f64 = 0.;
    let gphase: f64 = 2. * std::f64::consts::PI / 3.;
    let bphase: f64 = 4. * std::f64::consts::PI / 3.;

    let red = ((i * freq) + rphase).sin() * coefficient + offset;
    let green = ((i * freq) + gphase).sin() * coefficient + offset;
    let blue = ((i * freq) + bphase).sin() * coefficient + offset;

    termion::color::Rgb(red as u8, green as u8, blue as u8)
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
        iterations.map(|_| " ").unwrap_or("!")
    )
}

fn frame(viewport: &Viewport, bounds: (u16, u16)) -> String {
    let y_iter = 0..bounds.0;
    let x_iter = 0..bounds.1;

    y_iter
        .cartesian_product(x_iter)
        .collect::<Vec<(u16, u16)>>()
        .par_iter()
        .map(|pos| (pos, complex_at(&viewport, bounds, pos.clone())))
        .map(|(pos, c)| (pos, escapes(c, 500)))
        .map(|(pos, iter)| cell_ansi(pos.clone(), iter))
        .collect()
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
        let start: Instant = Instant::now();
        let buffer = frame(&viewport, termion::terminal_size().unwrap());
        let stop: Instant = Instant::now();
        let delta = stop - start;

        write!(screen, "{}", buffer).unwrap();
        write!(screen, "{}re = {:e}", termion::cursor::Goto(1, 1), viewport.re0).unwrap();
        write!(screen, "{}im = {:e}", termion::cursor::Goto(1, 2), viewport.im0).unwrap();
        write!(screen, "{}scalar = {:e}", termion::cursor::Goto(1, 3), viewport.scalar).unwrap();
        write!(screen, "{}duration = {}ms", termion::cursor::Goto(1, 4), delta.as_millis()).unwrap();
        screen.flush()?;

        match (&mut stdin).keys().next() {
            Some(Ok(Key::Char('q'))) => break,

            Some(Ok(Key::Char('+'))) => viewport.scalar /= 2.0,
            Some(Ok(Key::Char('-'))) => viewport.scalar *= 2.0,

            Some(Ok(Key::Char('a'))) => viewport.re0 -= viewport.scalar * 10.0,
            Some(Ok(Key::Char('d'))) => viewport.re0 += viewport.scalar * 10.0,

            Some(Ok(Key::Char('w'))) => viewport.im0 -= viewport.scalar * 10.0,
            Some(Ok(Key::Char('s'))) => viewport.im0 += viewport.scalar * 10.0,

            Some(Ok(Key::Char('r'))) => {
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
