use std::{path::PathBuf, time::Duration};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
    time::sleep,
};

pub async fn connect(addr: String, fileroot: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("[+] Connecting to {}", addr);
    let mut stream = TcpStream::connect(addr.clone()).await?;

    let (reader, writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    let mut buf = Vec::new();

    loop {
        let bytes_read = reader.read_buf(&mut buf).await?;
        if bytes_read == 0 {
            println!("[-] No more bytes received, closing connection");
            break;
        }

        // Receive buffersize
        let buffersize = String::from_utf8(buf.clone())?.parse::<usize>()?;
        println!("[+] Selected buffersize: {}", buffersize);
        buf.clear();

        // ACK buffersize
        writer.write_all(b"ACK\n").await.unwrap();
        writer.flush().await?;

        // Receive file amount (or termination request if the server does not have any files available)
        let file_amount: usize;
        let _bytes_read = reader.read_until(b'\n', &mut buf).await?;
        let msg = String::from_utf8(buf.clone())?;
        if msg.trim() == "FIN" {
            println!("[-] Server does not have any files available, closing connection");
            writer.write_all(b"FIN\n").await?;
            writer.flush().await?;
            break;
        } else {
            file_amount = msg.trim().parse::<usize>()?;
            println!("[+] Total of {} files available", file_amount);
            buf.clear();

            // ACK file amount
            writer.write_all(b"ACK\n").await?;
            writer.flush().await?;
        }

        // Receive file metadata
        println!("[+] Receiving file metadata");
        let mut metadata = Vec::<(String, u64)>::new();
        while metadata.len() < file_amount {
            reader.read_until(b'\n', &mut buf).await?;
            let msg = String::from_utf8(buf.clone())?;
            buf.clear();

            // Parse 'filesize:filename'
            let split = msg.split(":").collect::<Vec<&str>>();
            let filesize = split[0].trim().parse::<u64>()?;
            let filename = split[1].trim().to_string();

            metadata.push((filename, filesize));
        }
        println!("[INFO] Metadata: {:?}", metadata);

        // Send request for each file by filename
        // TODO: Choose files based on input
        println!("[+] Requesting files individually");
        for file in &metadata {
            println!("[INFO] Current request: [{:?}]", file);
            let msg = file.0.to_string() + "\n";
            writer.write_all(msg.as_bytes()).await?;
            writer.flush().await?;

            // Create file locally
            let mut output_path = fileroot.clone();
            output_path.push(file.0.clone());

            let output_file = File::create(output_path.clone()).await?;
            println!("[+] New file: {:#?}", output_path);
            let mut file_buf = BufWriter::new(output_file);

            // Receive the file itself
            let mut remaining_data = file.1;
            let mut buf = vec![0u8; buffersize];

            while remaining_data != 0 {
                if remaining_data >= buffersize as u64 {
                    let read_result = reader.read(&mut buf);

                    match read_result.await {
                        Ok(0) => {
                            println!("[-] Waiting for data to become available...");
                            sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                        Ok(n) => {
                            file_buf.write_all(&mut buf).await?;
                            file_buf.flush().await?;
                            remaining_data = remaining_data - n as u64;
                        }
                        _ => {}
                    }
                } else {
                    let read_result = reader.read(&mut buf);

                    match read_result.await {
                        Ok(_) => {
                            let mut buf_slice = &buf[0..(remaining_data as usize)];
                            file_buf.write_all(&mut buf_slice).await?;
                            file_buf.flush().await?;
                            remaining_data = 0;
                        }
                        _ => {}
                    }
                }
            }

            // ACK file
            writer.write_all(b"ACK\n").await?;
            writer.flush().await?;
            println!(
                "[+] Successfully wrote {} bytes to {:#?}\n",
                file.1, output_path
            );
        }

        println!("[+] All files finished, requesting connection termination");
        writer.write_all(b"FIN\n").await?;
        writer.flush().await?;
    }

    Ok(())
}
