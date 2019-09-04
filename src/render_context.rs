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
use crate::ComplexFn;
use crate::PolyComplexFn;
use crate::Julia;
use crate::Loc;
use crate::Mandelbrot;
use crate::Pos;
use itertools::Itertools;
use rayon::prelude::*;
use std::ops::Index;

#[derive(Clone, Debug)]
pub struct RenderContext {
    /// The current loc.
    pub loc: Loc,
    /// The active holomorphic function.
    pub holomorphic: PolyComplexFn,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            loc: Loc::default(),
            holomorphic: PolyComplexFn::default(),
        }
    }
}

impl RenderContext {
    const TRANSLATE_SCALAR: f64 = 10.;
    const SCALE_SCALAR: f64 = 2.;
    const ITERATIONS_SCALAR: u32 = 25;
    const EXP_SCALAR: f64 = 0.0125;

    /// Generate an escape matrix from the current application context.
    ///
    /// # Performance
    ///
    /// This fn is the most expensive operation in the application.
    ///
    pub fn to_ematrix(&self, bounds: Bounds) -> EMatrix {
        let y_iter = 0..bounds.height;
        let x_iter = 0..bounds.width;

        let escapes: Vec<Escape> = x_iter
            .cartesian_product(y_iter)
            .map(|pt| Pos::from(pt))
            .collect::<Vec<Pos>>()
            .par_iter()
            .map(|pos| self.loc.complex_at(bounds, *pos))
            .map(|c| self.holomorphic.render(c, self.loc.max_iter))
            .collect();

        EMatrix::from_vec(
            usize::from(bounds.height),
            usize::from(bounds.width),
            escapes,
        )
    }

    /// Create a new application context with a pre-defined location.
    pub fn with_loc(loc: Loc) -> Self {
        Self {
            loc,
            holomorphic: PolyComplexFn::default(),
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

            RctxTransform::IncExp => {
                *self.holomorphic.exp_mut() += Self::EXP_SCALAR;
            }
            RctxTransform::DecExp => {
                *self.holomorphic.exp_mut() -= Self::EXP_SCALAR;
            }

            RctxTransform::ToggleHolo => {
                let new_holo: PolyComplexFn;
                match self.holomorphic {
                    PolyComplexFn::Julia(ref j) => {
                        new_holo = PolyComplexFn::Mandelbrot(Mandelbrot::from(j));
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
                        new_holo = PolyComplexFn::Julia(Julia::from_c(m, self.loc.origin()))
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
    IncExp,
    DecExp,
    Reset,
}

impl tui::widgets::Widget for RenderContext {
    fn draw(&mut self, rect: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let bounds = Bounds {
            width: rect.width,
            height: rect.height,
        };

        let sr = crate::SineRGB::default();

        let ematrix = RenderContext::to_ematrix(self, bounds);

        for yi in 0..bounds.height {
            for xi in 0..bounds.width {
                let escape = ematrix.index((yi as usize, xi as usize));
                let rgb = sr.rgb(*escape);
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
