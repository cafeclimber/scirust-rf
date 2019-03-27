use std::path::Path;

use ndarray::array;
use ndarray::prelude::*;

use crate::frequency::{FreqUnit, Frequency};

#[derive(PartialEq)]
struct Network {
    f: Frequency,
    s: Array3<num::Complex<f32>>,
    z0: Array2<num::Complex<f32>>,
}

impl Network {
    fn new(f: Frequency, s: Array3<num::Complex<f32>>, z0: Array2<num::Complex<f32>>) -> Self {
        Network { f, s, z0 }
    }

    /*fn from_snp(file: &Path) -> Self {
        // Check that sNp is in s-parameter format
        // Get comments
        // Get port names
        // Get z0
        // Get sparameters
    }*/
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
