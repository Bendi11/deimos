use std::path::Path;

use tokio::{fs::File, io::AsyncReadExt};

/// Open the file at the given path and ensure that its permissions are as good as 700 at least,
/// then read it to a string
pub async fn load_check_permissions(path: impl AsRef<Path>) -> Result<Vec<u8>, std::io::Error> {
    let mut file = File::open(&path).await?;
    let meta = file.metadata().await?;

    #[cfg(unix)]
    {
        let permissions = meta.permissions();
        use std::os::unix::fs::PermissionsExt;
        let mode = permissions.mode();
        if mode & 0o077 != 0 {
            tracing::error!("Sensitive file {} has group and/or other read/write permissions - change to 600 or 400", path.as_ref().display());
            return Err(tokio::io::ErrorKind::InvalidInput.into());
        }
    }

    let mut buf = Vec::with_capacity(meta.len() as usize);
    file.read_to_end(&mut buf).await?;

    Ok(buf)
}
