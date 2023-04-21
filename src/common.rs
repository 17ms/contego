use crate::crypto;
use aes_gcm::{aead::consts::U12, aes::Aes256, AesGcm};
use rand::rngs::OsRng;
use std::{collections::HashMap, error::Error, net::SocketAddr, path::PathBuf};
use tokio::{
    io::{BufReader, BufWriter},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpStream,
    },
};

#[derive(Debug, PartialEq, Eq)]
pub enum Message {
    ErrorMsg(String),
    Files(Vec<PathBuf>),
    Metadata(HashMap<String, (u64, String)>),
    Chunksize(usize),
    ClientConnect(SocketAddr),
    ClientDisconnect(SocketAddr),
    ClientReq(String),
    ClientReqAll,
    ConnectionReady,
    Shutdown,
}

pub struct Connection<'a> {
    pub reader: BufReader<ReadHalf<'a>>,
    pub writer: BufWriter<WriteHalf<'a>>,
    pub cipher: AesGcm<Aes256, U12>,
    pub rng: OsRng,
}

impl<'a> Connection<'a> {
    pub async fn new(
        socket: &'a mut TcpStream,
    ) -> Result<Connection<'a>, Box<dyn Error + Send + Sync>> {
        let (reader, writer) = socket.split();
        let mut reader = BufReader::new(reader);
        let mut writer = BufWriter::new(writer);
        let cipher = crypto::aes_cipher(&mut reader, &mut writer, true).await?;
        let rng = OsRng;

        Ok(Self {
            reader,
            writer,
            cipher,
            rng,
        })
    }
}
