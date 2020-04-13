//! Mandelbrot

extern crate indicatif;
extern crate itertools;
extern crate num;
extern crate rayon;
extern crate serde;

use serde::{Deserialize, Serialize};
use std::io;

pub mod ematrix;
pub mod frontend;
pub mod loc;
pub mod polycomplex;
pub mod rctx;
pub use polycomplex::*;

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

impl From<serde_json::error::Error> for Error {
    fn from(err: serde_json::error::Error) -> Self {
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SineChannel {
    pub coef: f64,
    pub freq: f64,
    pub phase: f64,
    pub offset: f64,
}

fn saturate_channel(i: f64) -> u8 {
    match i {
        i if i < 0. => 0,
        i if i > 255. => 255,
        i => i as u8,
    }
}

impl SineChannel {
    const COEF: f64 = 140.;
    const FREQ: f64 = 0.1;
    const OFFSET: f64 = 112.;

    pub fn compute(&self, i: f64) -> u8 {
        let input = self.coef * ((i * self.freq) + self.phase).sin() + self.offset;
        saturate_channel(input)
    }

    pub fn sunset() -> (Self, Self, Self) {
        (
            Self {
                coef: Self::COEF,
                freq: Self::FREQ,
                phase: std::f64::consts::PI * 9. / 6.,
                offset: Self::OFFSET,
            },
            Self {
                coef: Self::COEF,
                freq: Self::FREQ,
                phase: std::f64::consts::PI * 10. / 6.,
                offset: Self::OFFSET,
            },
            Self {
                coef: Self::COEF,
                freq: Self::FREQ,
                phase: std::f64::consts::PI * 11. / 6.,
                offset: Self::OFFSET,
            },
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
