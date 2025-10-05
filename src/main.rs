use clap::Parser;
use msim_dap::{Config, Mode, Result};

#[derive(Parser, Debug)]
#[command(name = "adapter")]
struct Opts {
    /// Use the DAP side in TCP mode (default is stdio), with optional port number to use
    #[arg(short = 'd', long, num_args(0..=1), default_missing_value = "15000", value_name = "PORT")]
    dap_tcp_mode: Option<u16>,

    /// Port to use for MSIM connection
    #[arg(short = 'm', long, num_args(0..=1), default_value_t = 5000, value_name = "PORT")]
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
    verbose: bool
}

fn main() -> Result<()>{
    let opts = Opts::parse();

    let mode = match opts.dap_tcp_mode {
        Some(v) => Mode::TCP(v),
        None => Mode::Stdio,
    };

    let config = Config {
        mode,
        msim_port: opts.msim_port,
        kernel_path: opts.kernel_raw_path.as_ref(),
    };

    if opts.verbose {println!("Using config:\n{:#?}", config)}

    println!("Running...");
    msim_dap::run(&config)?;

    println!("Exiting...");
    Ok(())
}
