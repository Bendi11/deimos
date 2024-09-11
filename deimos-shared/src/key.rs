use std::path::Path;

use chacha20poly1305::Key;



pub async fn load_symmetric_pem(path: impl AsRef<Path>) -> Result<Key, 


#[derive(Debug, thiserror::Error)]
pub enum DeimosKeyError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("PEM parsing error: {0}")]
    Pem(#[from] )
}
