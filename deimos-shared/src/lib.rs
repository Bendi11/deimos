use chacha20poly1305::{aead::OsRng, AeadCore, AeadInPlace, ChaCha20Poly1305};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub mod header;
pub mod model;
pub mod key;
pub mod util;

pub use header::MessageHeader;

/// Wait for a message to arrive from the given reader, performing authentication and decryption
/// using the provided [ChaCha20Poly1305] instance
pub async fn read_message<T: DeserializeOwned, R: AsyncRead + Unpin>(reader: &mut R, cipher: &mut ChaCha20Poly1305, max_len: u32) -> Result<T, ReadMessageError>  {
    let header = MessageHeader::read(reader).await??;
    if header.length > max_len {
        return Err(ReadMessageError::InvalidLength(header.length))
    }

    let mut buf = Vec::with_capacity(header.length.min(5_000_000) as usize);
    reader.read_exact(&mut buf).await?;
    
    cipher.decrypt_in_place_detached(&header.nonce, &[], &mut buf, &header.tag).map_err(ReadMessageError::AEAD)?;

    bincode::deserialize(&buf)
        .map_err(Into::into)
}

/// Write a message to the given writer, serializing, encrypting, and authenticating the given `T`
pub async fn write_message<T: Serialize, W: AsyncWrite + Unpin>(writer: &mut W, cipher: &mut ChaCha20Poly1305, payload: &T) -> Result<(), WriteMessageError> {
    let mut buf = Vec::with_capacity(bincode::serialized_size(payload).unwrap_or(1024) as usize);
    bincode::serialize_into(&mut buf, payload)?;
    
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let tag = cipher.encrypt_in_place_detached(&nonce, &[], &mut buf).map_err(WriteMessageError::AEAD)?;
    let header = MessageHeader {
        length: buf.len() as u32,
        nonce,
        tag,
    };

    header.write(writer).await?;
    writer.write_all(&buf).await?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ReadMessageError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Failed to deserialize packet payload: {0}")]
    Deserialize(#[from] bincode::Error),
    #[error("Invalid header magic string: {0}")]
    InvalidMagic(#[from] header::InvalidMagic),
    #[error("Invalid header length {0}")]
    InvalidLength(u32),
    #[error("chacha20poly1305 error: {0}")]
    AEAD(chacha20poly1305::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum WriteMessageError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Failed to serialize message contents: {0}")]
    Serialize(#[from] bincode::Error),
    #[error("chacha20poly1305 error: {0}")]
    AEAD(chacha20poly1305::Error),
}
