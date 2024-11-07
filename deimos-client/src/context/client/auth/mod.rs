use zeroize::Zeroizing;

#[cfg(windows)]
mod dpapi;

/// A token stored in the context save file - this may be encrypted with platform-specific APIs
/// and may need to be decrypted before use with an [AuthenticationInterceptor]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PersistentToken {
    kind: PersistentTokenKind,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}


#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub enum PersistentTokenKind {
    Plaintext,
    #[cfg(windows)]
    Dpapi,
}

impl PersistentToken {
    /// Encrypt the given persistent key using platform specific APIs and return it
    pub fn protect(kind: PersistentTokenKind, data: Vec<u8>) -> Result<Self, String> {
        match kind {
            PersistentTokenKind::Plaintext => Ok(
                Self {
                    kind,
                    data,
                }
            ),
            #[cfg(windows)]
            PersistentTokenKind::Dpapi => Ok(
                Self {
                    kind,
                    data: auth::dpapi::unprotect(&data).map_err(|e| e.to_string()),      
                }
            ),
        }
    }
    
    /// Decrypt the contents of this token using platform-specific APIs specified in the [PersistentTokenKind]
    pub fn unprotect(&self) -> Result<Zeroizing<Vec<u8>>, String>  {
        match self.kind {
            PersistentTokenKind::Plaintext => Ok(self.data.clone().into()),
            #[cfg(windows)]
            PersistentTokenKind::Dpapi => auth::dpapi::protect(&self.data).map(Into::into).map_err(|e| e.to_string()),
        }
    }
}

impl Drop for PersistentToken {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.data.zeroize();
    }
}
