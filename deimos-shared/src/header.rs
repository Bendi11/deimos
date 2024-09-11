use chacha20poly1305::{Nonce, Tag};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};


/// Header prepended to all RPC messages
#[derive(Clone, Copy)]
pub struct MessageHeader {
    /// Length of the message to follow in bytes
    pub length: u32,
    /// nonce used for encryption
    pub nonce: Nonce,
    /// poly1305 authentication tag
    pub tag: Tag,
}


impl MessageHeader {
    /// Magic string used to identify the protocol over TCP
    pub const MAGIC: [u8 ; 6] = *b"DEIMOS";
    
    /// Attempt to read a message header from the given source, returning Ok(Err(e)) if the header
    /// could not be successfully decoded
    pub async fn read<R: AsyncRead + Unpin>(reader: &mut R)
        -> Result<Result<Self, InvalidMagic>, std::io::Error> {
        let mut magic_buf = [0u8 ; Self::MAGIC.len()];
        reader.read_exact(&mut magic_buf).await?;
        
        Ok(match magic_buf {
            Self::MAGIC => {
                let length = reader.read_u32().await?;
                
                let mut nonce = Nonce::default();
                reader.read_exact(nonce.as_mut_slice()).await?;

                let mut tag = Tag::default();
                reader.read_exact(tag.as_mut_slice()).await?;
                
                Ok(Self {
                    length,
                    nonce,
                    tag,
                })
            },
            invalid => Err(InvalidMagic(invalid))
        })
    }
    
    /// Write a message header including magic string to the given writer
    pub async fn write<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        writer.write_all(&Self::MAGIC).await?;
        writer.write_u32(self.length).await?;
        writer.write_all(self.tag.as_slice()).await
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid magic string {0:?}")]
pub struct InvalidMagic([u8 ; MessageHeader::MAGIC.len()]);
