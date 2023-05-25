use std::error::Error;

use base64::{engine::general_purpose, Engine};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpStream,
    },
};

use crate::crypto::Crypto;

pub struct SocketHandler<'a> {
    writer: BufWriter<WriteHalf<'a>>,
    reader: BufReader<ReadHalf<'a>>,
    crypto: Option<Crypto>,
}

impl<'a> SocketHandler<'a> {
    pub fn new(socket: &'a mut TcpStream) -> Self {
        let (reader, writer) = socket.split();
        let mut reader = BufReader::new(reader);
        let mut writer = BufWriter::new(writer);

        Self {
            writer,
            reader,
            crypto: None,
        }
    }

    pub fn set_crypto(&self, crypto: Crypto) {
        // setting up AES cipher requires DH key exchange in plaintext,
        // meaning crypto can't be initialized at the same time as the socket handler
        self.crypto = Some(crypto);
    }

    pub async fn send(&mut self, data: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = match &self.crypto {
            Some(c) => c.encrypt(data).await?,
            None => data.to_vec(),
        };

        self.send_raw(&data).await?;

        Ok(())
    }

    pub async fn send_raw(&mut self, data: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.writer.write_all(data).await?;
        self.writer.flush().await?;

        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let mut buf = self.recv_raw().await?;
        buf.pop();
        buf = general_purpose::STANDARD_NO_PAD.decode(&buf)?.to_vec();

        let data = match &self.crypto {
            Some(c) => c.decrypt(&buf).await?,
            None => buf,
        };

        Ok(data)
    }

    pub async fn recv_raw(&mut self) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let mut buf = Vec::new();
        let n = self.reader.read_until(b':', &mut buf).await?;

        if n == 0 {
            return Err("Received 0 bytes from the socket".into());
        }

        Ok(buf)
    }
}
