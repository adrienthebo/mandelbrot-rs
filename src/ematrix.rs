//! An escape matrix contains an evaluated section of one of the holomorphic functions.

use crate::Escape;

/// An EMatrix maps the cells in a frame to corresponding evaluated escapes.
#[derive(Debug, Clone)]
pub struct EMatrix(nalgebra::DMatrix<Escape>);

impl EMatrix {
    pub fn from_vec(ncols: usize, nrows: usize, v: Vec<Escape>) -> EMatrix {
        let mat = nalgebra::DMatrix::from_vec(ncols, nrows, v);
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

    pub fn iter(&self) -> EMatrixRefIterator {
        EMatrixRefIterator {
            mat: self,
            index: 0,
        }
    }

    pub fn into_iter(self) -> EMatrixIterator {
        EMatrixIterator {
            mat: self,
            index: 0
        }
    }

    pub fn into_img(self) -> image::RgbImage {
        let mat = self.0;
        image::RgbImage::from_fn(mat.ncols() as u32, mat.nrows() as u32, move |x, y| {
            let escape = mat.index((y as usize, x as usize));
            let term_rgb = crate::rgb(*escape);
            image::Rgb([term_rgb.0, term_rgb.1, term_rgb.2])
        })
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
