use std::path::Path;

use ndarray::prelude::*;
use num::complex::Complex;

use crate::frequency::Frequency;
use crate::io::touchstone::Touchstone;

#[derive(Debug, PartialEq)]
pub struct Network {
    f: Frequency,
    s: Array3<Complex<f64>>,
    z0: Array2<Complex<f64>>,
}

impl Network {
    pub fn new(f: Frequency, s: Array3<Complex<f64>>, z0: Array2<Complex<f64>>) -> Self {
        Network { f, s, z0 }
    }

    pub fn from_snp(file: &Path) -> Result<Self, crate::result::ParseError> {
        let touchstone = Touchstone::new(file)?;
        Ok(Network {
            f: Frequency::from(touchstone.freqs()),
            s: touchstone.s_params(),
            z0: Array::from_elem((1, touchstone.freqs().len()), Complex::new(50., 0.)),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::frequency::{FreqUnit, Frequency};

    #[test]
    fn test_instantiation() {
        let freq = Frequency::new(1., 3., Some(3), Some(FreqUnit::GHz));
        let one_c = num::Complex::new(1., 0.);
        let s = Array::from_elem((1, 1, 3), one_c);
        let z0 = Array::from_elem((1, 3), one_c);
        let net = Network::new(freq, s, z0);
    }
}
