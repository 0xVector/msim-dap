mod index;
mod parse;

pub use crate::dwarf::index::DwarfIndex;
pub use crate::dwarf::parse::parse_dwarf;

pub type Result<T> = std::result::Result<T, DwarfError>;

#[derive(thiserror::Error, Debug)]
pub enum DwarfError {
    #[error("Parse error")]
    Parse(String),

    #[error("Object lib error")]
    Object(#[from] object::Error),

    #[error("IO error")]
    Io(#[from] std::io::Error),
}
