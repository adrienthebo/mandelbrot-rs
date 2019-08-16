//! Mandelbrot

extern crate num;
extern crate serde;

use num::complex::Complex64;
use serde::Serialize;
use std::io;

pub mod ematrix;
pub use ematrix::*;

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
pub type Escape = Option<u32>;

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
                phase: 0.,
                offset: Self::OFFSET,
            },
            Self {
                coef: Self::COEF,
                freq: Self::FREQ,
                phase: std::f64::consts::PI / 3.,
                offset: Self::OFFSET,
            },
            Self {
                coef: Self::COEF,
                freq: Self::FREQ,
                phase: std::f64::consts::PI * 2. / 3.,
                offset: Self::OFFSET,
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct SineRGB {
    channels: (SineChannel, SineChannel, SineChannel),
}

impl SineRGB {
    fn rgb(&self, escape: Escape) -> (u8, u8, u8) {
        match escape.map(|iters| f64::from(iters)) {
            None => (0,0,0),
            Some(i) => {
                (
                    self.channels.0.compute(i),
                    self.channels.1.compute(i),
                    self.channels.2.compute(i),
                )
            }
        }
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
pub fn rgb(iterations: Escape) -> termion::color::Rgb {
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
pub enum Holomorphic {
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
