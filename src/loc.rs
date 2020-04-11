//! A location and magnification within the complex plane.

use crate::Bounds;
use crate::Pos;
use num::complex::Complex64;
use serde::{Deserialize, Serialize};

/// A location, scalar, and rendering context for a position in the complex plane.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Loc {
    /// The imaginary axis origin.
    pub im0: f64,

    /// The real axis origin.
    pub re0: f64,

    /// Magnification/zoom factor.
    pub scalar: f64,

    /// The maximum iterations before declaring a complex does not converge.
    pub max_iter: u32,
}

#[derive(Clone, Debug)]
pub enum ScaleMethod {
    Min,
    Avg,
    Max,
}

impl Loc {
    /// Create a location scaled appropriately for a given bounds.
    pub fn for_bounds(bounds: Bounds) -> Self {
        let re_steps: f64 = 1.5 / f64::from(bounds.width);
        let im_steps: f64 = 1.5 / f64::from(bounds.height);

        let scalar = re_steps.max(im_steps);

        Self {
            im0: 0.,
            re0: -0.,
            scalar: scalar,
            max_iter: 100,
        }
    }

    /// Given a current bounds and a new bounds, a location that's scaled such that the original
    /// location and new location describe approximately equivalent spaces with different resolutions.
    ///
    /// This acts to downscale/upscale a location.
    pub fn scale(&self, old: Bounds, new: Bounds, scale_by: ScaleMethod) -> Self {
        let scalar: f64;

        let re_scalar = f64::from(new.width) / f64::from(old.width);
        let im_scalar = f64::from(new.height) / f64::from(old.height);

        match scale_by {
            ScaleMethod::Min => {
                scalar = self.scalar / re_scalar.min(im_scalar);
            }
            ScaleMethod::Avg => {
                scalar = self.scalar / ((re_scalar + im_scalar) / 2.);
            }
            ScaleMethod::Max => {
                scalar = self.scalar / re_scalar.max(im_scalar);
            }
        }

        Self { scalar, ..*self }
    }

    pub fn origin(&self) -> Complex64 {
        Complex64 {
            re: self.re0,
            im: self.im0,
        }
    }

    /// Move the location to the position given by an imaginary number.
    pub fn move_to(&mut self, c: Complex64) {
        self.im0 = c.im;
        self.re0 = c.re;
    }
}

/// Generate a default location with scaling set for a terminal.
impl Default for Loc {
    fn default() -> Self {
        Self {
            im0: 0.0,
            re0: 0.0,
            scalar: 0.1,
            max_iter: 100,
        }
    }
}
