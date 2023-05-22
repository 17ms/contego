use std::{error::Error, net::SocketAddr, path::PathBuf};

use clap::{command, ArgGroup, Parser, Subcommand};

use contego::parsers::{addr_parser, dirpath_parser, filepath_parser};

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
    #[clap(group(ArgGroup::new("input").required(true).args(&["infile", "files"])))]
    Host {
        /// Host port
        #[clap(short = 'p', long, default_value_t = 8080)]
        port: u16,
        /// Use IPv6 instead of IPv4
        #[clap(short = '6', long, default_value_t = false)]
        ipv6: bool,
        /// Path to the inputfile
        #[clap(short = 'i', long, value_parser = filepath_parser, conflicts_with = "files", group = "input")]
        infile: Option<PathBuf>,
        /// Paths to the files
        #[clap(short = 'f', long, num_args = 1.., value_parser = filepath_parser, conflicts_with = "infile", group = "input")]
        files: Option<Vec<PathBuf>>,
        /// Outgoing traffic chunksize in bytes
        #[clap(short = 'c', long, default_value_t = 8192)]
        chunksize: usize,
        /// Host the files locally
        #[clap(short = 'l', long, default_value_t = false)]
        local: bool,
    },
    /// Connect to hosted server by providing address, output folder and access key
    Connect {
        /// IP address of the server (IPv4 or IPv6)
        #[clap(short = 'a', long, value_parser = addr_parser)]
        addr: SocketAddr,
        /// Path to the output folder
        #[clap(short = 'o', long, value_parser = dirpath_parser)]
        out: PathBuf,
        /// Access key for the fileserver
        #[clap(short = 'k', long)]
        key: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Host {
            port,
            ipv6,
            infile,
            files,
            chunksize,
            local,
        } => {}
        Commands::Connect { addr, out, key } => {}
    };

    Ok(())
}
