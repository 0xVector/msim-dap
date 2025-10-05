mod tcp;
mod message;
mod commander;

pub use commander::{Commands, Commander};

pub type Result<T> = std::result::Result<T, MSIMError>;

#[derive(thiserror::Error, Debug)]
pub enum MSIMError {
    #[error("IO error")]
    IOError(#[from] std::io::Error),
}
