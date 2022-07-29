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
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpStream,
    },
    time::sleep,
};

pub async fn connect(
    addr: String,
    fileroot: PathBuf,
    access_key: String,
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

        // Authenticate connection
        match authenticate_connection(&mut reader, &mut writer, &mut buf, &access_key).await? {
            None => println!("[+] Connection authenticated successfully"),
            Some(err_msg) => {
                println!("{}", err_msg);
                exit(0x0100);
            }
        }

        // Receive buffersize
        let buffersize = recv_msg_string(&mut reader, &mut buf)
            .await?
            .parse::<usize>()?;
        println!("[+] Selected buffersize: {}", buffersize);

        // ACK buffersize
        send_msg(&mut writer, "ACK\n").await?;

        // Receive metadata
        let metadata = match receive_metadata(&mut reader, &mut writer, &mut buf).await? {
            Some(metadata) => metadata,
            None => exit(0x0100),
        };

        println!("[+] Received metadata: {:#?}", metadata);

        // Send request for each file by filename
        println!("[+] [<Filename> + Enter] to make a request\n");
        handle_file_reqs(
            &mut reader,
            &mut writer,
            rx,
            &buffersize,
            &metadata,
            &fileroot,
            &download_all,
        )
        .await?;

        // Terminating connection
        println!("[+] Requesting connection termination");
        writer.write_all(b"FIN\n").await?;
        writer.flush().await?;

        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });

    // Separate thread for blocking stdin
    let input_task = thread::spawn(move || handle_stdin(tx));

    match connection_task.join().unwrap().await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("[ERROR] Error inside connection thread: {}", e);
            exit(0x0100);
        }
    }

    if !download_all {
        match input_task.join().unwrap() {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[ERROR] Error inside input thread: {}", e);
                exit(0x0100);
            }
        }
    }

    Ok(())
}

async fn send_msg(
    writer: &mut BufWriter<WriteHalf<'_>>,
    msg: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    writer.write_all(msg.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn recv_msg_string(
    reader: &mut BufReader<ReadHalf<'_>>,
    buf: &mut Vec<u8>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let bytes_received = reader.read_until(b'\n', buf).await?;

    if bytes_received == 0 {
        let e: Box<dyn Error + Send + Sync> =
            format!("No message received or server crashed").into();
        return Err::<String, Box<dyn Error + Send + Sync>>(e);
    }

    let msg = String::from_utf8(buf.clone())?;
    buf.clear();

    Ok(msg.trim().to_string())
}

fn handle_stdin(tx: Sender<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut input_string = String::new();
    while input_string.trim() != "DISCONNECT" {
        input_string.clear();
        stdin().read_line(&mut input_string)?;
        print!("\n");
        tx.send(input_string.clone())?;
    }

    Ok::<(), Box<dyn Error + Send + Sync>>(())
}

async fn authenticate_connection(
    reader: &mut BufReader<ReadHalf<'_>>,
    writer: &mut BufWriter<WriteHalf<'_>>,
    buf: &mut Vec<u8>,
    access_key: &String,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    // Receive ACK to indicate ready-to-receive status
    if recv_msg_string(reader, buf).await? != "SYN" {
        return Ok(Some(
            "[-] Server is not ready to receive access key, terminating connection".to_string(),
        ));
    }

    // Send access key
    send_msg(writer, (access_key.to_string() + "\n").as_str()).await?;

    // Terminate connection if key is invalid
    if recv_msg_string(reader, buf).await? == "FIN" {
        return Ok(Some(
            "[-] Incorrect access key, terminating connection".to_string(),
        ));
    } else {
        send_msg(writer, "ACK\n").await?;
        Ok(None)
    }
}

async fn receive_metadata(
    reader: &mut BufReader<ReadHalf<'_>>,
    writer: &mut BufWriter<WriteHalf<'_>>,
    buf: &mut Vec<u8>,
) -> Result<Option<HashMap<String, u64>>, Box<dyn Error + Send + Sync>> {
    // Receive file amount or terminate if no files available
    let msg = recv_msg_string(reader, buf).await?;
    if msg == "FIN" {
        println!("[-] Server does not have any files available, closing connection");
        return Ok(None);
    }

    let file_amount = msg.parse::<usize>()?;
    println!("[+] Total of {} files available", file_amount);

    // ACK file amount
    send_msg(writer, "ACK\n").await?;

    // Receive file metadata
    let mut metadata = HashMap::new();
    while metadata.len() < file_amount {
        let msg = recv_msg_string(reader, buf).await?;

        // Parse 'filesize:filename'
        let split = msg.split(":").collect::<Vec<&str>>();
        let filesize = split[0].trim().parse::<u64>()?;
        let filename = split[1].trim().to_string();

        metadata.insert(filename, filesize);
    }

    Ok(Some(metadata))
}

async fn create_filehandle(
    fileroot: &PathBuf,
    filename: &String,
) -> Result<(BufWriter<File>, PathBuf), Box<dyn Error + Send + Sync>> {
    let mut output_path = fileroot.clone();
    output_path.push(&filename);
    let output_file = File::create(output_path.clone()).await?;
    println!("[+] New file: {:#?}", output_path);

    Ok((BufWriter::new(output_file), output_path))
}

async fn handle_file_reqs(
    reader: &mut BufReader<ReadHalf<'_>>,
    writer: &mut BufWriter<WriteHalf<'_>>,
    rx: Receiver<String>,
    buffersize: &usize,
    metadata: &HashMap<String, u64>,
    fileroot: &PathBuf,
    download_all: &bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let filenames = metadata.keys().collect::<Vec<&String>>();
    let mut filenames_iter = filenames.iter();

    let mut input_string = String::new();

    loop {
        input_string.clear();

        if *download_all {
            match filenames_iter.next() {
                Some(filename) => {
                    input_string.push_str(filename);
                }
                None => input_string.push_str("DISCONNECT"),
            }
        } else {
            // Blocks the current thread until a message is readable
            // Requests (messages) get queued if they can't be served immediately
            let msg = rx.recv()?;
            input_string.push_str(msg.trim());
        }

        // Terminate connection on request
        if input_string == "DISCONNECT" {
            break;
        } else if !metadata.contains_key(input_string.as_str()) {
            println!("[-] No file named '{}' available\n", input_string);
            continue;
        }

        // Handle request based on input received from channel
        println!("[+] Requesting file named '{}'", input_string);
        send_msg(writer, (input_string.to_string() + "\n").as_str()).await?;

        // Create file locally
        let (mut file_buf, output_path) = create_filehandle(&fileroot, &input_string).await?;

        // Receive the file itself
        let filesize = metadata.get(input_string.as_str()).unwrap().clone();
        receive_file(reader, &mut file_buf, &filesize, buffersize).await?;

        // ACK file
        send_msg(writer, "ACK\n").await?;
        println!(
            "[+] Successfully wrote {} bytes to {:#?}\n",
            filesize, output_path
        );
    }

    Ok(())
}

async fn receive_file(
    reader: &mut BufReader<ReadHalf<'_>>,
    file_buf: &mut BufWriter<File>,
    filesize: &u64,
    buffersize: &usize,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut remaining_data = *filesize;
    let mut buf = vec![0u8; *buffersize];

    while remaining_data != 0 {
        if remaining_data >= *buffersize as u64 {
            let read_result = reader.read(&mut buf);

            match read_result.await {
                Ok(0) => {
                    println!("[-] Connection lost, trying again until [Ctrl + V]...");
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

    Ok(())
}
