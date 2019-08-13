extern crate image;
extern crate itertools;
extern crate nalgebra;
extern crate num;
extern crate rayon;
extern crate serde;
extern crate termion;

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

/// An Escape represents the status of an evaluated point's escape iteration.
type Escape = Option<u32>;

/// An EMatrix maps the cells in a frame to corresponding evaluated escapes.
#[derive(Debug, Clone)]
struct EMatrix(nalgebra::DMatrix<Escape>);

impl EMatrix {
    pub fn from_vec(ncols: usize, nrows: usize, v: Vec<Escape>) -> EMatrix {
        let mat = nalgebra::DMatrix::from_vec(ncols, nrows, v);
        Self(mat)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Escape> {
        self.0.get_mut(index)
    }

    pub fn get(&self, index: usize) -> Option<&Escape> {
        self.0.get(index)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> EMatrixRefIterator {
        EMatrixRefIterator { mat: self, index: 0 }
    }

    pub fn into_img(self) -> image::RgbImage {
        let mat = self.0;
        image::RgbImage::from_fn(mat.ncols() as u32, mat.nrows() as u32, move |x, y| {
            let escape = mat.index((y as usize, x as usize));
            let term_rgb = rgb(*escape);
            image::Rgb([term_rgb.0, term_rgb.1, term_rgb.2])
        })
    }
}

struct EMatrixIterator {
    mat: EMatrix,
    index: usize,
}

impl std::iter::Iterator for EMatrixIterator {
    type Item = Escape;

    fn next(&mut self) -> Option<Escape> {
        if self.index >= self.mat.len() {
            None
        } else {
            let esc: Escape = self.mat.0.index(self.index).clone(); // XXX bad memory allocation
            self.index += 1;
            Some(esc)
        }
    }
}

impl std::iter::IntoIterator for EMatrix {
    type Item = Escape;
    type IntoIter = EMatrixIterator;

    fn into_iter(self) -> Self::IntoIter {
        EMatrixIterator {
            mat: self,
            index: 0
        }
    }
}

struct EMatrixRefIterator<'a> {
    mat: &'a EMatrix,
    index: usize
}

impl<'a> std::iter::Iterator for EMatrixRefIterator<'a> {
    type Item = &'a Escape;

    fn next(&mut self) -> Option<&'a Escape> {
        if self.index >= self.mat.len() {
            None
        } else {
            let esc: &Escape = self.mat.0.index(self.index);
            self.index += 1;
            Some(esc)
        }
    }
}

impl<'a> IntoIterator for &'a EMatrix {
    type Item = &'a Escape;
    type IntoIter = EMatrixRefIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        EMatrixRefIterator { mat: self, index: 0 }
    }
}

#[derive(Clone, Debug, Serialize)]
struct Mandelbrot {
    pub exp: f64,
}

impl Default for Mandelbrot {
    fn default() -> Self {
        Mandelbrot { exp: 2. }
    }
}

impl From<&Julia> for Mandelbrot {
    fn from(j: &Julia) -> Self {
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

#[derive(Clone, Debug, Serialize)]
struct Julia {
    pub exp: f64,
    pub c_offset: Complex64,
}

impl Default for Julia {
    fn default() -> Self {
        Julia {
            exp: 2.,
            c_offset: Complex64 { re: 0.6, im: 0.4 },
        }
    }
}

impl Julia {
    /// Create a Julia set with a given mandelbrot algorithm and
    /// re/im coordinates.
    pub fn from_c(m: &Mandelbrot, c_offset: Complex64) -> Self {
        Julia {
            exp: m.exp,
            c_offset: c_offset,
        }
    }

    fn render(&self, c: Complex64, limit: u32) -> Escape {
        let mut z = c.clone();
        for i in 0..limit {
            z *= z;
            z += self.c_offset;
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
#[derive(Clone, Debug, Serialize)]
enum Holomorphic {
    Julia(Julia),
    Mandelbrot(Mandelbrot),
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

/// The rendering context or view for a given position.
#[derive(Clone, Debug, Serialize)]
struct Viewport {
    /// The x axis origin.
    pub im0: f64,

    /// The y axis origin.
    pub re0: f64,

    /// Dimensional scaling factors in case the canvas is not square.
    ///
    /// This compensates for terminal cells having a 2:1 ratio.
    pub comp: (f64, f64),

    /// Magnification/zoom factor.
    pub scalar: f64,

    /// The maximum iterations before declaring a complex does not converge.
    pub max_iter: u32,

    /// The active holomorphic function.
    pub holomorphic: Holomorphic,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            im0: 0.0,
            re0: 0.0,
            comp: (1., 2.),
            scalar: 0.1,
            max_iter: 100,
            holomorphic: Holomorphic::default(),
        }
    }
}

/// Compute the complex number for a given terminal position, viewport, and bounds.
fn complex_at(viewport: &Viewport, bounds: (u16, u16), pos: (u16, u16)) -> Complex64 {
    let origin: (i32, i32) = (i32::from(bounds.0 / 2), i32::from(bounds.1 / 2));
    let offset: (i32, i32) = (i32::from(pos.0) - origin.0, i32::from(pos.1) - origin.1);

    Complex64 {
        re: viewport.comp.0 * f64::from(offset.0) * viewport.scalar + viewport.re0,
        // Hack - the doubling compensates for terminal cell x/y variation
        // XXX THIS REALLY NEEDS TO BE ADDRESSED IN A MORE FLEXIBLE FASHION.
        //im: 2. * f64::from(offset.1) * viewport.scalar + viewport.im0,
        im: viewport.comp.1 * f64::from(offset.1) * viewport.scalar + viewport.im0,
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
            let freq: f64 = 0.05;
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

fn ematrix_to_frame(mat: &EMatrix, bounds: (u16, u16)) -> String {
    let y_iter = 0..bounds.0;
    let x_iter = 0..bounds.1;

    y_iter
        .cartesian_product(x_iter)
        .zip(mat.iter())
        .map(move |(pos, escape)| cell_ansi(pos, *escape))
        .collect()
}

fn draw_frame<W: Write>(screen: &mut W, viewport: &Viewport) -> Result<(), crate::Error> {
    let bounds = termion::terminal_size()?;

    let render_start: Instant = Instant::now();
    let mat = escape_matrix(&viewport, bounds);
    let buffer = ematrix_to_frame(&mat, bounds);
    let render_stop: Instant = Instant::now();

    let draw_start = Instant::now();
    write!(screen, "{}", buffer).unwrap();
    screen.flush()?;
    let draw_stop = Instant::now();

    let render_delta = render_stop - render_start;
    let draw_delta = draw_stop - draw_start;

    let labels = vec![
        format!("viewport = {:?}", &viewport),
        format!("re     = {:.4e}", viewport.re0),
        format!("im     = {:.4e}", viewport.im0),
        format!("iter   = {}", viewport.max_iter),
        format!("scalar = {:.4e}", viewport.scalar),
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

#[allow(unused)]
fn write_ematrix(ematrix: &EMatrix) -> io::Result<()> {
    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let path = format!("mb-{}.png", unix_secs.as_secs() as u64);
    let img = ematrix.clone().into_img();
    img.save(path)
}

fn write_viewport(viewport: &Viewport) -> std::io::Result<()> {
    let unix_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let path = format!("mb-{}.json", unix_secs.as_secs() as u64);

    let mut f = File::create(path)?;
    let buf = serde_json::to_string(&viewport).unwrap();
    f.write_all(&buf.as_bytes())
}

fn main() -> std::result::Result<(), crate::Error> {
    // Terminal initialization
    let mut stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode().unwrap();
    let mut screen = AlternateScreen::from(stdout);

    let mut viewport = Viewport::default();
    write!(screen, "{}", ToAlternateScreen).unwrap();
    write!(screen, "{}", termion::cursor::Hide).unwrap();

    loop {
        draw_frame(&mut screen, &viewport)?;
        match (&mut stdin).keys().next() {
            Some(Ok(Key::Char('q'))) => break,

            // Zoom in keys - shift key is optional.
            Some(Ok(Key::Char('+'))) => viewport.scalar /= 2.0,
            Some(Ok(Key::Char('='))) => viewport.scalar /= 2.0,

            // Zoom out keys - shift key is optional.
            Some(Ok(Key::Char('-'))) => viewport.scalar *= 2.0,
            Some(Ok(Key::Char('_'))) => viewport.scalar *= 2.0,

            // Move left/right along the real axis.
            Some(Ok(Key::Char('a'))) => viewport.re0 -= viewport.scalar * 10.0,
            Some(Ok(Key::Char('d'))) => viewport.re0 += viewport.scalar * 10.0,

            // Move up and down on the imaginary axis.
            Some(Ok(Key::Char('w'))) => viewport.im0 -= viewport.scalar * 10.0,
            Some(Ok(Key::Char('s'))) => viewport.im0 += viewport.scalar * 10.0,

            // Increase the limit on iterations to escape.
            Some(Ok(Key::Char('t'))) => viewport.max_iter += 25,
            Some(Ok(Key::Char('g'))) => viewport.max_iter -= 25,

            // Reset to default.
            Some(Ok(Key::Char('m'))) => {
                std::mem::replace(&mut viewport, Viewport::default());
            }

            // Write the viewport state to a JSON file.
            Some(Ok(Key::Char('p'))) => {
                // TODO: handle write errors without panicking.
                let mut zoomed_viewport = viewport.clone();
                zoomed_viewport.scalar *= 0.05;
                zoomed_viewport.comp.1 = 1.;
                let mat = escape_matrix(&zoomed_viewport, (4000, 4000));
                let _v = write_viewport(&zoomed_viewport);
                eprintln!("viewport: {:?}", &_v);
                let _e = write_ematrix(&mat);
                eprintln!("ematrix: {:?}", &_e);
            }

            // Toggle between the Julia sets and the Mandelbrot sets.
            Some(Ok(Key::Char('x'))) => {
                let new_holo: Holomorphic;
                match viewport.holomorphic {
                    Holomorphic::Julia(ref j) => {
                        new_holo = Holomorphic::Mandelbrot(Mandelbrot::from(j));
                    }
                    Holomorphic::Mandelbrot(ref m) => {
                        let c = Complex64 {
                            re: viewport.re0,
                            im: viewport.im0,
                        };
                        new_holo = Holomorphic::Julia(Julia::from_c(m, c))
                    }
                }
                viewport.holomorphic = new_holo;
            }

            _ => {}
        }
    }

    write!(screen, "{}", ToMainScreen).unwrap();
    write!(screen, "{}", termion::cursor::Show).unwrap();
    screen.flush()?;

    Ok(())
}
