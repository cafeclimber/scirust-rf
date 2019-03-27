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

#[derive(Debug)]
enum TouchstoneVersion {
    One,
    Two,
}

impl Default for TouchstoneVersion {
    fn default() -> Self {
        TouchstoneVersion::One
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
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
    resistance: f32,
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
    num_freq_points: Option<usize>,
    num_noise_freq_points: Option<usize>,
    reference: Option<Vec<f32>>,
    options: TouchstoneOptions,
    s_params: Array3<num::Complex<f32>>,
    rank: usize,
    noise: Array3<num::Complex<f32>>,
}

impl Touchstone {
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
        let mut row = 1;
        let mut new_row = true;
        let mut temp_row: (f32, Vec<f32>) = (0., vec![]);
        let mut freqs = vec![];
        let mut matrix_data = vec![];
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
                } else {
                    touchstone.reference =
                        Some(line.split(' ').map(|r| r.parse::<f32>().unwrap()).collect());
                }
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
                let mut chunked: Vec<f32> = line
                    .split_whitespace()
                    .map(|v| v.parse::<f32>().unwrap())
                    .collect();
                if new_row {
                    row = 1;
                    temp_row = (0., vec![]);
                    freqs.push(chunked[0]);
                    chunked.remove(0);
                    new_row = false;
                }
                if row <= touchstone.rank {
                    temp_row.1.append(&mut chunked);
                    row += 1;
                    // If the second condition is true, it means the file stores all data for one
                    // frequency on one line
                    if row > touchstone.rank || temp_row.1.len() == 2 * (touchstone.rank.pow(2)) {
                        new_row = true;
                        let pairs_iter = temp_row.1.chunks(2);
                        let complex_vec: Vec<Complex<f32>> = pairs_iter
                            .map(|pair| Complex::new(pair[0], pair[1]))
                            .collect();
                        matrix_data.push(complex_vec);
                    }
                }
            }
        }
        let mut array_3d: Array3<Complex<f32>> =
            Array3::zeros((matrix_data.len(), touchstone.rank, touchstone.rank));
        for (i, mut v) in array_3d.axis_iter_mut(Axis(0)).enumerate() {
            let temp =
                Array::from_shape_vec((touchstone.rank, touchstone.rank), matrix_data[i].clone())
                    .unwrap();
            v.assign(&temp);
        }
        touchstone.s_params = array_3d;
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
            options.resistance = match split_line[index + 1].parse::<f32>() {
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
            "Touchstone:\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            format!("\tFilename: {:?}", self.filename),
            format!("\tVersion: {:?}", self.version),
            format!("\tOptions: {:?}", self.options),
            format!("\tNumber of Ports: {:?}", self.num_ports),
            format!("\tNumber of Frequency Points: {:?}", self.num_freq_points),
            format!("\tNumber of Noise Points: {:?}", self.num_noise_freq_points),
            format!("\tReference: {:?}", self.reference),
            format!("\tRank: {:?}", self.rank),
            format!(
                "\tS Parameters: ndarray::Array3<Complex<f32>>{:?}",
                self.s_params.shape()
            ),
            format!("\tNoise: \n{:?}", self.noise),
            format!("\tComments: \n{:?}", self.comments),
        )
    }
}
