use std::path::Path;

mod protocol;
mod parser;

fn main() {
    let index = parser::parse_dwarf(Path::new("kernel.raw"));
    protocol::run().expect("Protocol error");
}
