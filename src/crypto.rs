use crate::comms;
use aes_gcm::{
    aead::{consts::U12, AeadMut},
    aes::Aes256,
    Aes256Gcm, AesGcm, KeyInit, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use std::error::Error;
use tokio::{
    io::{BufReader, BufWriter},
    net::tcp::{ReadHalf, WriteHalf},
};
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

const AES_NONCE_SIZE: usize = 12;

pub async fn edh(
    reader: &mut BufReader<ReadHalf<'_>>,
    writer: &mut BufWriter<WriteHalf<'_>>,
    buf: &mut Vec<u8>,
    go_first: bool,
) -> Result<SharedSecret, Box<dyn Error + Send + Sync>> {
    let own_sec = EphemeralSecret::new(OsRng);
    let own_pbk = PublicKey::from(&own_sec);

    if go_first {
        comms::send_bytes(writer, None, &own_pbk.as_bytes().to_vec()).await?;
        comms::recv_bytes(reader, None, buf).await?;
    } else {
        comms::recv_bytes(reader, None, buf).await?;
        comms::send_bytes(writer, None, &own_pbk.as_bytes().to_vec()).await?;
    }

    let sliced_buf: [u8; 32] = buf[..32].try_into()?;
    let recv_pbk = PublicKey::from(sliced_buf);
    buf.clear();

    Ok(own_sec.diffie_hellman(&recv_pbk))
}

pub fn aes_cipher(
    secret: SharedSecret,
) -> Result<AesGcm<Aes256, U12>, Box<dyn Error + Sync + Send>> {
    Ok(Aes256Gcm::new(secret.as_bytes().into()))
}

fn generate_nonce(rng: &mut impl RngCore) -> Nonce<U12> {
    let mut nonce = Nonce::default();
    rng.fill_bytes(&mut nonce);

    nonce
}

pub fn aes_encrypt(
    data: &Vec<u8>,
    cipher: &mut AesGcm<Aes256, U12>,
    rng: &mut OsRng,
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let nonce = generate_nonce(rng);
    let encrypted = cipher.encrypt(&nonce, data.as_ref()).unwrap(); // TODO: handle errors
    let mut data = nonce.to_vec();
    data.extend_from_slice(&encrypted);

    Ok(data)
}

pub fn aes_decrypt(
    data: &Vec<u8>,
    cipher: &mut AesGcm<Aes256, U12>,
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let (nonce_bytes, data) = data.split_at(AES_NONCE_SIZE);
    let decrypted = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), data.as_ref())
        .unwrap(); // TODO: handle errors

    Ok(decrypted)
}
