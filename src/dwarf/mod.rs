mod index;
mod parse;

pub use crate::dwarf::index::DwarfIndex;
pub use crate::dwarf::parse::parse_dwarf;

pub type Result<T> = std::result::Result<T, DwarfError>;

#[derive(thiserror::Error, Debug)]
pub enum DwarfError {
    #[error("Parse error")]
    ParseError(String),

    #[error("Object lib error")]
    ObjectError(#[from] object::Error),

    #[error("IO error")]
    IoError(#[from] std::io::Error),
}
