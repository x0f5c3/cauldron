//! The `errors` module defines the common error types.

use std::error;
use std::fmt;
use std::io;

use super::Result;

/// `Error` provides an enumeration of all possible errors reported by Sonata.
#[derive(Debug)]
pub enum Error {
    /// An IO error occurred while reading or writing audio stream.
    IoError(io::Error),
    /// The stream contained malformed data and could not be parsed.
    ParseError(&'static str),
    /// An unsupported codec is passed.
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IoError(ref err) => err.fmt(f),
            Error::ParseError(ref msg) => write!(f, "Malformed stream encountered: {}", msg),
            Error::Unsupported(ref codec) => write!(f, "Unsupported codec encountered: {}", codec),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::IoError(ref err) => Some(err),
            Error::ParseError(_) => None,
            Error::Unsupported(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IoError(err)
    }
}

/// function to create a decode error.
pub fn parse_error<T>(desc: &'static str) -> Result<T> {
    Err(Error::ParseError(desc))
}

/// function to create an unsupported codec error.
pub fn unsupported_error<T>(codec: &'static str) -> Result<T> {
    Err(Error::Unsupported(codec))
}
