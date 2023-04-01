use crate::crypto;
use aes_gcm::{aead::consts::U12, aes::Aes256, AesGcm};
use base64::{engine::general_purpose, Engine};
use rand::rngs::OsRng;
use std::error::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::tcp::{ReadHalf, WriteHalf},
};

pub async fn send(
    writer: &mut BufWriter<WriteHalf<'_>>,
    cipher: Option<&mut AesGcm<Aes256, U12>>,
    rng: Option<&mut OsRng>,
    data: &Vec<u8>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let enc: Vec<u8>;

    if let (Some(cipher), Some(rng)) = (cipher, rng) {
        enc = crypto::aes_encrypt(data, cipher, rng)?;
    } else {
        enc = data.clone();
    }

    let mut encoded = general_purpose::STANDARD_NO_PAD
        .encode(enc)
        .as_bytes()
        .to_vec();
    encoded.push(b':');
    writer.write_all(&encoded).await?;
    writer.flush().await?;

    Ok(())
}

pub async fn recv(
    reader: &mut BufReader<ReadHalf<'_>>,
    cipher: Option<&mut AesGcm<Aes256, U12>>,
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let mut buf = Vec::new();
    let n = reader.read_until(b':', &mut buf).await?;

    if n == 0 {
        todo!("maybe error handling :)");
    }

    buf.pop();
    buf = general_purpose::STANDARD_NO_PAD.decode(&buf)?.to_vec();

    if let Some(cipher) = cipher {
        buf = crypto::aes_decrypt(&buf, cipher)?;
    } else {
        buf = buf.clone();
    }

    Ok(buf)
}
