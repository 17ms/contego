use crate::{
    common::{Connection, Message},
    comms, crypto,
};
use std::{collections::HashMap, error::Error, net::SocketAddr, path::PathBuf, str::FromStr};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

#[derive(Debug, Clone, Copy)]
pub struct Listener {
    host_addr: SocketAddr,
    access_key: &'static str,
    chunksize: usize,
}

// TODO: impl Drop (?)

impl Listener {
    pub fn new(host_addr: SocketAddr, access_key: &'static str, chunksize: usize) -> Self {
        Self {
            host_addr,
            access_key,
            chunksize,
        }
    }

    pub async fn start(
        self,
        tx: mpsc::Sender<Message>,
        mut kill: mpsc::Receiver<Message>,
        files: Vec<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        tokio::select! {
            _ = self.listen(tx, files) => Ok(()),
            _ = kill.recv() => Ok(()),
        }
    }

    async fn listen(
        self,
        tx: mpsc::Sender<Message>,
        files: Vec<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind(self.host_addr).await?;

        loop {
            let files = files.clone();
            let (mut socket, addr) = listener.accept().await?;
            tx.send(Message::ClientConnect(addr)).await?;
            let this_tx = tx.clone();

            tokio::spawn(async move {
                self.connection(&mut socket, addr, this_tx, &files).await?;
                Ok::<(), Box<dyn Error + Send + Sync>>(())
            });
        }
    }

    async fn connection(
        &self,
        socket: &mut TcpStream,
        addr: SocketAddr,
        tx: mpsc::Sender<Message>,
        files: &Vec<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut connection = Connection::new(socket).await?;

        if !self.authorize(&mut connection).await? {
            return Ok::<(), Box<dyn Error + Send + Sync>>(());
        }

        let index = self.metadata_handler(&mut connection, files).await?;
        tx.send(Message::ConnectionReady).await?;
        self.request_handler(&mut connection, &index).await?;
        tx.send(Message::ClientDisconnect(addr)).await?;

        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }

    async fn authorize(
        &self,
        conn: &mut Connection<'_>,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let buf = comms::recv(&mut conn.reader, Some(&mut conn.cipher)).await?;
        let key = String::from_utf8(buf)?;
        let msg: Vec<u8>;
        let res: bool;

        if key != self.access_key {
            res = false;
            msg = b"DISCONNECT".to_vec();
        } else {
            res = true;
            msg = b"OK".to_vec();
        }

        comms::send(
            &mut conn.writer,
            Some(&mut conn.cipher),
            Some(&mut conn.rng),
            &msg,
        )
        .await?;

        Ok(res)
    }

    async fn metadata(
        &self,
        files: &Vec<String>,
    ) -> Result<
        (usize, Vec<(String, u64, String)>, HashMap<String, String>),
        Box<dyn Error + Send + Sync>,
    > {
        let mut metadata: Vec<(String, u64, String)> = Vec::new();
        let mut index = HashMap::new();

        for path in files {
            let split: Vec<&str> = path.split('/').collect(); // TODO: different path delimiters?
            let name = split[split.len() - 1].to_string();
            let handle = File::open(PathBuf::from_str(path)?).await?;
            let size = handle.metadata().await?.len();
            let hash = crypto::try_hash(path)?;

            if size > 0 {
                metadata.push((name, size, hash.clone()));
                index.insert(hash, path.clone());
            }
        }

        Ok((metadata.len(), metadata, index))
    }

    async fn metadata_handler(
        &self,
        conn: &mut Connection<'_>,
        files: &Vec<String>,
    ) -> Result<HashMap<String, String>, Box<dyn Error + Send + Sync>> {
        let (amt, metadata, index) = self.metadata(files).await?;
        let msg = amt.to_string().as_bytes().to_vec();

        comms::send(
            &mut conn.writer,
            Some(&mut conn.cipher),
            Some(&mut conn.rng),
            &msg,
        )
        .await?;

        let buf = comms::recv(&mut conn.reader, Some(&mut conn.cipher)).await?;
        let msg = String::from_utf8(buf)?;

        if msg != "AMT" {
            todo!("maybe error handling :)");
        }

        for file in metadata {
            let msg = format!("{}:{}:{}", file.0, file.1, file.2)
                .as_bytes()
                .to_vec();

            comms::send(
                &mut conn.writer,
                Some(&mut conn.cipher),
                Some(&mut conn.rng),
                &msg,
            )
            .await?;
        }

        Ok(index)
    }

    async fn request_handler(
        &self,
        conn: &mut Connection<'_>,
        index: &HashMap<String, String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        loop {
            let buf = comms::recv(&mut conn.reader, Some(&mut conn.cipher)).await?;
            let cmd = String::from_utf8(buf)?;

            if cmd == "DISCONNECT" {
                break;
            }

            let mut file = File::open(index[&cmd].clone()).await?;
            let mut remaining = file.metadata().await?.len();
            let mut send_buf = vec![0u8; self.chunksize];

            while remaining != 0 {
                let n = file.read(&mut send_buf).await?;

                comms::send(
                    &mut conn.writer,
                    Some(&mut conn.cipher),
                    Some(&mut conn.rng),
                    &send_buf[..n].to_vec(),
                )
                .await?;

                remaining -= n as u64;
            }

            let buf = comms::recv(&mut conn.reader, Some(&mut conn.cipher)).await?;
            let msg = String::from_utf8(buf)?;

            if msg == "ERROR" {
                todo!("maybe error handling :)");
            }
        }

        Ok(())
    }
}
