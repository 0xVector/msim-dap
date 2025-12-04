# msim-dap

[Debugging Adapter Protocol (DAP)](https://microsoft.github.io/debug-adapter-protocol/) implementation for the [MSIM simulator](https://github.com/d-iii-s/msim)
written in Rust.

## Releases

You can download a pre-built binary in the [releases section](https://github.com/0xVector/msim-dap/releases).

Currently, Linux `linux-x86` (`amd64`) and macOS Apple Silicon (`darwin-arm64`) binaries are provided,
if you need any other, you have to build them yourself.

## Building from sources

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
