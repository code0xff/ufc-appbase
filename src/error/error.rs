use std::env::VarError;
use std::fmt::{Display, Formatter};
use std::num::ParseIntError;
use std::str::ParseBoolError;

use lettre::transport::smtp;

#[derive(Debug)]
pub enum ExpectedError {
    TypeError(String),
    NoneError(String),
    ProcessError(String),
    InvalidError(String),
    RequestError(String),
    ParsingError(String),
    ChannelError(String),
    FilterError(String),
    BlockHeightError(String),
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

impl From<reqwest::Error> for ExpectedError {
    fn from(err: reqwest::Error) -> Self {
        ExpectedError::RequestError(err.to_string())
    }
}

impl From<serde_json::Error> for ExpectedError {
    fn from(err: serde_json::Error) -> Self {
        ExpectedError::ParsingError(err.to_string())
    }
}

impl<T> From<tokio::sync::broadcast::error::SendError<T>> for ExpectedError {
    fn from(err: tokio::sync::broadcast::error::SendError<T>) -> Self {
        ExpectedError::ChannelError(err.to_string())
    }
}

impl From<ParseIntError> for ExpectedError {
    fn from(err: ParseIntError) -> Self {
        ExpectedError::ParsingError(err.to_string())
    }
}

impl Display for ExpectedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpectedError::TypeError(err) => write!(f, "{}", err),
            ExpectedError::NoneError(err) => write!(f, "{}", err),
            ExpectedError::ProcessError(err) => write!(f, "{}", err),
            ExpectedError::InvalidError(err) => write!(f, "{}", err),
            ExpectedError::RequestError(err) => write!(f, "{}", err),
            ExpectedError::ParsingError(err) => write!(f, "{}", err),
            ExpectedError::ChannelError(err) => write!(f, "{}", err),
            ExpectedError::FilterError(err) => write!(f, "{}", err),
            ExpectedError::BlockHeightError(err) => write!(f, "{}", err),
        }
    }
}
