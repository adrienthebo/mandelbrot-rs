//! A location and magnification within the complex plane.

use num::complex::Complex64;
use serde::Serialize;
use crate::Bounds;

/// A location, scalar, and rendering context for a position in the complex plane.
#[derive(Clone, Debug, Serialize)]
pub struct Loc {
    /// The imaginary axis origin.
    pub im0: f64,

    /// The real axis origin.
    pub re0: f64,

    /// Dimensional scaling factors in case the canvas is not square.
    ///
    /// This compensates for terminal cells having a 2:1 ratio.
    pub comp: (f64, f64),

    /// Magnification/zoom factor.
    pub scalar: f64,

    /// The maximum iterations before declaring a complex does not converge.
    pub max_iter: u32,
}

impl Loc {
    /// Create a location scaled appropriately for a given bounds.
    pub fn for_bounds(bounds: Bounds) -> Self {
        let re_steps: f64 = 1.5 / f64::from(bounds.0);
        let im_steps: f64 = 1.5 / f64::from(bounds.1);

        let scalar = if re_steps > im_steps { re_steps } else { im_steps };

        Self {
            im0: 0.,
            re0: -0.5,
            comp: (1., 2.),
            scalar: scalar,
            max_iter: 100
        }
    }


    /// Determine the complex value at a given offset of the origin with respect to the provided
    /// bounds.
    pub fn complex_at(&self, bounds: Bounds, pos: (u16, u16)) -> Complex64 {
        let origin: (i32, i32) = (i32::from(bounds.0 / 2), i32::from(bounds.1 / 2));
        let offset: (i32, i32) = (i32::from(pos.0) - origin.0, i32::from(pos.1) - origin.1);

        Complex64 {
            re: self.comp.0 * f64::from(offset.0) * self.scalar + self.re0,
            im: self.comp.1 * f64::from(offset.1) * self.scalar + self.im0,
        }
    }

    /// Given a current bounds and a new bounds, a location that's scaled such that the original
    /// location and new location describe approximately equivalent spaces with different resolutions.
    ///
    /// This acts to downscale/upscale a location.
    pub fn scale(&self, old: Bounds, new: Bounds) -> Self {
        let re_scalar = f64::from(new.0) / f64::from(old.0);
        let im_scalar = f64::from(new.1) / f64::from(old.1);
        let min = if re_scalar < im_scalar { re_scalar } else { im_scalar };
        let scalar = self.scalar / min;

        Self { scalar, .. *self }
    }

    pub fn origin(&self) -> Complex64 {
        Complex64 {
            re: self.re0,
            im: self.im0,
        }
    }
}

impl Default for Loc {
    fn default() -> Self {
        Self {
            im0: 0.0,
            re0: 0.0,
            comp: (1., 2.),
            scalar: 0.1,
            max_iter: 100,
        }
    }
}


