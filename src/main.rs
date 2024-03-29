use std::{error::Error, net::SocketAddr, path::PathBuf};

use clap::{command, ArgGroup, Parser, Subcommand};

use contego::{
    client::Client,
    parser::{addr_parser, dirpath_parser, filepath_parser},
    server::Server,
    util::{ascii, filepaths, metadata, Ip},
};
use env_logger::Env;
use log::{error, info};
use tokio::{signal, sync::mpsc};

#[derive(Debug, Parser)]
#[command(about, version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[clap(group(ArgGroup::new("input").required(true).args(&["source", "files"])))]
    Host {
        /// Access key
        #[clap(short = 'k', long)]
        key: String,
        /// Path to a source file (alternative to --files)
        #[clap(short = 's', long, value_parser = filepath_parser, conflicts_with = "files", group = "input")]
        source: Option<PathBuf>,
        /// Paths to shareable files (alternative to --source)
        #[clap(short = 'f', long, num_args = 1.., value_parser = filepath_parser, conflicts_with = "source", group = "input")]
        files: Option<Vec<PathBuf>>,
        /// Host port
        #[clap(short = 'p', long, default_value_t = 8080)]
        port: u16,
        /// IPv6 instead of IPv4
        #[clap(short = '6', long, default_value_t = false)]
        ipv6: bool,
        /// Transmit chunksize in bytes
        #[clap(short = 'c', long, default_value_t = 8192)]
        chunksize: usize,
        /// Host locally
        #[clap(short = 'l', long, default_value_t = false)]
        local: bool,
    },
    Connect {
        /// IP address of the instance
        #[clap(short = 'a', long, value_parser = addr_parser)]
        addr: SocketAddr,
        /// Path to an output folder
        #[clap(short = 'o', long, value_parser = dirpath_parser)]
        out: PathBuf,
        /// Access key
        #[clap(short = 'k', long)]
        key: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    ascii();

    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Host {
            port,
            ipv6,
            source,
            files,
            chunksize,
            local,
            key,
        } => {
            let (tx, rx) = mpsc::channel::<()>(1);

            let paths = filepaths(source, files)?;
            let (metadata, index) = metadata(&paths).await?;
            let (display_addr, bind_addr) = match (local, ipv6) {
                (true, _) => Ip::Local.fetch(port)?,
                (false, true) => Ip::V6.fetch(port)?,
                (false, false) => Ip::V4.fetch(port)?,
            };

            let server = Server::new(display_addr, key, chunksize, metadata, index);

            tokio::spawn(async move {
                match server.start(rx, &bind_addr).await {
                    Ok(_) => {}
                    Err(e) => error!("Error during server execution: {}", e),
                };
            });

            match signal::ctrl_c().await {
                Ok(_) => {
                    tx.send(()).await?;
                    info!("Captured Ctrl+C, shutting down");
                }
                Err(_) => error!("Failed to listen for a Ctrl+C event"),
            };
        }
        Commands::Connect { addr, out, key } => {
            let client = Client::new(addr, key, out);
            match client.connection().await {
                Ok(_) => {}
                Err(e) => error!("Error during client execution: {}", e),
            };
        }
    };

    Ok(())
}
