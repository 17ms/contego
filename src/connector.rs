use crate::{
    common::{Connection, Message},
    comms, crypto,
};
use std::{collections::HashMap, error::Error, net::SocketAddr, path::PathBuf};
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
    net::TcpStream,
    sync::mpsc,
};

#[derive(Debug)]
pub struct Request {
    pub name: String,
    pub size: u64,
    pub hash: String,
}

impl Request {
    pub fn new(name: String, metadata: &HashMap<String, (u64, String)>) -> Option<Self> {
        let (size, hash) = metadata.get(&name)?.clone();
        Some(Self { name, size, hash })
    }
}

#[derive(Debug, Clone)]
pub struct Connector {
    target_addr: SocketAddr,
    access_key: &'static str,
    output_path: PathBuf,
}

impl Connector {
    pub fn new(target_addr: SocketAddr, access_key: &'static str, output_path: PathBuf) -> Self {
        Self {
            target_addr,
            access_key,
            output_path,
        }
    }

    pub async fn connect(
        self,
        tx: mpsc::Sender<Message>,
        mut rx: mpsc::Receiver<Message>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut socket = TcpStream::connect(self.target_addr).await?;
        let mut connection = Connection::new(&mut socket).await?;

        self.authorize(&mut connection).await?;
        let metadata = self.metadata(&mut connection).await?;
        tx.send(Message::Metadata(metadata.clone())).await?;
        self.request_handler(&mut connection, &mut rx, &metadata)
            .await?;

        let msg = b"FIN".to_vec();
        comms::send(
            &mut connection.writer,
            Some(&mut connection.cipher),
            Some(&mut connection.rng),
            &msg,
        )
        .await?;

        Ok(())
    }

    async fn authorize(
        &self,
        conn: &mut Connection<'_>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let msg = self.access_key.to_string().as_bytes().to_vec();
        comms::send(
            &mut conn.writer,
            Some(&mut conn.cipher),
            Some(&mut conn.rng),
            &msg,
        )
        .await?;

        let buf = comms::recv(&mut conn.reader, Some(&mut conn.cipher)).await?;
        let msg = String::from_utf8(buf)?;

        if msg == "FIN" {
            todo!("maybe error handling :)");
        }

        Ok(())
    }

    async fn metadata(
        &self,
        conn: &mut Connection<'_>,
    ) -> Result<HashMap<String, (u64, String)>, Box<dyn Error + Send + Sync>> {
        let buf = comms::recv(&mut conn.reader, Some(&mut conn.cipher)).await?;
        let amt: usize = String::from_utf8(buf)?.parse()?;

        let msg = b"AMT".to_vec();
        comms::send(
            &mut conn.writer,
            Some(&mut conn.cipher),
            Some(&mut conn.rng),
            &msg,
        )
        .await?;

        let mut metadata = HashMap::new();

        while metadata.len() < amt {
            let buf = comms::recv(&mut conn.reader, Some(&mut conn.cipher)).await?;
            let msg = String::from_utf8(buf)?;

            let split: Vec<&str> = msg.split(":").collect();
            let name = split[0].trim().to_string();
            let size: u64 = split[1].trim().parse()?;
            let hash = split[2].trim().to_string();

            metadata.insert(name, (size, hash));
        }

        Ok(metadata)
    }

    async fn new_handle(
        &self,
        filename: &str,
    ) -> Result<(BufWriter<File>, String), Box<dyn Error + Send + Sync>> {
        let mut dir_path = self.output_path.clone();
        dir_path.push(filename);
        let str_path = dir_path.to_str().unwrap().to_string();
        let filehandle = File::create(dir_path).await?;

        Ok((BufWriter::new(filehandle), str_path))
    }

    async fn request(
        &self,
        conn: &mut Connection<'_>,
        req: Request,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let (mut handle, path) = self.new_handle(&req.name).await?;
        let msg = req.hash.as_bytes().to_vec();
        comms::send(
            &mut conn.writer,
            Some(&mut conn.cipher),
            Some(&mut conn.rng),
            &msg,
        )
        .await?;

        let mut remaining = req.size.clone();

        while remaining != 0 {
            let buf = comms::recv(&mut conn.reader, Some(&mut conn.cipher)).await?;
            handle.write_all(&buf).await?;
            handle.flush().await?;
            remaining -= buf.len() as u64;
        }

        let msg: Vec<u8>;
        let new_hash = crypto::try_hash(&path)?;

        if new_hash == req.hash {
            msg = b"OK".to_vec();
        } else {
            msg = b"ERROR".to_vec();
        }

        comms::send(
            &mut conn.writer,
            Some(&mut conn.cipher),
            Some(&mut conn.rng),
            &msg,
        )
        .await?;

        Ok(true)
    }

    async fn request_handler(
        &self,
        conn: &mut Connection<'_>,
        rx: &mut mpsc::Receiver<Message>,
        metadata: &HashMap<String, (u64, String)>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        loop {
            let rx_msg = rx.recv().await;

            match rx_msg.unwrap() {
                Message::ClientReq(name) => {
                    let req = Request::new(name, metadata).unwrap(); // TODO: handle
                    self.request(conn, req).await?;
                }
                Message::ClientReqAll => {
                    for name in metadata.keys() {
                        let req = Request::new(name.clone(), metadata).unwrap(); // TODO: handle
                        self.request(conn, req).await?;
                    }
                }
                Message::Shutdown => {
                    let msg = b"DISCONNECT".to_vec();
                    comms::send(
                        &mut conn.writer,
                        Some(&mut conn.cipher),
                        Some(&mut conn.rng),
                        &msg,
                    )
                    .await?;

                    break;
                }
                _ => continue,
            }
        }

        Ok(())
    }
}
