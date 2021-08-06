use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct TypeError;

impl Display for TypeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "type does not match!")
    }
}

impl Error for TypeError {}
