use std::time::Duration;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
    time::sleep,
};

// TODO: Remove panics/unwraps & add proper error handling

const BUFFERSIZE: usize = 8192;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Clap
    let addr = "127.0.0.1:8080";
    let mut stream = TcpStream::connect(addr).await?;
    println!("[+] Connecting to {}", addr);

    let (reader, writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    let mut buf = Vec::new();

    loop {
        let bytes_read = reader.read_buf(&mut buf).await.unwrap();
        if bytes_read == 0 {
            println!("[-] No more bytes received, disconnecting from the server...");
            break;
        }

        // Receive file amount
        let file_amount = String::from_utf8(buf.clone())
            .unwrap()
            .parse::<usize>()
            .unwrap();
        println!("[+] Total of {} files available", file_amount);
        buf.clear();

        // ACK file amount
        writer.write_all(b"ACK").await.unwrap();
        writer.flush().await.unwrap();

        // Receive file metadata
        println!("[+] Receiving file metadata");
        let mut metadata = Vec::<(String, u64)>::new();
        while metadata.len() < file_amount {
            reader.read_until(b'\n', &mut buf).await.unwrap();
            let msg = String::from_utf8(buf.clone()).unwrap();
            buf.clear();

            // Parse 'filesize:filename'
            let split = msg.split(":").collect::<Vec<&str>>();
            let filesize = split[0].trim().parse::<u64>().unwrap();
            let filename = split[1].trim().to_string();

            metadata.push((filename, filesize));
        }
        println!("[INFO] Metadata: {:?}", metadata);

        // Send request for each file by filename
        println!("[+] Requesting files individually"); // TODO: Choose files based on input
        for file in &metadata {
            println!("[INFO] Current request: [{:?}]", file);
            writer.write_all(file.0.as_bytes()).await.unwrap();
            writer.flush().await.unwrap();

            // Create file locally (./output/)
            let output_path = String::from("./output/") + file.0.as_str();

            let output_file = File::create(output_path.clone()).await.unwrap();
            println!("[+] New file: {}", output_path);
            let mut file_buf = BufWriter::new(output_file);

            // Receive the file itself
            let mut remaining_data = file.1;
            let mut buf = [0u8; BUFFERSIZE];

            while remaining_data != 0 {
                if remaining_data >= BUFFERSIZE as u64 {
                    let read_result = reader.read(&mut buf);

                    match read_result.await {
                        Ok(0) => {
                            println!("[-] Waiting for data to become available...");
                            sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                        Ok(n) => {
                            file_buf.write_all(&mut buf).await.unwrap();
                            file_buf.flush().await.unwrap();
                            remaining_data = remaining_data - n as u64;
                        }
                        _ => {}
                    }
                } else {
                    let read_result = reader.read(&mut buf);

                    match read_result.await {
                        Ok(_) => {
                            let mut buf_slice = &buf[0..(remaining_data as usize)];
                            file_buf.write_all(&mut buf_slice).await.unwrap();
                            file_buf.flush().await.unwrap();
                            remaining_data = 0;
                        }
                        _ => {}
                    }
                }
            }

            // ACK file
            writer.write_all(b"ACK").await.unwrap();
            writer.flush().await.unwrap();
            println!(
                "[+] Successfully wrote {} bytes to {}\n",
                file.1, output_path
            );
        }

        println!("[+] All finished, disconnecting...");
        writer.write_all(b"FIN").await.unwrap();
        writer.flush().await.unwrap();
    }

    Ok(())
}
