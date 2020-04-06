//! An escape matrix contains an evaluated section of one of the complex polynomial functions.

use crate::Escape;

/// An EMatrix maps the cells in a frame to corresponding evaluated escapes.
#[derive(Debug, Clone)]
pub struct EMatrix(nalgebra::DMatrix<Escape>);

impl EMatrix {
    pub fn from_vec(ncols: usize, nrows: usize, v: Vec<Escape>) -> EMatrix {
        let mat = nalgebra::DMatrix::from_vec(ncols, nrows, v);
        Self(mat)
    }

    pub fn from_dmatrix(mat: nalgebra::DMatrix<Escape>) -> Self {
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

    pub fn nrows(&self) -> usize {
        self.0.nrows()
    }

    pub fn ncols(&self) -> usize {
        self.0.ncols()
    }

    /// Fetch a reference to the inner matrix.
    pub fn inner(&self) -> &nalgebra::DMatrix<Escape> {
        &self.0
    }

    /// Fetch a mutable reference to the inner matrix.
    pub fn inner_mut(&mut self) -> &nalgebra::DMatrix<Escape> {
        &mut self.0
    }

    pub fn iter(&self) -> EMatrixRefIterator {
        EMatrixRefIterator {
            mat: self,
            index: 0,
        }
    }

    pub fn into_iter(self) -> EMatrixIterator {
        EMatrixIterator {
            mat: self,
            index: 0,
        }
    }

    pub fn to_img(&self, colorer: &crate::SineRGB) -> image::RgbImage {
        let mat = &self.0;

        image::RgbImage::from_fn(mat.ncols() as u32, mat.nrows() as u32, move |x, y| {
            let escape = mat.index((y as usize, x as usize));
            let term_rgb = colorer.rgb(*escape);
            image::Rgb([term_rgb.0, term_rgb.1, term_rgb.2])
        })
    }

    /// Create a new ematrix with a gaussian blur
    pub fn gaussian_blur(&self) -> Self {
        let kernel = nalgebra::base::Matrix3::new(1., 2., 1., 2., 16., 2., 1., 2., 1.);
        let divisor = kernel.sum();

        Self(nalgebra::base::DMatrix::from_fn(
            self.0.nrows(),
            self.0.ncols(),
            |yi, xi| {
                if yi == 0 || xi == 0 || yi == self.0.nrows() - 1 || xi == self.0.ncols() - 1 {
                    // Gaussian blurring along image edges isn't implemented.
                    self.0.index((yi, xi)).clone()
                } else {
                    match self.0.index((yi, xi)) {
                        None => None,
                        Some(_) => {
                            let area = self.0.slice((yi - 1, xi - 1), (3, 3));

                            // Accumulate the region described by the kernel.
                            let acc = area.zip_fold(&kernel, None, |acc, escape, k| {
                                match (acc, escape) {
                                    // Nothing accumulated, no new value at the given position
                                    (None, None) => None,
                                    // Nothing accumulated, scale value by kernel and accumulate
                                    (None, Some(iters)) => Some(iters * k),
                                    // Value accumulated, no new value. Return accumulator.
                                    (Some(acc), None) => Some(acc),
                                    // Value accumulated, value present, Scale value by kernel and
                                    // add to accumulator.
                                    (Some(acc), Some(iters)) => Some(acc + (iters * k)),
                                }
                            });

                            // Take the mean of the accumulator with respect to the kernel scaling
                            // values.
                            //
                            // Note - this doesn't correctly consider missing values.
                            acc.map(|iters| iters / divisor)
                        }
                    }
                }
            },
        ))
    }
}

impl std::ops::Index<(usize, usize)> for EMatrix {
    type Output = Escape;
    fn index(&self, pos: (usize, usize)) -> &Self::Output {
        self.0.index(pos)
    }
}

/// An iterator that consumes and returns elements of an `EMatrix` in minor/major order.
pub struct EMatrixIterator {
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

/// An iterator over an `&Ematrix` in minor/major order.
impl std::iter::IntoIterator for EMatrix {
    type Item = Escape;
    type IntoIter = EMatrixIterator;

    fn into_iter(self) -> Self::IntoIter {
        EMatrixIterator {
            mat: self,
            index: 0,
        }
    }
}

/// An iterator over an `&mut Ematrix` in minor/major order.
pub struct EMatrixRefIterator<'a> {
    mat: &'a EMatrix,
    index: usize,
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
        EMatrixRefIterator {
            mat: self,
            index: 0,
        }
    }
}
