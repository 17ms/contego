use clap::Parser;
use fragilebyte::{client, server};
use std::{path::PathBuf, str::FromStr};
use tokio;

#[derive(Parser, Debug)]
#[clap(author, about, version, long_about = None)]
struct Args {
    #[clap(short = 't', long, value_parser)]
    /// Server's address when connecting as a client
    target: Option<String>,
    #[clap(default_value_t = 8080u16, short = 'p', long, value_parser = validate_arg::<u16>)]
    /// Port where the service is hosted
    port: u16,
    #[clap(default_value_t = 8192usize, short = 'b', long, value_parser = validate_arg::<usize>)]
    /// Buffersize used in the file transfer (bytes)
    buffersize: usize,
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.target {
        Some(addr) => {
            // Client
            let fileroot = match args.fileroot {
                Some(n) => n,
                None => PathBuf::from("./output"),
            };

            client::connect(addr, fileroot, args.all)
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
                args.buffersize,
                args.localhost,
                args.timeout,
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
