mod protocol;
mod parser;

fn main() {
    protocol::run().expect("Protocol error");
}
