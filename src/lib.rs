//! This is a port of pythons scikit-rf module.
//! It is currently intended as an exericise in
//! developing rust, python, and software architecture

mod frequency;
mod network;
pub mod touchstone;

mod result {
    #[derive(Debug)]
    pub struct ParseError;
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
