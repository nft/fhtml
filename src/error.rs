use std::fmt;

/// A compile error with a 1-based source position.
///
/// Per SPEC §11, errors are strict and precise — there is no recovery mode
/// that silently guesses.
#[derive(Debug)]
pub struct Error {
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn err<T>(line: usize, col: usize, msg: impl Into<String>) -> Result<T> {
    Err(Error {
        line,
        col,
        msg: msg.into(),
    })
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: error: {}", self.line, self.col, self.msg)
    }
}

impl std::error::Error for Error {}
