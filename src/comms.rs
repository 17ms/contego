use super::crypto;
use aes_gcm::{aead::consts::U12, aes::Aes256, AesGcm};
use rand::rngs::OsRng;
use std::error::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::tcp::{ReadHalf, WriteHalf},
};

pub async fn send_bytes(
    writer: &mut BufWriter<WriteHalf<'_>>,
    enc: Option<(&mut AesGcm<Aes256, U12>, &mut OsRng)>,
    data: &Vec<u8>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let processed = enc.map_or(Ok(data.clone()), |enc| {
        crypto::aes_encrypt(data, enc.0, enc.1)
    })?;
    writer.write_all(&processed).await?;
    writer.flush().await?;

    Ok(())
}

pub async fn recv_bytes(
    reader: &mut BufReader<ReadHalf<'_>>,
    cipher: Option<&mut AesGcm<Aes256, U12>>,
    buf: &mut Vec<u8>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let recv_bytes = reader.read_until(b'\n', buf).await?;
    *buf = cipher.map_or(Ok(buf.clone()), |c| crypto::aes_decrypt(&buf, c))?;
    if recv_bytes == 0 {
        todo!("ERROR: No message received or client <xyz> crashed");
    }

    Ok(())
}
