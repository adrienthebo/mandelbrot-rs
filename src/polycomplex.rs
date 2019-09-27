//! Polynomial complex functions

use crate::Escape;
use num::complex::Complex64;
use serde::{Deserialize, Serialize};

/// A complex polynomial function with a variable exponent.
pub trait ComplexFn {
    fn escape(&self, c: Complex64, limit: u32) -> Escape;
    fn exp(&self) -> f64;
    fn exp_mut(&mut self) -> &mut f64;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
                let fract = (z.norm_sqr().ln() / Self::ESCAPE_VALUE.ln()).ln() / self.exp.ln();
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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
                let fract = (z.norm_sqr().ln() / Self::ESCAPE_VALUE.ln()).ln() / self.exp.ln();
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
#[derive(Clone, Debug, Deserialize, Serialize)]
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

