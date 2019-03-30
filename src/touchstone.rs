use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;
use std::str::FromStr;

use ndarray::prelude::*;
use num::Complex;

use crate::frequency::FreqUnit;
use crate::result::ParseError;
use crate::CxArray3;


#[derive(PartialEq, Debug)]
enum TouchstoneVersion {
    One,
    Two,
}

impl Default for TouchstoneVersion {
    fn default() -> Self {
        TouchstoneVersion::One
    }
}

#[derive(PartialEq, Debug)]
enum ParamType {
    S,
    Y,
    Z,
    G,
    H,
}

impl FromStr for ParamType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParamType::*;
        match s {
            "s" | "S" => Ok(S),
            "y" | "Y" => Ok(Y),
            "z" | "Z" => Ok(Z),
            "g" | "G" => Ok(G),
            "h" | "H" => Ok(H),
            _ => Err(ParseError),
        }
    }
}

#[derive(PartialEq, Debug)]
enum ParamFormat {
    DBAngle,
    MagAngle,
    RealImag,
}

impl FromStr for ParamFormat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParamFormat::*;
        match s {
            "db" | "DB" => Ok(DBAngle),
            "ma" | "MA" => Ok(MagAngle),
            "ri" | "RI" => Ok(RealImag),
            _ => Err(ParseError),
        }
    }
}

#[derive(Debug)]
struct TouchstoneOptions {
    unit: FreqUnit,
    param_type: ParamType,
    param_format: ParamFormat,
    resistance: f64,
}

impl Default for TouchstoneOptions {
    fn default() -> Self {
        TouchstoneOptions {
            unit: FreqUnit::GHz,
            param_type: ParamType::S,
            param_format: ParamFormat::MagAngle,
            resistance: 50.,
        }
    }
}

#[derive(Default)]
pub struct Touchstone {
    filename: String,
    version: TouchstoneVersion,
    comments: Vec<String>,
    num_ports: Option<usize>,
    freqs: Vec<f64>,
    num_freq_points: Option<usize>,
    num_noise_freq_points: Option<usize>,
    reference: Option<Vec<f64>>,
    options: TouchstoneOptions,
    s_params: CxArray3,
    rank: usize,
    noise: Option<CxArray3>,
}

impl Touchstone {
    pub fn freqs(&self) -> Vec<f64> {
        self.freqs.clone()
    }

    pub fn s_params(&self) -> CxArray3 {
        self.s_params.clone()
    }

    pub fn new(path: &Path) -> Result<Self, ParseError> {
        let mut touchstone = Touchstone::default();

        if let Some(extension) = path.extension() {
            let extension = extension.to_str().unwrap();
            // If it'"s an sNp file, ensure N matches dimension
            if extension.starts_with('s') && extension.ends_with('p') {
                if let Ok(rank_from_file) = extension
                    .trim_start_matches('s')
                    .trim_end_matches('p')
                    .parse::<usize>()
                {
                    touchstone.rank = rank_from_file;
                } else {
                    return Err(ParseError);
                }
            } else if extension == "ts" {
                unimplemented!();
            } else {
                return Err(ParseError);
            }
        }

        // Main parse loop
        let mut options_read = false;
        let file = File::open(path).unwrap();
        let mut buf_reader = BufReader::new(file);
        let mut line_buf = String::new();
        let mut temp_s_params: Vec<Complex<f64>> = vec![];
        loop {
            line_buf.clear();
            if buf_reader
                .read_line(&mut line_buf)
                .expect("An error occurred while reading the file")
                == 0
            {
                break;
            }
            let mut line = line_buf.to_lowercase();

            if line.trim_end().is_empty() {
                continue;
            }

            if let Some(idx) = line.rfind('!') {
                if !options_read {
                    touchstone
                        .comments
                        .push(line[idx..].trim_start_matches('!').to_owned());
                    continue;
                } else {
                    if line.starts_with('!') {
                        continue;
                    } // If the whole line is a comment, ignore it
                    line = line[..idx].to_owned(); // otherwise, get rid of comment and parse the line
                }
            }

            if line.starts_with("[version]") {
                if line.trim_start_matches("[version]").trim() == "2.0" {
                    touchstone.version = TouchstoneVersion::Two;
                }
            } else if line.starts_with("[reference]") {
                if line.trim_start_matches("[reference]").trim() == "" {
                    line.clear();
                    buf_reader.read_line(&mut line).unwrap();
                }
                touchstone.reference =
                    Some(line.trim_end().split(' ').map(|r| r.parse::<f64>().unwrap()).collect());
            } else if line.starts_with("[number of ports]") {
                touchstone.num_ports = line
                    .trim_start_matches("[number of ports]")
                    .parse::<usize>()
                    .ok();
            } else if line.starts_with("[number of frequencies]") {
                touchstone.num_freq_points = line
                    .trim_start_matches("[number of frequencies]")
                    .parse::<usize>()
                    .ok();
            } else if line.starts_with("[number of noise frequencies]") {
                touchstone.num_noise_freq_points = line
                    .trim_start_matches("[number of noise frequencies]")
                    .parse::<usize>()
                    .ok();
            } else if line.starts_with("[network data]") {
                // According to the spec, this just explicitly marks the beginning of network data.
                // It seems we can just ignore it.
                continue;
            } else if line.starts_with("[end]") {
                break;
            } else if line.starts_with('#') {
                parse_options_line(&line, &mut touchstone.options)?;
                options_read = true;
            } else {
                let mut chunked: Vec<f64> = line
                    .split_whitespace()
                    .map(|v| v.parse::<f64>().unwrap())
                    .collect();
                // If the line starts with a frequency or if all data is contained in one line
                if chunked.len() == (touchstone.rank * 2) + 1
                    || chunked.len() == 2 * num::pow(touchstone.rank, 2) + 1
                {
                    touchstone.freqs.push(chunked[0]);
                    chunked.remove(0);
                }
                let pairs_iter = chunked.chunks(2);
                let mut temp: Vec<Complex<f64>> = pairs_iter
                    .map(|pair| Complex::new(pair[0], pair[1]))
                    .collect();
                temp_s_params.append(&mut temp);
            }
        }
        touchstone.s_params = match Array::from_shape_vec(
            (touchstone.freqs.len(), touchstone.rank, touchstone.rank),
            temp_s_params,
        ) {
            Ok(s_params) => {
                println!("{:?}", s_params);
                s_params
            }
            _ => return Err(ParseError),
        };
        Ok(touchstone)
    }
}

fn parse_options_line(line: &str, options: &mut TouchstoneOptions) -> Result<(), ParseError> {
    let mut split_line: Vec<&str> = line.split_whitespace().collect();
    split_line.remove(0);
    for (index, entry) in split_line.iter().enumerate() {
        if entry.contains("hz") {
            options.unit = entry.parse()?;
        } else if entry.len() == 2 {
            match *entry {
                "db" | "ma" | "ri" => options.param_format = entry.parse()?,
                _ => { /* Do nothing */ }
            }
        } else if "syzhg".contains(entry) && entry.len() == 1 {
            // TODO: Is this correct?
            match *entry {
                "s" | "y" | "z" | "h" | "g" => options.param_type = entry.parse()?,
                _ => { /* Do nothing */ }
            }
        } else if *entry == "r" {
            options.resistance = match split_line[index + 1].parse::<f64>() {
                Ok(r) => r,
                Err(_) => return Err(ParseError),
            }
        }
    }
    Ok(())
}

impl fmt::Debug for Touchstone {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Touchstone:\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            format!("\tFilename: {:?}", self.filename),
            format!("\tVersion: {:?}", self.version),
            format!("\tOptions: {:?}", self.options),
            format!("\tNumber of Ports: {:?}", self.num_ports),
            format!("\tNumber of Frequency Points: {:?}", self.num_freq_points),
            format!("\tNumber of Noise Points: {:?}", self.num_noise_freq_points),
            format!("\tReference: {:?}", self.reference),
            format!("\tRank: {:?}", self.rank),
            format!("\tFreqs: {:?}", self.freqs.len()),
            format!(
                "\tS Parameters: ndarray::Array3<Complex<f64>>{:?}",
                self.s_params.shape()
            ),
            format!("\tNoise: \n{:?}", self.noise),
            format!("\tComments: \n{:?}", self.comments),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;
    use ndarray::prelude::*;

    #[test]
    fn test_hfss_s2p() {
        let path = std::path::PathBuf::from("tests/hfss_twoport.s2p");
        let touchstone = Touchstone::new(&path).unwrap();
        assert_eq!(touchstone.s_params.dim(), (101, 2, 2));
        assert_eq!(touchstone.options.unit, FreqUnit::GHz);
        assert_eq!(touchstone.options.param_type, ParamType::S);
        assert_eq!(touchstone.options.param_format, ParamFormat::MagAngle);
    }

    #[test]
    fn test_hfss_s3p() {
        let path = std::path::PathBuf::from("tests/hfss_threeport_DB.s3p");
        let touchstone = Touchstone::new(&path).unwrap();
        assert_eq!(touchstone.s_params.dim(), (451, 3, 3));
        assert_eq!(touchstone.options.unit, FreqUnit::GHz);
        assert_eq!(touchstone.options.param_type, ParamType::S);
        assert_eq!(touchstone.options.param_format, ParamFormat::DBAngle);
    }

    #[test]
    fn test_cst_example_4ports_s4p() {
        let path = std::path::PathBuf::from("tests/cst_example_4ports.s4p");
        let touchstone = Touchstone::new(&path).unwrap();
        assert_eq!(touchstone.s_params.dim(), (601, 4, 4));
        assert_eq!(touchstone.options.unit, FreqUnit::MHz);
        assert_eq!(touchstone.options.param_type, ParamType::S);
        assert_eq!(touchstone.options.param_format, ParamFormat::MagAngle);
    }

    #[test]
    fn test_cst_example_6ports_v2_s6p() {
        let path = std::path::PathBuf::from("tests/cst_example_6ports_V2.s6p");
        let touchstone = Touchstone::new(&path).unwrap();
        assert_eq!(touchstone.s_params.dim(), (1001, 6, 6));
        assert_eq!(touchstone.options.unit, FreqUnit::MHz);
        assert_eq!(touchstone.options.param_type, ParamType::S);
        assert_eq!(touchstone.options.param_format, ParamFormat::MagAngle);
        assert_eq!(touchstone.options.resistance, 15.063);
        assert_eq!(touchstone.reference, Some(vec![15.063, 15.063, 15.063, 15.063, 15.063, 15.063]));
    }

    #[test]
    fn test_simple_s2p() {
        let path = std::path::PathBuf::from("tests/ntwk_arbitrary_frequency.s2p");
        let touchstone = Touchstone::new(&path).unwrap();
        let good_array = array![
            [
                [
                    Complex::new(0.0217920488, -0.151514165),
                    Complex::new(0.926746562, -0.170089428)
                ],
                [
                    Complex::new(0.926746562, -0.170089428),
                    Complex::new(0.0234769169, -0.121728077)
                ]
            ],
            [
                [
                    Complex::new(0.0165040395, -0.165812914),
                    Complex::new(0.92149708, -0.186257735)
                ],
                [
                    Complex::new(0.92149708, -0.186257735),
                    Complex::new(0.0185387559, -0.133078528)
                ]
            ],
            [
                [
                    Complex::new(0.0107648639, -0.179877134),
                    Complex::new(0.915799359, -0.202194824)
                ],
                [
                    Complex::new(0.915799359, -0.202194824),
                    Complex::new(0.0131812086, -0.144202856)
                ]
            ],
            [
                [
                    Complex::new(0.00458796245, -0.193689477),
                    Complex::new(0.909666648, -0.217883591)
                ],
                [
                    Complex::new(0.909666648, -0.217883591),
                    Complex::new(0.00741732005, -0.155084364)
                ]
            ]
        ];
        assert_eq!(touchstone.s_params, good_array);
        assert_eq!(touchstone.freqs, vec![1., 4., 10., 20.]);
        assert_eq!(touchstone.options.unit, FreqUnit::Hz);
        assert_eq!(touchstone.options.param_type, ParamType::S);
        assert_eq!(touchstone.options.param_format, ParamFormat::RealImag);
    }
}
