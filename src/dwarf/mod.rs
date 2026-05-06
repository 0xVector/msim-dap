//! DWARF parsing and indexing functionality.
mod index;
mod parse;

pub use index::DebugIndex;
pub use parse::parse_dwarf;

/// Result type for DWARF parsing and indexing operations.
pub type Result<T> = std::result::Result<T, DwarfError>;

/// Errors that can occur during DWARF parsing and indexing.
#[derive(thiserror::Error, Debug)]
pub enum DwarfError {
    #[error("Parse error")]
    Parse(String),

    #[error("Object lib error: {0}")]
    Object(#[from] object::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
