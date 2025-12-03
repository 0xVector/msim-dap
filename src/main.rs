use clap::Parser;
use msim_dap::{Config, Mode, Result};
use std::env;
use std::fs::OpenOptions;
use std::os::fd::AsRawFd;

#[derive(Parser, Debug)]
#[command(name = "adapter")]
struct Opts {
    /// Use the DAP side in TCP mode (default is stdio), with optional port number to use \[default: 10506]
    #[arg(short = 'd', long, num_args(0..=1), default_missing_value = "10506", value_name = "PORT")]
    dap_tcp_mode: Option<u16>,

    /// Port to use for MSIM connection
    #[arg(short = 'm', long, num_args(0..=1), default_value_t = 10505, value_name = "PORT")]
    msim_port: u16,

    /// kernel.raw path to use
    #[arg(
        short = 'p',
        long,
        num_args(0..=1),
        default_value_t = String::from("kernel/kernel.raw"),
        value_name = "PATH"
    )]
    kernel_raw_path: String,

    /// Use verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Log to file instead of stderr (/tmp/msim-dap.log)
    #[arg(short, long)]
    log: bool,
}

fn redirect_stderr() {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/msim-dap.log")
        .expect("open log");

    unsafe {
        libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO);
    }
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    if opts.log {redirect_stderr();}

    let cwd = env::current_dir().unwrap();
    eprintln!("Starting adapter in dir {:?}", cwd);

    let mode = match opts.dap_tcp_mode {
        Some(v) => Mode::TCP(v),
        None => Mode::Stdio,
    };

    let config = Config {
        mode,
        msim_port: opts.msim_port,
        kernel_path: opts.kernel_raw_path.as_ref(),
    };

    if opts.verbose {
        eprintln!("Using config:\n{:#?}", config)
    }

    eprintln!("Running...");
    msim_dap::run(&config)?;

    eprintln!("Exiting...");
    Ok(())
}
