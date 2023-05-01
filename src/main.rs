use clap::{Parser, Subcommand};
use std::error::Error;

#[derive(Debug, Parser)]
#[command(about, version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Suspend all output expect errors
    #[clap(long, default_value_t = false)]
    quiet: bool,
}

// TODO: add validators for infile, outdir & address (IPv4/6)

#[derive(Debug, Subcommand)]
enum Commands {
    /// Host fileserver instance by providing JSON file with paths or list of paths
    Host {
        /// Use IPv6 instead of IPv4
        #[clap(short = '6', long, default_value_t = false)]
        ipv6: bool,
        /// Path to the inputfile (JSON)
        infile: Option<String>,
        /// Paths to the files
        #[clap(short = 'f', long)]
        files: Option<Vec<String>>,
    },
    /// Connect to hosted server by providing address, output folder and access key
    Connect {
        /// IP address of the server (IPv4 or IPv6)
        address: String,
        /// Path to the output folder
        #[clap(short = 'o', long)]
        outdir: String,
        /// Access key for the fileserver
        #[clap(short = 'k', long)]
        key: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    println!("{:?}", cli);

    Ok(())
}
