use super::crypto;
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

const PUBLIC_IPV4: &str = "https://ipinfo.io/ip";
const PUBLIC_IPV6: &str = "https://ipv6.icanhazip.com";

#[derive(Debug, PartialEq, Eq)]
pub enum Message {
    ErrorMsg(String),
    Files(Vec<PathBuf>),
    Metadata(HashMap<String, (u64, String)>),
    ClientConnect(SocketAddr),
    ClientDisconnect(SocketAddr),
    ClientReq(String),
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

#[derive(PartialEq, Eq)]
pub enum Ip {
    V4,
    V6,
}

impl Ip {
    pub fn fetch(self) -> Result<SocketAddr, Box<dyn Error>> {
        let addr = match self {
            Ip::V4 => PUBLIC_IPV4,
            Ip::V6 => PUBLIC_IPV6,
        };

        let res = ureq::get(addr).call()?.into_string()?;
        let addr: SocketAddr = res.trim().parse()?;

        Ok(addr)
    }
}
