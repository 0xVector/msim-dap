use std::path::Path;

mod dwarf;
mod dap;
mod msim;

fn main() {
    println!("Parsing dwarf...");
    let index = dwarf::parse_dwarf(Path::new("kernel.raw"));
    println!("Running...");
    dap::run(index).expect("Protocol error");
    println!("Exiting...");
}
