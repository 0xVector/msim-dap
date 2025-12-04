# msim-dap

[Debugging Adapter Protocol (DAP)](https://microsoft.github.io/debug-adapter-protocol/) implementation for the [MSIM simulator](https://github.com/d-iii-s/msim)
written in Rust.

## Building

To build the binary, first [install Rust](https://rust-lang.org/tools/install/).

Then, clone the repository and run in the root:
```shell
cargo build --release
```

You can now find the built `msim-dap` binary in [`./target/release/`](./target/release/).

## Usage

Most easily used with the [`msim-debugger` VS Code extension](https://github.com/0xVector/msim-debugger),
see the tutorial there.

It can also work standalone, run it as `msim-dap -h` to view the options.
