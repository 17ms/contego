use std::{collections::HashMap, error::Error, net::SocketAddr, path::PathBuf, sync::Arc};

use tokio::{
    fs::File,
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

use crate::{crypto::Crypto, sockets::SocketHandler, util::FileInfo};

#[derive(Clone)]
pub struct Server {
    addr: SocketAddr,
    key: String,
    chunksize: usize,
    metadata: Vec<FileInfo>,
    index: HashMap<String, PathBuf>,
}

impl Server {
    pub fn new(
        addr: SocketAddr,
        key: String,
        chunksize: usize,
        metadata: Vec<FileInfo>,
        index: HashMap<String, PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self {
            addr,
            key,
            chunksize,
            metadata,
            index,
        })
    }

    pub async fn start(
        self: Arc<Self>,
        mut kill: mpsc::Receiver<()>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        tokio::select! {
            _ = self.listen() => Ok(()),
            _ = kill.recv() => Ok(()),
        }
    }

    async fn listen(self: Arc<Self>) -> Result<(), Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind(self.addr).await?;

        loop {
            let this_self = self.clone();
            let (mut socket, addr) = listener.accept().await?;

            // log: new client connected: <addr>

            match tokio::spawn(async move { this_self.connection(&mut socket).await }).await {
                Ok(_) => {}
                Err(e) => eprintln!("Error during connection ({}): {}", addr, e),
            };
        }
    }

    async fn connection(&self, socket: &mut TcpStream) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut handler = SocketHandler::new(socket);
        let crypto = Crypto::new(&mut handler, true).await?;
        handler.set_crypto(crypto);

        if !self.authorize(&mut handler).await? {
            return Ok(());
        }

        self.metadata(&mut handler).await?;
        self.requests(&mut handler).await?;

        Ok(())
    }

    async fn authorize(
        &self,
        handler: &mut SocketHandler<'_>,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let buf = handler.recv().await?;
        let key = String::from_utf8(buf)?;

        let is_valid: bool;
        let res_msg: Vec<u8>;

        if key != self.key {
            is_valid = false;
            res_msg = b"DISCONNECT".to_vec();
        } else {
            is_valid = true;
            res_msg = b"VALID".to_vec();
        }

        handler.send(&res_msg).await?;

        Ok(is_valid)
    }

    async fn metadata(
        &self,
        handler: &mut SocketHandler<'_>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let amt = self.metadata.len();
        let msg = amt.to_string().as_bytes().to_vec();

        handler.send(&msg).await?;

        let buf = handler.recv().await?;
        let res_amt = String::from_utf8(buf)?.trim().parse::<usize>()?;

        if res_amt != amt {
            return Err("Broken message sequence during metadata exchange".into());
        }

        for file in &self.metadata {
            let msg = format!("{}:{}:{}", file.name, file.size, file.hash)
                .as_bytes()
                .to_vec();
            handler.send(&msg).await?;
        }

        Ok(())
    }

    async fn requests(
        &self,
        handler: &mut SocketHandler<'_>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        loop {
            let buf = handler.recv().await?;
            let hash = String::from_utf8(buf)?;
            let hash = hash.trim();

            if hash == "DISCONNECT" {
                break;
            }

            let mut file = File::open(self.index[hash].clone()).await?;
            let mut remaining = file.metadata().await?.len();
            let mut sendbuf = vec![0u8; self.chunksize];

            while remaining != 0 {
                let n = file.read(&mut sendbuf).await?;
                handler.send(&sendbuf[..n].to_vec()).await?;
                remaining -= n as u64;
            }

            let buf = handler.recv().await?;
            let confirmation = String::from_utf8(buf)?;
            let confirmation = confirmation.trim();

            if confirmation != hash {
                return Err("Unsuccessful file transfer, hashes don't match".into());
            }
        }

        Ok(())
    }
}
