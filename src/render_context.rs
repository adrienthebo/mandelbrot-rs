//! The set of state needed to render a holomorphic function to an image.
//!
//! The rendering context binds together a holomorphic function and location
//!
//! Remaining work:
//!
//! - Add coloring logic
//! - Add a related type that binds a rendering context with a specific bounds.

use crate::Bounds;
use crate::EMatrix;
use crate::Escape;
use crate::Holomorphic;
use crate::Julia;
use crate::Loc;
use crate::Mandelbrot;
use itertools::Itertools;
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct RenderContext {
    /// The current loc.
    pub loc: Loc,
    /// The active holomorphic function.
    pub holomorphic: Holomorphic,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            loc: Loc::default(),
            holomorphic: Holomorphic::default(),
        }
    }
}

impl RenderContext {
    const TRANSLATE_SCALAR: f64 = 10.;
    const SCALE_SCALAR: f64 = 2.;
    const ITERATIONS_SCALAR: u32 = 25;

    /// Generate an escape matrix from the current application context.
    ///
    /// # Performance
    ///
    /// This fn is the most expensive operation in the application.
    ///
    pub fn render(&self, bounds: Bounds) -> EMatrix {
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

    /// Create a new application context with a pre-defined location.
    pub fn with_loc(loc: Loc) -> Self {
        Self {
            loc,
            holomorphic: Holomorphic::default(),
        }
    }

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

            RctxTransform::ToggleHolo => {
                let new_holo: Holomorphic;
                match self.holomorphic {
                    Holomorphic::Julia(ref j) => {
                        new_holo = Holomorphic::Mandelbrot(Mandelbrot::from(j));
                    }
                    Holomorphic::Mandelbrot(ref m) => {
                        new_holo = Holomorphic::Julia(Julia::from_c(m, self.loc.origin()))
                    }
                }
                self.holomorphic = new_holo;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RctxTransform {
    TranslateUp,
    TranslateDown,
    TranslateLeft,
    TranslateRight,
    ScaleIn,
    ScaleOut,
    IncIterations,
    DecIterations,
    ToggleHolo,
    Reset,
}
