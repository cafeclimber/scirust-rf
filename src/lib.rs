//! This is a port of pythons scikit-rf module.
//! It is currently intended as an exericise in
//! developing rust, python, and software architecture

mod frequency;
mod network;
pub mod touchstone;

use num::complex::Complex;
use ndarray::prelude::{Array2, Array3};

mod result {
    #[derive(Debug)]
    pub struct ParseError;
}

type CxArray2 = Array2<Complex<f64>>;
type CxArray3 = Array3<Complex<f64>>;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
