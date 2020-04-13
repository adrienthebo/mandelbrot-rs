//! The set of state needed to render a complex polynomial function to an image.
//!
//! The rendering context binds together a complex polynomial function and location.
//!
//! Remaining work:
//!
//! - Add a related type that binds a rendering context with a specific bounds.

use crate::{
    ematrix::EMatrix, loc::Loc, Bounds, ComplexFn, Escape, Julia, Mandelbrot, PolyComplexFn, Pos,
};
use indicatif::ParallelProgressIterator;
use itertools::Itertools;
use num::complex::Complex64;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::ops::Index;

/// The context for rending a specific point or region within a fractal.
///
/// An `Rctx` gives magnitude, scaling factors, and other properties for the image but does
/// not bound the precise dimensions.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rctx {
    /// The current loc.
    pub loc: Loc,

    /// The active complex polynomial function.
    pub complexfn: PolyComplexFn,

    /// The colorer for individual escapes.
    pub colorer: crate::SineRGB,

    /// Dimensional scaling factors in case the canvas is not square.
    ///
    /// This compensates for terminal cells having a 2:1 ratio.
    pub comp: (f64, f64),
}

impl Rctx {
    const TRANSLATE_SCALAR: f64 = 10.;
    const SCALE_SCALAR: f64 = 2.;
    const ITERATIONS_SCALAR: u32 = 25;
    const EXP_SCALAR: f64 = 0.001;

    pub fn bind<'a>(&'a self, bounds: Bounds) -> BoundRctx<'a> {
        BoundRctx {
            rctx: &self,
            bounds,
        }
    }

    /// Create a new application context with a pre-defined location.
    pub fn with_loc(loc: Loc) -> Self {
        let mut rctx = Rctx::default();
        rctx.loc = loc;
        rctx
    }

    /// Determine the complex value at a given offset of the origin with respect to the provided
    /// bounds.
    pub fn complex_at(&self, bounds: Bounds, pos: Pos) -> Complex64 {
        let offset = pos - bounds.center();

        Complex64 {
            im: self.comp.0 * f64::from(offset.y) * self.loc.scalar + self.loc.im0,
            re: self.comp.1 * f64::from(offset.x) * self.loc.scalar + self.loc.re0,
        }
    }

    /// Apply a transform to the rctx.
    pub fn transform(&mut self, transform: &RctxTransform) {
        match *transform {
            RctxTransform::TranslateUp => self.loc.im0 -= self.loc.scalar * Self::TRANSLATE_SCALAR,
            RctxTransform::TranslateDown => {
                self.loc.im0 += self.loc.scalar * Self::TRANSLATE_SCALAR
            }
            RctxTransform::TranslateLeft => {
                self.loc.re0 -= self.loc.scalar * Self::TRANSLATE_SCALAR
            }
            RctxTransform::TranslateRight => {
                self.loc.re0 += self.loc.scalar * Self::TRANSLATE_SCALAR
            }

            RctxTransform::IncIterations => self.loc.max_iter += Self::ITERATIONS_SCALAR,
            RctxTransform::DecIterations => self.loc.max_iter -= Self::ITERATIONS_SCALAR,

            RctxTransform::ScaleIn => self.loc.scalar /= Self::SCALE_SCALAR,
            RctxTransform::ScaleOut => self.loc.scalar *= Self::SCALE_SCALAR,

            RctxTransform::Reset => {
                // TODO: use `Loc::for_bounds()` for appropriate zoom selection
                std::mem::replace(&mut self.loc, Loc::default());
            }

            RctxTransform::IncExp => {
                *self.complexfn.exp_mut() += Self::EXP_SCALAR;
            }
            RctxTransform::DecExp => {
                *self.complexfn.exp_mut() -= Self::EXP_SCALAR;
            }

            RctxTransform::SwitchFn => {
                let new_fn: PolyComplexFn;
                match self.complexfn {
                    PolyComplexFn::Julia(ref j) => {
                        new_fn = PolyComplexFn::Mandelbrot(Mandelbrot::from(j));
                        // When switching from a Julia fractal to the mandelbrot fractal, we need
                        // to change the location specified in the Julia offset. This allows the
                        // user to switch back and forth between the two fractals to observe how
                        // Julia fractals change as the position in the mandelbrot set changes.
                        self.loc.move_to(j.c_offset);
                    }
                    PolyComplexFn::Mandelbrot(ref m) => {
                        // When switching from the mandelbrot fractal to a Julia fractal, the
                        // current position generally maps to a similar looking position. The
                        // location can be preserved.
                        new_fn = PolyComplexFn::Julia(Julia::from_c(m, self.loc.origin()))
                    }
                }
                self.complexfn = new_fn;
            }
        }
    }

    /// Create a cell rendering context with compensations for terminal cell sizes
    pub fn for_terminal(loc: Option<Loc>) -> Self {
        Self {
            loc: loc.unwrap_or(Loc::default()),
            comp: (2.3, 1.),
            .. Self::default()
        }
    }
}

impl Default for Rctx {
    fn default() -> Self {
        Self {
            loc: Loc::default(),
            complexfn: PolyComplexFn::default(),
            colorer: crate::SineRGB::default(),
            comp: (1., 1.),
        }
    }
}

/// A rendering context with the given bounds.
pub struct BoundRctx<'a> {
    pub rctx: &'a Rctx,
    pub bounds: Bounds,
}

impl<'a> BoundRctx<'a> {
    pub fn to_ematrix(&self) -> EMatrix {
        let y_iter = 0..self.bounds.height;
        let x_iter = 0..self.bounds.width;

        let escapes: Vec<Escape> = x_iter
            .cartesian_product(y_iter)
            .map(|pt| Pos::from(pt))
            .collect::<Vec<Pos>>()
            .par_iter()
            .map(|pos| self.rctx.complex_at(self.bounds, *pos))
            .map(|c| self.rctx.complexfn.escape(c, self.rctx.loc.max_iter))
            .collect();

        EMatrix::from_vec(
            usize::from(self.bounds.height),
            usize::from(self.bounds.width),
            escapes,
        )
    }

    pub fn to_ematrix_with_bar(&self, bar: indicatif::ProgressBar) -> EMatrix {
        let y_iter = 0..self.bounds.height;
        let x_iter = 0..self.bounds.width;

        bar.set_length(self.bounds.height as u64 * self.bounds.width as u64);

        let escapes: Vec<Escape> = x_iter
            .cartesian_product(y_iter)
            .map(|pt| Pos::from(pt))
            .collect::<Vec<Pos>>()
            .par_iter()
            .progress_with(bar)
            .map(|pos| self.rctx.complex_at(self.bounds, *pos))
            .map(|c| self.rctx.complexfn.escape(c, self.rctx.loc.max_iter))
            .collect();

        EMatrix::from_vec(
            usize::from(self.bounds.height),
            usize::from(self.bounds.width),
            escapes,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RctxTransform {
    /// Translate the image upward, ie decrement loc.im0
    TranslateUp,
    /// Translate the image downward, ie increment loc.im0
    TranslateDown,
    /// Translate the image left, ie decrement loc.re0
    TranslateLeft,
    /// Translate the image right, ie increment loc.re0
    TranslateRight,
    /// Increase the scale factor
    ScaleIn,
    /// Decrease the scale factor
    ScaleOut,
    /// Increment the escape iteration limit
    IncIterations,
    /// Decrement the escape iteration limit
    DecIterations,
    /// Switch to the next function
    SwitchFn,
    /// Increment the function exponent
    IncExp,
    /// Decrement the function exponent
    DecExp,
    /// Reset the context to defaults
    Reset,
}

impl tui::widgets::Widget for Rctx {
    fn draw(&mut self, rect: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let bounds = Bounds {
            width: rect.width,
            height: rect.height,
        };

        let ematrix = self.bind(bounds).to_ematrix();

        for yi in 0..bounds.height {
            for xi in 0..bounds.width {
                let escape = ematrix.index((yi as usize, xi as usize));
                let rgb = self.colorer.rgb(*escape);
                let color = tui::style::Color::Rgb(rgb.0, rgb.1, rgb.2);
                buf.get_mut(xi + rect.x, yi + rect.y).set_bg(color);
            }
        }
        buf.set_string(
            rect.x,
            rect.y,
            format!("bounds={:?}, rect={:?}", bounds, rect),
            tui::style::Style::default(),
        );

        buf.set_string(
            rect.x,
            rect.y + 1,
            format!(
                "termion::terminalsize={:?}",
                termion::terminal_size().unwrap()
            ),
            tui::style::Style::default(),
        );
    }
}
