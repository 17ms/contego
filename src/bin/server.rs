use std::fs::read_dir;
use tokio::{
    self,
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
};

// TODO: Remove panics/unwraps & add proper error handling

const BUFFERSIZE: usize = 8192;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Clap
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await?;
    println!("[+] Listening on {}", addr);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("[+] New client: {}", addr);

        tokio::spawn(async move {
            let (reader, writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut writer = BufWriter::new(writer);

            let mut vec_buf = Vec::new();

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

                    let input_path = String::from("./data/") + msg.as_str();

                    println!("[+] File requested: {}", input_path);
                    let mut file = File::open(input_path.clone()).await.unwrap();
                    let mut remaining_data = file.metadata().await.unwrap().len();
                    let mut filebuf = [0u8; BUFFERSIZE];

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
                    println!("[+] File transfer successfully done\n");
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

        metadata.push((filename, filesize));
    }

    let amount = metadata.len();

    (metadata, amount)
}
