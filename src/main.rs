use clap::{Parser, Subcommand};
use contego::parsers::{addr_parser, dirpath_parser, filepath_parser};
use std::{error::Error, net::SocketAddr, path::PathBuf};

#[derive(Debug, Parser)]
#[command(about, version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Suspend all output expect errors
    #[clap(long, default_value_t = false)]
    quiet: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Host fileserver instance by providing JSON file with paths or list of paths
    Host {
        /// Use IPv6 instead of IPv4
        #[clap(short = '6', long, default_value_t = false)]
        ipv6: bool,
        /// Path to the inputfile
        #[clap(short = 'i', long, value_parser = filepath_parser)]
        infile: Option<PathBuf>,
        /// Paths to the files (comma separated)
        #[clap(short = 'f', long, num_args = 1.., value_parser = filepath_parser)]
        files: Option<Vec<PathBuf>>,
    },
    /// Connect to hosted server by providing address, output folder and access key
    Connect {
        /// IP address of the server (IPv4 or IPv6)
        #[clap(short = 'a', long, value_parser = addr_parser)]
        address: SocketAddr,
        /// Path to the output folder
        #[clap(short = 'o', long, value_parser = dirpath_parser)]
        outdir: PathBuf,
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
