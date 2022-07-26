use std::{
    collections::HashMap,
    error::Error,
    io::stdin,
    path::PathBuf,
    process::exit,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
    time::sleep,
};

pub async fn connect(
    addr: String,
    fileroot: PathBuf,
    download_all: bool,
) -> Result<(), Box<dyn Error>> {
    let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();

    let connection_task = thread::spawn(move || async move {
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
            let mut metadata = HashMap::new();
            while metadata.len() < file_amount {
                reader.read_until(b'\n', &mut buf).await?;
                let msg = String::from_utf8(buf.clone())?;
                buf.clear();

                // Parse 'filesize:filename'
                let split = msg.split(":").collect::<Vec<&str>>();
                let filesize = split[0].trim().parse::<u64>()?;
                let filename = split[1].trim().to_string();

                metadata.insert(filename, filesize);
            }
            println!("[INFO] Metadata: {:?}", metadata);

            // Send request for each file by filename
            println!("[+] Requesting files individually\n");
            let filenames = metadata.keys().collect::<Vec<&String>>();
            let mut filenames_iter = filenames.iter();

            let mut input = String::new();
            loop {
                input.clear();

                if download_all {
                    match filenames_iter.next() {
                        Some(filename) => {
                            input.push_str(filename);
                        }
                        None => input.push_str("DISCONNECT"),
                    }
                } else {
                    // Blocks the current thread until a message is readable
                    // Requests (messages) get queued if they can't be served immediately
                    let msg = rx.recv()?;

                    input.push_str(msg.trim());
                }

                if input == "DISCONNECT" {
                    break;
                } else if !metadata.contains_key(input.as_str()) {
                    println!("[-] No file named '{}' available\n", input);
                    continue;
                }

                // Handle request based on input received from channel
                println!("[+] Requesting file named '{}'", input);
                let msg = input.to_string() + "\n";
                writer.write_all(msg.as_bytes()).await?;
                writer.flush().await?;

                // Create file locally
                let mut output_path = fileroot.clone();
                output_path.push(input.clone());

                let output_file = File::create(output_path.clone()).await?;
                println!("[+] New file: {:#?}", output_path);
                let mut file_buf = BufWriter::new(output_file);

                // Receive the file itself
                let filesize = metadata.get(input.as_str()).unwrap().clone();
                let mut remaining_data = filesize;
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
                    filesize, output_path
                );
            }

            println!("[+] Requesting connection termination");
            writer.write_all(b"FIN\n").await?;
            writer.flush().await?;
        }

        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });

    // Separate thread for blocking stdin
    let input_task = thread::spawn(move || {
        let mut input_string = String::new();
        while input_string.trim() != "DISCONNECT" {
            input_string.clear();
            stdin().read_line(&mut input_string)?;
            print!("\n");
            tx.send(input_string.clone())?;
        }

        Ok::<(), Box<dyn Error + Send + Sync>>(())
    });

    match connection_task.join().unwrap().await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("[-] Error inside connection thread: {}", e);
            exit(0x0100);
        }
    }

    if !download_all {
        match input_task.join().unwrap() {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[-] Error inside input thread: {}", e);
                exit(0x0100);
            }
        }
    }

    Ok(())
}
