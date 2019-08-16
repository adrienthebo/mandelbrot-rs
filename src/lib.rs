//! Mandelbrot

use std::io;

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
        EMatrixRefIterator { mat: self, index: 0 }
    }

    pub fn into_img(self) -> image::RgbImage {
        let mat = self.0;
        image::RgbImage::from_fn(mat.ncols() as u32, mat.nrows() as u32, move |x, y| {
            let escape = mat.index((y as usize, x as usize));
            let term_rgb = rgb(*escape);
            image::Rgb([term_rgb.0, term_rgb.1, term_rgb.2])
        })
    }
}

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

impl std::iter::IntoIterator for EMatrix {
    type Item = Escape;
    type IntoIter = EMatrixIterator;

    fn into_iter(self) -> Self::IntoIter {
        EMatrixIterator {
            mat: self,
            index: 0
        }
    }
}

pub struct EMatrixRefIterator<'a> {
    mat: &'a EMatrix,
    index: usize
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
        EMatrixRefIterator { mat: self, index: 0 }
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
