use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct Address(u64);

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct FileId(u32);

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct LineKey {
    file: FileId,
    line: u64
}

pub struct DwarfIndexBuilder {
    current_id: FileId,
    file_ids: HashMap<PathBuf, FileId>,
    line_to_address: HashMap<LineKey, Address>
}

impl DwarfIndexBuilder {
    pub fn new() -> Self {
        DwarfIndexBuilder{
            current_id: FileId(0),
            file_ids: HashMap::new(),
            line_to_address: HashMap::new()
        }
    }
    
    pub fn insert(&mut self, address: u64, path: PathBuf, line: u64) {
        let id = *self.file_ids.entry(path).or_insert_with(|| {
            self.current_id = FileId(self.current_id.0 + 1);
            self.current_id
        });
        self.line_to_address.insert(LineKey{file: id, line}, Address(address));
    }

    pub fn build(self) -> DwarfIndex {
        DwarfIndex{
            file_ids: self.file_ids,
            line_to_address: self.line_to_address
        }
    }
}

pub struct DwarfIndex {
    file_ids: HashMap<PathBuf, FileId>,
    line_to_address: HashMap<LineKey, Address>
}

impl DwarfIndex {
    pub fn get_address(&self, file_path: &Path, line: u64) -> Option<Address> {
        let id = *self.file_ids.get(file_path)?;
        self.line_to_address.get(&LineKey { file: id, line }).copied()
    }
}

pub struct LineEntry {
    pub file_path: String,
    pub line_num: u64,
    pub col_num: u64
}