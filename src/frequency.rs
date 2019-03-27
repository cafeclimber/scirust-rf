use std::ops::Mul;
use std::str::FromStr;

use ndarray::prelude::*;

#[derive(Copy, Clone, Debug)]
pub enum FreqUnit {
    Hz,
    KHz,
    MHz,
    GHz,
    THz,
}

impl FromStr for FreqUnit {
    type Err = crate::result::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use FreqUnit::*;
        match s {
            "hz" => Ok(Hz),
            "khz" => Ok(KHz),
            "mhz" => Ok(MHz),
            "ghz" => Ok(GHz),
            "thz" => Ok(THz),
            _ => Err(crate::result::ParseError),
        }
    }
}

impl Mul<f32> for FreqUnit {
    type Output = f32;

    fn mul(self, rhs: f32) -> f32 {
        use FreqUnit::*;
        match self {
            Hz => rhs,
            KHz => 1e3 * rhs,
            MHz => 1e6 * rhs,
            GHz => 1e9 * rhs,
            THz => 1e12 * rhs,
        }
    }
}

/// Represents a frequency band
#[derive(PartialEq, Debug)]
pub struct Frequency {
    f: Array1<f32>,
    start: f32,
    stop: f32,
    npoints: usize,
}

impl From<Vec<f32>> for Frequency {
    fn from(freqs: Vec<f32>) -> Self {
        let temp = freqs.clone();
        Frequency {
            f: Array::from_vec(temp),
            start: freqs[0],
            stop: freqs.last().cloned().unwrap(),
            npoints: freqs.len(),
        }
    }
}

impl Frequency {
    pub fn new(start: f32, stop: f32, npoints: Option<usize>, unit: Option<FreqUnit>) -> Self {
        let n = match npoints {
            Some(n) => n,
            None => 0,
        };
        let unit = match unit {
            Some(u) => u,
            None => FreqUnit::Hz,
        };
        let f = Array::linspace(unit * start, unit * stop, n);
        Frequency {
            f,
            start,
            stop,
            npoints: n,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_instantiation() {
        let good = Frequency {
            f: array![0., 1., 2., 3., 4., 5.],
            start: 0.,
            stop: 5.,
            npoints: 6,
        };
        let test = Frequency::new(0., 5., Some(6), Some(FreqUnit::Hz));
        assert_eq!(test, good);
    }
}
