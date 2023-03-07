use clap::Parser;
use fragilebyte::{client, server};
use std::{error::Error, path::PathBuf, process::exit, str::FromStr};
use tokio;

#[derive(Parser, Debug)]
#[clap(author, about, version, long_about = None)]
struct Args {
    #[clap(short = 't', long, value_parser)]
    /// Server's address when connecting as a client
    target: Option<String>,
    #[clap(short = 'k', long, value_parser)]
    /// Alphanumeric 8 characters long key required to establish a connection to the host
    key: Option<String>,
    #[clap(default_value_t = 8080u16, short = 'p', long, value_parser = validate_arg::<u16>)]
    /// Port where the service is hosted
    port: u16,
    #[clap(default_value_t = 8192usize, short = 'b', long, value_parser = validate_arg::<usize>)]
    /// Chunksize used in the file transfer (bytes)
    chunksize: usize,
    #[clap(default_value_t = false, long, action)]
    /// Run only in the local network
    localhost: bool,
    #[clap(default_value_t = 30, long, value_parser = validate_arg::<u64>)]
    /// Seconds of inactivity after which the server closes itself
    timeout: u64,
    #[clap(short = 'f', long, value_parser)]
    /// Path to the folder where the files are outputted as a client or
    /// served from as a server [default: './output' / './data']
    fileroot: Option<PathBuf>,
    #[clap(default_value_t = false, short = 'a', long, action)]
    /// Automatically download every available file from the host (skips stdin)
    all: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    match args.target {
        Some(addr) => {
            // Client
            let fileroot = match args.fileroot {
                Some(n) => n,
                None => PathBuf::from("./output"),
            };
            let access_key = match args.key {
                Some(n) => n,
                None => {
                    eprintln!("[-] Access key required as a client, please try again");
                    exit(0x0100);
                }
            };

            client::connect(addr, fileroot, access_key, args.all)
                .await
                .expect("Error initializing client");
        }
        None => {
            // Server
            let fileroot = match args.fileroot {
                Some(n) => n,
                None => PathBuf::from("./data"),
            };

            server::listen(
                args.port,
                fileroot,
                args.chunksize,
                args.localhost,
                args.timeout,
                false,
            )
            .await
            .expect("Error initializing server");
        }
    }

    Ok(())
}

fn validate_arg<T: FromStr>(value: &str) -> Result<T, String> {
    match value.parse::<T>() {
        Ok(n) => Ok(n),
        Err(_) => Err(format!("Invalid argument: {}", value)),
    }
}
