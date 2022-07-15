use local_ip_address::local_ip;
use std::{
    fs::read_dir,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};
use tokio::{
    self,
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
    time::timeout,
};

pub async fn listen(
    port: u16,
    fileroot: PathBuf,
    buffersize: usize,
    localhost: bool,
    timeout_duration: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = match localhost {
        true => SocketAddr::new(IpAddr::from_str("127.0.0.1")?, port),
        false => SocketAddr::new(local_ip()?, port),
    };

    let listener = TcpListener::bind(addr).await?;
    println!("[+] Listening on {}", addr);

    loop {
        let alt_fileroot = fileroot.clone();

        let (mut socket, addr) =
            match timeout(Duration::from_secs(timeout_duration), listener.accept()).await {
                Ok(n) => n?,
                Err(_) => {
                    println!("\nConnection timed out after {} seconds", timeout_duration);
                    break;
                }
            };
        println!("\n[+] New client: {}", addr);

        tokio::spawn(async move {
            let (reader, writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut writer = BufWriter::new(writer);

            let mut vec_buf = Vec::new();

            // Send buffersize
            writer.write_all(buffersize.to_string().as_bytes()).await?;
            writer.flush().await?;

            // Read ACK
            let _bytes_read = reader.read_buf(&mut vec_buf).await?;
            if String::from_utf8(vec_buf.clone())? != "ACK" {
                panic!("ACK not received (buffersize)");
            } else {
                vec_buf.clear();
            }

            let (metadata_list, file_amount) = get_metadata().await?;

            // Send file amount
            writer.write_all(file_amount.to_string().as_bytes()).await?;
            writer.flush().await?;

            // Read ACK
            let _bytes_read = reader.read_buf(&mut vec_buf).await?;
            if String::from_utf8(vec_buf.clone())? != "ACK" {
                panic!("ACK not received (amount)");
            } else {
                vec_buf.clear();
            }

            // Send file metadata
            for file in &metadata_list {
                // Newline as delimiter between instances
                let msg = format!("{}:{}\n", file.1, file.0);
                writer.write_all(msg.as_bytes()).await?;
                writer.flush().await?;
            }

            // Handle file request(s)
            println!("[+] Ready to serve files");
            loop {
                let bytes_read = reader.read_buf(&mut vec_buf).await?;

                if bytes_read == 0 {
                    println!("File request never received");
                    break;
                } else {
                    let msg = String::from_utf8(vec_buf.clone())?;
                    vec_buf.clear();

                    if msg == "FIN" {
                        println!("[+] FIN received, terminating individual connection...");
                        break;
                    }

                    let mut input_path = alt_fileroot.clone();
                    input_path.push(msg);

                    println!("\n[+] File requested: {:#?}", input_path);
                    let mut file = File::open(input_path.clone()).await?;
                    let mut remaining_data = file.metadata().await?.len();
                    let mut filebuf = vec![0u8; buffersize];

                    while remaining_data != 0 {
                        let read_result = file.read(&mut filebuf);
                        match read_result.await {
                            Ok(n) => {
                                writer.write_all(&filebuf).await?;
                                writer.flush().await?;
                                remaining_data = remaining_data - n as u64;
                            }
                            _ => {}
                        }
                    }
                }

                // Read ACK
                let _bytes_read = reader.read_buf(&mut vec_buf).await?;
                if String::from_utf8(vec_buf.clone())? != "ACK" {
                    panic!("ACK not received (amount)");
                } else {
                    println!("[+] File transfer successfully done");
                    vec_buf.clear();
                }
            }

            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });
    }

    Ok(())
}

async fn get_metadata(
) -> Result<(Vec<(String, u64)>, usize), Box<dyn std::error::Error + Send + Sync>> {
    let mut metadata = Vec::<(String, u64)>::new();
    let paths = read_dir("./data")?;

    for filename in paths {
        let filepath = filename?.path().display().to_string();
        let split = filepath.split("/").collect::<Vec<&str>>();
        let filename = split[split.len() - 1].to_string();
        let file = File::open(filepath).await?;
        let filesize = file.metadata().await?.len();

        if filesize > 0 {
            metadata.push((filename, filesize));
        }
    }

    let amount = metadata.len();

    Ok((metadata, amount))
}
