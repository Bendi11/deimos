use std::{fs::Permissions, os::unix::fs::PermissionsExt, path::Path};

use chacha20poly1305::{ChaCha20Poly1305, Key, KeySizeUser};
use pem::Pem;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::util;

pub const DEIMOS_PEM_TAG: &str = "DEIMOS SYMMETRIC KEY";

/// Load a symmetric key from a PEM file located at the given path, checking it for the appropriate permissions
/// and ensuring the key's length matches the expected length of a chacha20poly1305 key
pub async fn load_symmetric_pem(path: impl AsRef<Path>) -> Result<Key, DeimosKeyError> {
    let pem_str = util::load_check_permissions(&path).await?;
    let pem = pem::parse(pem_str)?;
    if pem.tag() != DEIMOS_PEM_TAG {
        tracing::warn!("pem key file {} has unrecognized tag {}", path.as_ref().display(), pem.tag());
    }

    let key_bytes = pem.contents();
    if key_bytes.len() != ChaCha20Poly1305::key_size() {
        return Err(DeimosKeyError::InvalidKeyLength(key_bytes.len()))
    }

    Ok(Key::clone_from_slice(key_bytes))
}

/// Write a symmetric key to the given path
pub async fn save_symmetric_pem(path: impl AsRef<Path>, key: Key) -> Result<(), DeimosKeyError> {
    let key_pem = Pem::new(DEIMOS_PEM_TAG, key.as_slice());
    
    let mut file = File::create(path).await?;
    #[cfg(unix)]
    file.set_permissions(Permissions::from_mode(0o600)).await?;

    file.write_all(pem::encode(&key_pem).as_bytes()).await?;
    Ok(())
}


#[derive(Debug, thiserror::Error)]
pub enum DeimosKeyError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("PEM parsing error: {0}")]
    Pem(#[from] pem::PemError),
    #[error("PEM file contains key with invalid length {0}B")]
    InvalidKeyLength(usize),
}
