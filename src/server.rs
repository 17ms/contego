use local_ip_address::local_ip;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::{
    error::Error,
    fs::read_dir,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    process::exit,
    str::FromStr,
    time::Duration,
};
use tokio::{
    self,
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener,
    },
    time::timeout,
};

pub async fn listen(
    port: u16,
    fileroot: PathBuf,
    buffersize: usize,
    localhost: bool,
    timeout_duration: u64,
    use_testing_key: bool,
) -> Result<(), Box<dyn Error>> {
    let addr = match localhost {
        true => SocketAddr::new(IpAddr::from_str("127.0.0.1")?, port),
        false => SocketAddr::new(local_ip()?, port),
    };
    // Use weak access key for integration testing, otherwise 8 char alphanumeric
    let access_key = match use_testing_key {
        true => "test".to_string(),
        false => generate_key(),
    };

    let listener = TcpListener::bind(addr).await?;
    println!("[+] Listening on {}", addr);
    println!("[+] Access key: {}", access_key);

    loop {
        // The first loop iteration would take the ownership without cloning
        let alt_fileroot = fileroot.clone();
        let alt_access_key = access_key.clone();

        let (mut socket, addr) =
            match timeout(Duration::from_secs(timeout_duration), listener.accept()).await {
                Ok(n) => n?,
                Err(_) => {
                    println!(
                        "\n[-] Connection timed out after {} seconds",
                        timeout_duration
                    );
                    break;
                }
            };

        println!("\n[NEW] {}: Connected", addr);

        tokio::spawn(async move {
            let (reader, writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut writer = BufWriter::new(writer);

            let mut vec_buf = Vec::new();

            // ACK ready-to-receive status
            send_msg(&mut writer, "SYN\n").await?;

            // Check access key
            if !check_access_key(&mut reader, &mut writer, &mut vec_buf, &alt_access_key).await? {
                println!("[FIN] {}: Incorrect access key", addr);
                return Ok::<(), Box<dyn Error + Send + Sync>>(());
            }

            // Send buffersize
            send_msg(&mut writer, (buffersize.to_string() + "\n").as_str()).await?;

            // ACK buffersize
            if recv_msg_string(&mut reader, &mut vec_buf).await? != "ACK" {
                return Ok::<(), Box<dyn Error + Send + Sync>>(());
            }

            // Send metadata
            match handle_metadata(&mut reader, &mut writer, &mut vec_buf, &alt_fileroot, &addr)
                .await?
            {
                None => println!("[DATA] {}: Ready to serve files", addr),
                Some(err_msg) => {
                    println!("{}", err_msg);
                    exit(0x0100);
                }
            }

            // Send filedata
            match handle_file_reqs(
                &mut reader,
                &mut writer,
                &mut vec_buf,
                &alt_fileroot,
                &buffersize,
                &addr,
            )
            .await?
            {
                None => println!("[FIN] {}: Disconnected", addr),
                Some(err_msg) => {
                    println!("{}", err_msg);
                    exit(0x0100);
                }
            }

            Ok::<(), Box<dyn Error + Send + Sync>>(())
        });
    }

    Ok(())
}

async fn get_metadata(
    fileroot: &PathBuf,
) -> Result<(Vec<(String, u64)>, usize), Box<dyn Error + Send + Sync>> {
    let mut metadata = Vec::<(String, u64)>::new();
    let paths = read_dir(fileroot)?;

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

async fn handle_metadata(
    reader: &mut BufReader<ReadHalf<'_>>,
    writer: &mut BufWriter<WriteHalf<'_>>,
    buf: &mut Vec<u8>,
    fileroot: &PathBuf,
    addr: &SocketAddr,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let (metadata_list, file_amount) = get_metadata(fileroot).await?;

    // Terminate if fileroot is empty
    if file_amount == 0 {
        send_msg(writer, "FIN\n").await?;
        return Ok(Some(format!(
            "[-] No files inside {:#?}, shutting host down",
            fileroot
        )));
    }

    // Send metadata amount
    send_msg(writer, (file_amount.to_string() + "\n").as_str()).await?;

    // ACK metadata amount
    if recv_msg_string(reader, buf).await? != "ACK" {
        return Ok(Some(format!(
            "[ERROR] {}: No confirmation of metadata amount",
            addr
        )));
    }

    // Send metadata
    for file in &metadata_list {
        send_msg(writer, format!("{}:{}\n", file.1, file.0).as_str()).await?;
    }

    Ok(None)
}

fn generate_key() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect::<String>()
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
            format!("No message received or client crashed").into();
        return Err::<String, Box<dyn Error + Send + Sync>>(e);
    }

    let msg = String::from_utf8(buf.clone())?;
    buf.clear();

    Ok(msg.trim().to_string())
}

async fn check_access_key(
    reader: &mut BufReader<ReadHalf<'_>>,
    writer: &mut BufWriter<WriteHalf<'_>>,
    buf: &mut Vec<u8>,
    access_key: &String,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    if recv_msg_string(reader, buf).await? != *access_key {
        send_msg(writer, "FIN\n").await?;
        return Ok(false);
    } else {
        send_msg(writer, "ACK\n").await?;
        recv_msg_string(reader, buf).await?; // Might be a bit unnecessary ACK
        return Ok(true);
    }
}

async fn handle_file_reqs(
    reader: &mut BufReader<ReadHalf<'_>>,
    writer: &mut BufWriter<WriteHalf<'_>>,
    buf: &mut Vec<u8>,
    fileroot: &PathBuf,
    buffersize: &usize,
    addr: &SocketAddr,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    loop {
        // Receive filename or termination request
        let req = recv_msg_string(reader, buf).await?;

        if req == "FIN" {
            break;
        }

        let mut input_path = fileroot.clone();
        input_path.push(req);

        println!("\n[REQ] {}: {:#?}", addr, input_path);
        let mut file = File::open(input_path.clone()).await?;
        let mut remaining_data = file.metadata().await?.len();
        let mut filebuf = vec![0u8; *buffersize];

        // Serve the file itself
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

        // ACK file
        if recv_msg_string(reader, buf).await? != "ACK" {
            return Ok(Some(format!(
                "[ERROR] {}: No confirmation of file {:#?}",
                addr, input_path
            )));
        } else {
            println!("[ACK] {}: File finished successfully", addr);
        }
    }

    Ok(None)
}
