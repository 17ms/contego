use clap::Parser;
use local_ip_address::local_ip;
use std::{
    fs::read_dir,
    net::{IpAddr, SocketAddr},
    str::FromStr,
};
use tokio::{
    self,
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
};

// TODO: Remove panics/unwraps & add proper error handling

#[derive(Parser, Debug)]
#[clap(author, about, version)]
struct Args {
    #[clap(default_value_t = 8080u16, short = 'p', long, value_parser = validate_port)]
    port: u16,
    #[clap(default_value = "./data/", short = 'f', long, value_parser)]
    fileroot: String,
    #[clap(default_value_t = 8192usize, short = 'b', long, value_parser = validate_buffersize)]
    buffersize: usize,
    #[clap(default_value_t = false, short = 'l', long, action)]
    local: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = match args.local {
        true => SocketAddr::new(IpAddr::from_str("127.0.0.1")?, args.port),
        false => SocketAddr::new(local_ip()?, args.port),
    };

    let listener = TcpListener::bind(addr).await?;
    println!("[+] Listening on {}", addr);

    loop {
        let args = Args::parse();
        let buffersize = args.buffersize;
        let fileroot = args.fileroot;

        let (mut socket, addr) = listener.accept().await?;
        println!("\n[+] New client: {}", addr);

        tokio::spawn(async move {
            let (reader, writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut writer = BufWriter::new(writer);

            let mut vec_buf = Vec::new();

            // Send buffersize
            writer
                .write_all(buffersize.to_string().as_bytes())
                .await
                .unwrap();
            writer.flush().await.unwrap();

            // Read ACK
            let _bytes_read = reader.read_buf(&mut vec_buf).await.unwrap();
            if String::from_utf8(vec_buf.clone()).unwrap() != "ACK" {
                panic!("ACK not received (buffersize)");
            } else {
                vec_buf.clear();
            }

            let (metadata_list, file_amount) = get_metadata().await;

            // Send file amount
            writer
                .write_all(file_amount.to_string().as_bytes())
                .await
                .unwrap();
            writer.flush().await.unwrap();

            // Read ACK
            let _bytes_read = reader.read_buf(&mut vec_buf).await.unwrap();
            if String::from_utf8(vec_buf.clone()).unwrap() != "ACK" {
                panic!("ACK not received (amount)");
            } else {
                vec_buf.clear();
            }

            // Send file metadata
            for file in &metadata_list {
                // Newline as delimiter between instances
                let msg = format!("{}:{}\n", file.1, file.0);
                writer.write_all(msg.as_bytes()).await.unwrap();
                writer.flush().await.unwrap();
            }

            // Handle file request(s)
            println!("[+] Ready to serve files");
            loop {
                let bytes_read = reader.read_buf(&mut vec_buf).await.unwrap();

                if bytes_read == 0 {
                    println!("File request never received");
                    break;
                } else {
                    let msg = String::from_utf8(vec_buf.clone()).unwrap();
                    vec_buf.clear();

                    if msg == "FIN" {
                        println!("[+] FIN received, terminating connection...");
                        break;
                    }

                    let input_path = fileroot.clone() + msg.as_str();

                    println!("\n[+] File requested: {}", input_path);
                    let mut file = File::open(input_path.clone()).await.unwrap();
                    let mut remaining_data = file.metadata().await.unwrap().len();
                    let mut filebuf = vec![0u8; buffersize];

                    while remaining_data != 0 {
                        let read_result = file.read(&mut filebuf);
                        match read_result.await {
                            Ok(n) => {
                                writer.write_all(&filebuf).await.unwrap();
                                writer.flush().await.unwrap();
                                remaining_data = remaining_data - n as u64;
                            }
                            _ => {}
                        }
                    }
                }

                // Read ACK
                let _bytes_read = reader.read_buf(&mut vec_buf).await.unwrap();
                if String::from_utf8(vec_buf.clone()).unwrap() != "ACK" {
                    panic!("ACK not received (amount)");
                } else {
                    println!("[+] File transfer successfully done");
                    vec_buf.clear();
                }
            }
        });
    }
}

async fn get_metadata() -> (Vec<(String, u64)>, usize) {
    let mut metadata = Vec::<(String, u64)>::new();
    let paths = read_dir("./data").unwrap();

    for filename in paths {
        let filepath = filename.unwrap().path().display().to_string(); // ????
        let split = filepath.split("/").collect::<Vec<&str>>();
        let filename = split[split.len() - 1].to_string();
        let file = File::open(filepath).await.unwrap();
        let filesize = file.metadata().await.unwrap().len();

        if filesize > 0 {
            metadata.push((filename, filesize));
        }
    }

    let amount = metadata.len();

    (metadata, amount)
}

fn validate_buffersize(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(n) => Ok(n),
        Err(_) => Err(format!("Invalid buffersize: {}", value)),
    }
}

fn validate_port(value: &str) -> Result<u16, String> {
    match value.parse::<u16>() {
        Ok(n) => Ok(n),
        Err(_) => Err(format!("Invalid port-number: {}", value)),
    }
}
