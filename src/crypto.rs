use std::{error::Error, path::Path};

use aes_gcm::{
    aead::{consts::U12, Aead},
    aes::Aes256,
    Aes256Gcm, AesGcm, KeyInit, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

use crate::sockets::SocketHandler;

const AES_NONCE_SIZE: usize = 12;
const DH_PBK_SIZE: usize = 32;

#[derive(Clone)]
pub struct Crypto {
    cipher: AesGcm<Aes256, U12>,
    rng: OsRng,
}

impl Crypto {
    pub async fn new(
        handler: &mut SocketHandler<'_>,
        go_first: bool,
    ) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let secret = Self::ecdh(handler, go_first).await?;
        let cipher = Aes256Gcm::new(secret.as_bytes().into());
        let rng = OsRng;

        Ok(Self { cipher, rng })
    }

    async fn ecdh(
        handler: &mut SocketHandler<'_>,
        go_first: bool,
    ) -> Result<SharedSecret, Box<dyn Error + Send + Sync>> {
        let buf: Vec<u8>;
        let own_sec = EphemeralSecret::new(OsRng);
        let own_pbk = PublicKey::from(&own_sec);
        let msg = own_pbk.as_bytes().to_vec();

        if go_first {
            handler.send(&msg).await?;
            buf = handler.recv().await?;
        } else {
            buf = handler.recv().await?;
            handler.send(&msg).await?;
        }

        let slice: [u8; DH_PBK_SIZE] = buf[..DH_PBK_SIZE].try_into()?;
        let recv_pbk = PublicKey::from(slice);

        Ok(own_sec.diffie_hellman(&recv_pbk))
    }

    fn nonce(&self) -> Nonce<U12> {
        let mut nonce = Nonce::default();
        self.rng.fill_bytes(&mut nonce);

        nonce
    }

    pub async fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let nonce = self.nonce();
        let encrypted = match self.cipher.encrypt(&nonce, data.as_ref()) {
            Ok(data) => data,
            Err(e) => return Err(format!("Encryption failed: {}", e).into()),
        };

        let mut data = nonce.to_vec();
        data.extend_from_slice(&encrypted);

        Ok(data)
    }

    pub async fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let (nonce_bytes, data) = data.split_at(AES_NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);
        let decrypted = match self.cipher.decrypt(nonce, data.as_ref()) {
            Ok(data) => data,
            Err(e) => return Err(format!("Decryption failed: {}", e).into()),
        };

        Ok(decrypted)
    }
}

pub fn try_hash(path: &Path) -> Result<String, Box<dyn Error + Send + Sync>> {
    let hash = sha256::try_digest(path)?;

    Ok(hash)
}

// TODO: unit test if deemed necessary
