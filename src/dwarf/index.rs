use crate::{Address, LineNo};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct FileId(u32);

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct LineKey {
    file: FileId,
    line: LineNo,
}

pub struct DwarfIndexBuilder {
    current_id: FileId,
    file_ids: HashMap<PathBuf, FileId>,
    line_to_address: HashMap<LineKey, Address>,
    address_to_line: BTreeMap<Address, LineKey>,
}

impl DwarfIndexBuilder {
    pub fn new() -> Self {
        Self {
            current_id: FileId(0),
            file_ids: HashMap::new(),
            line_to_address: HashMap::new(),
            address_to_line: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, address: Address, path: PathBuf, line: LineNo) {
        let file = *self.file_ids.entry(path).or_insert_with(|| {
            self.current_id = FileId(self.current_id.0 + 1);
            self.current_id
        });
        let key = LineKey { file, line };
        self.line_to_address.insert(key, address);
        self.address_to_line.insert(address, key);
    }

    pub fn build(self) -> DwarfIndex {
        DwarfIndex {
            file_ids: self.file_ids,
            line_to_address: self.line_to_address,
            address_to_line: self.address_to_line,
        }
    }
}

pub struct DwarfIndex {
    file_ids: HashMap<PathBuf, FileId>,
    line_to_address: HashMap<LineKey, Address>,
    address_to_line: BTreeMap<Address, LineKey>,
}

impl DwarfIndex {
    pub fn get_address(&self, file_path: &Path, line: LineNo) -> Option<Address> {
        let id = *self.file_ids.get(file_path)?;
        self.line_to_address
            .get(&LineKey { file: id, line })
            .copied()
    }

    pub fn resolve_address(&self, address: Address) -> Option<(&Path, LineNo)> {
        self.address_to_line
            .range(..=address)
            .next_back()
            .and_then(|(_, key)| {
                let path = self
                    .file_ids
                    .iter()
                    .find(|&(_, v)| *v == key.file)?
                    .0
                    .as_path();
                Some((path, key.line))
            })
    }
}
