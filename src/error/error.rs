use std::fmt::{Display, Formatter};
use lettre::transport::smtp;
use std::str::ParseBoolError;
use std::env::VarError;

#[derive(Debug)]
pub enum ExpectedError {
    TypeError(String),
    NoneError(String),
    ProcessError(String),
    InvalidError(String),
}

impl From<smtp::Error> for ExpectedError {
    fn from(err: smtp::Error) -> Self {
        ExpectedError::ProcessError(err.to_string())
    }
}

impl From<ParseBoolError> for ExpectedError {
    fn from(err: ParseBoolError) -> Self {
        ExpectedError::TypeError(err.to_string())
    }
}

impl From<VarError> for ExpectedError {
    fn from(err: VarError) -> Self {
        match err {
            VarError::NotPresent => ExpectedError::NoneError(err.to_string()),
            VarError::NotUnicode(_) => ExpectedError::TypeError(err.to_string()),
        }
    }
}

impl Display for ExpectedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpectedError::TypeError(err) => write!(f, "{}", err),
            ExpectedError::NoneError(err) => write!(f, "{}", err),
            ExpectedError::ProcessError(err) => write!(f, "{}", err),
            ExpectedError::InvalidError(err) => write!(f, "{}", err),
        }
    }
}
