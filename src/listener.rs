use crate::{
    common::{Connection, Message},
    comms, crypto,
};
use std::{collections::HashMap, error::Error, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

#[derive(Debug, Clone)]
pub struct Listener {
    host_addr: SocketAddr,
    access_key: String,
    chunksize: usize,
}

// TODO: impl Drop (?)

impl Listener {
    pub fn new(
        host_addr: SocketAddr,
        access_key: String,
        chunksize: usize,
    ) -> Result<Arc<Self>, Box<dyn Error>> {
        Ok(Arc::new(Self {
            host_addr,
            access_key,
            chunksize,
        }))
    }

    pub async fn start(
        self: Arc<Self>,
        tx: mpsc::Sender<Message>,
        mut kill: mpsc::Receiver<Message>,
        files: Vec<PathBuf>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        tokio::select! {
            _ = self.listen(tx, files) => Ok(()),
            _ = kill.recv() => Ok(()),
        }
    }

    async fn listen(
        self: Arc<Self>,
        tx: mpsc::Sender<Message>,
        files: Vec<PathBuf>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind(self.host_addr).await?;

        loop {
            let files = files.clone();
            let (mut socket, addr) = listener.accept().await?;
            tx.send(Message::ClientConnect(addr)).await?;
            let conn_tx = tx.clone();
            let err_tx = tx.clone();
            let conn_self = Arc::clone(&self);

            match tokio::spawn(async move {
                conn_self
                    .connection(&mut socket, addr, conn_tx, &files)
                    .await
            })
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    let err_msg = format!("{}: {}", addr, e);
                    err_tx.send(Message::Error(err_msg)).await?;
                }
            };
        }
    }

    async fn connection(
        &self,
        socket: &mut TcpStream,
        addr: SocketAddr,
        tx: mpsc::Sender<Message>,
        files: &Vec<PathBuf>,
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
        files: &Vec<PathBuf>,
    ) -> Result<
        (usize, Vec<(String, u64, String)>, HashMap<String, PathBuf>),
        Box<dyn Error + Send + Sync>,
    > {
        let mut metadata: Vec<(String, u64, String)> = Vec::new();
        let mut index = HashMap::new();

        for path in files {
            let split: Vec<&str> = path.to_str().unwrap().split('/').collect();
            let name = split[split.len() - 1].to_string();
            let handle = File::open(path).await?;
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
        files: &Vec<PathBuf>,
    ) -> Result<HashMap<String, PathBuf>, Box<dyn Error + Send + Sync>> {
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
            return Err("Broken message sequence".into());
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
        index: &HashMap<String, PathBuf>,
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
                return Err("Incomplete file request (hashes don't match)".into());
            }
        }

        Ok(())
    }
}
