//! Mandelbrot

extern crate itertools;
extern crate num;
extern crate rayon;
extern crate serde;

use num::complex::Complex64;
use serde::Serialize;
use std::io;

pub mod ematrix;
pub use ematrix::*;
pub mod loc;
pub use loc::*;
pub mod rctx;
pub use rctx::*;

#[derive(Debug)]
pub struct Error {
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
pub type Escape = Option<f64>;

/// The bounds for a given image, in column major order.
#[derive(Copy, Clone, Debug)]
pub struct Bounds {
    pub height: u16,
    pub width: u16,
}

impl Bounds {
    pub fn center(&self) -> Pos {
        Pos {
            x: self.width / 2,
            y: self.height / 2,
        }
    }
}

/// The dimensions of a TTY, in row major order.
///
/// This type will commonly be generated from `termion::terminal_size()`.
type TerminalSize = (u16, u16);

impl From<TerminalSize> for Bounds {
    fn from(ts: TerminalSize) -> Self {
        Self {
            width: ts.0,
            height: ts.1,
        }
    }
}

/// A position within some matrix, e.g. an escape matrix, color matrix, or image.
#[derive(Copy, Clone, Debug)]
pub struct Pos {
    pub x: u16,
    pub y: u16,
}

impl std::ops::Sub for Pos {
    type Output = Offset;

    fn sub(self, other: Pos) -> Self::Output {
        Self::Output {
            x: i32::from(self.x) - i32::from(other.x),
            y: i32::from(self.y) - i32::from(other.y),
        }
    }
}

impl std::ops::Add for Pos {
    type Output = Offset;

    fn add(self, other: Pos) -> Self::Output {
        Self::Output {
            x: i32::from(self.x) + i32::from(other.x),
            y: i32::from(self.y) + i32::from(other.y),
        }
    }
}

/// A position offset from an origin.
#[derive(Copy, Clone, Debug)]
pub struct Offset {
    pub x: i32,
    pub y: i32,
}

/// A position within a matrix or image, in row major order.
type PositionTuple = (u16, u16);

impl From<PositionTuple> for Pos {
    fn from(pt: PositionTuple) -> Self {
        Self { x: pt.0, y: pt.1 }
    }
}

/// A single color channel for HSV/RGB conversion.
#[derive(Debug, Clone)]
pub struct SineChannel {
    pub coef: f64,
    pub freq: f64,
    pub phase: f64,
    pub offset: f64,
}

impl SineChannel {
    const COEF: f64 = 127.;
    const FREQ: f64 = 0.05;
    const OFFSET: f64 = 127.;

    pub fn compute(&self, i: f64) -> u8 {
        (self.coef * ((i * self.freq) + self.phase).sin() + self.offset) as u8
    }

    pub fn sunset() -> (Self, Self, Self) {
        (
            Self {
                coef: Self::COEF,
                freq: Self::FREQ,
                phase: std::f64::consts::PI * 4. / 3.,
                offset: Self::OFFSET,
            },
            Self {
                coef: Self::COEF,
                freq: Self::FREQ,
                phase: std::f64::consts::PI * 5. / 3.,
                offset: Self::OFFSET,
            },
            Self {
                coef: Self::COEF,
                freq: Self::FREQ,
                phase: std::f64::consts::PI * 6. / 3.,
                offset: Self::OFFSET,
            },
        )
    }
}

#[derive(Debug, Clone)]
pub struct SineRGB {
    channels: (SineChannel, SineChannel, SineChannel),
}

impl Default for SineRGB {
    fn default() -> Self {
        Self {
            channels: SineChannel::sunset(),
        }
    }
}

impl SineRGB {
    /// Convert Mandelbrot escape iterations to an RGB value.
    ///
    /// Color is computed by representing (approximate) RGB values with 3 sine waves.
    ///
    /// Note: To produce true RGB the sine waves need to be 120 degrees (2pi/3) apart.
    /// Using a 60 degree phase offset produces some beautiful sunset colors, so this
    /// isn't a true RGB conversion. It delights me to inform the reader that in this
    /// case form trumps function, so deal with it.
    pub fn rgb(&self, escape: Escape) -> (u8, u8, u8) {
        match escape.map(|iters| f64::from(iters)) {
            None => (0, 0, 0),
            Some(i) => (
                self.channels.0.compute(i),
                self.channels.1.compute(i),
                self.channels.2.compute(i),
            ),
        }
    }
}

/// A complex polynomial function with a variable exponent.
pub trait ComplexFn {
    fn escape(&self, c: Complex64, limit: u32) -> Escape;
    fn exp(&self) -> f64;
    fn exp_mut(&mut self) -> &mut f64;
}

#[derive(Clone, Debug, Serialize)]
pub struct Mandelbrot {
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
    const ESCAPE_VALUE: f64 = 8.;

    pub fn render(&self, c: Complex64, limit: u32) -> Escape {
        let mut z = Complex64 { re: 0.0, im: 0.0 };
        for i in 0..limit {
            z = z.powf(self.exp);
            z += c;
            if z.norm_sqr() > Self::ESCAPE_VALUE {
                let fract = ((z.norm_sqr().ln() / Self::ESCAPE_VALUE.ln())).ln() / self.exp.ln();
                return Some(f64::from(i) - fract);
            }
        }

        return None;
    }
}

impl ComplexFn for Mandelbrot {
    fn escape(&self, c: Complex64, limit: u32) -> Escape {
        self.render(c, limit)
    }

    fn exp(&self) -> f64 {
       self.exp
    }

    fn exp_mut(&mut self) -> &mut f64 {
        &mut self.exp
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Julia {
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
    const ESCAPE_VALUE: f64 = 8.;

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
            z = z.powf(self.exp);
            z += self.c_offset;
            if z.norm_sqr() > Self::ESCAPE_VALUE {
                let fract = ((z.norm_sqr().ln() / Self::ESCAPE_VALUE.ln())).ln() / self.exp.ln();
                return Some(f64::from(i) - fract);
            }
        }

        return None;
    }
}

impl ComplexFn for Julia {
    fn escape(&self, c: Complex64, limit: u32) -> Escape {
        self.render(c, limit)
    }

    fn exp(&self) -> f64 {
       self.exp
    }

    fn exp_mut(&mut self) -> &mut f64 {
        &mut self.exp
    }
}

/// A polynomial complex-valued function.
///
/// At present this represents either the Mandelbrot set or a Julia set, and provides a common
/// interface to generating and manipulating the functions generating these sets.
#[derive(Clone, Debug, Serialize)]
pub enum PolyComplexFn {
    Julia(Julia),
    Mandelbrot(Mandelbrot),
}

impl PolyComplexFn {
    pub fn render(&self, c: Complex64, limit: u32) -> Escape {
        match self {
            PolyComplexFn::Julia(j) => j.render(c, limit),
            PolyComplexFn::Mandelbrot(m) => m.render(c, limit),
        }
    }
}

impl Default for PolyComplexFn {
    fn default() -> Self {
        PolyComplexFn::Mandelbrot(Mandelbrot::default())
    }
}

impl ComplexFn for PolyComplexFn {
    fn escape(&self, c: Complex64, limit: u32) -> Escape {
        self.render(c, limit)
    }

    fn exp(&self) -> f64 {
        match self {
            PolyComplexFn::Mandelbrot(ref m) => m.exp,
            PolyComplexFn::Julia(ref j) => j.exp,
        }
    }

    fn exp_mut(&mut self) -> &mut f64 {
        match self {
            PolyComplexFn::Mandelbrot(ref mut m) => &mut m.exp,
            PolyComplexFn::Julia(ref mut j) => &mut j.exp,
        }
    }
}
