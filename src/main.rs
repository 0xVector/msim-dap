use std::path::Path;

mod parser;
mod protocol;

fn main() {
    println!("Parsing dwarf...");
    let index = parser::parse_dwarf(Path::new("kernel.raw"));
    println!("Running...");
    protocol::run(index).expect("Protocol error");
    println!("Exiting...");
}
