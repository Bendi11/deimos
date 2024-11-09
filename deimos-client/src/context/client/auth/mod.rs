use std::sync::Arc;

use chrono::{DateTime, Utc};
use deimosproto::auth::DeimosTokenKey;

#[cfg(windows)]
mod dpapi;

/// A token stored in the context save file - this may be encrypted with platform-specific APIs
/// and may need to be decrypted before use with an [AuthenticationInterceptor]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PersistentToken {
    pub kind: PersistentTokenKind,
    pub data: DeimosToken,
}


/// An unprotected token as it is sent in the API
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DeimosToken {
    pub user: Arc<str>,
    pub issued: DateTime<Utc>,
    pub key: DeimosTokenKey,
    base64: Arc<str>,
}


#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub enum PersistentTokenKind {
    Plaintext,
    #[cfg(windows)]
    Dpapi,
}

impl DeimosToken {
    pub fn base64(&self) -> &str {
        &self.base64
    }
}

impl PersistentToken {
    /// Get the username that this token was issued to
    pub fn user(&self) -> &str {
        &self.data.user
    }

    /// Get the datetime that this token was issued at
    pub fn issued_at(&self) -> DateTime<Utc> {
        self.data.issued
    }

    /// Encrypt the given token's key using platform specific APIs and return it
    pub fn protect(kind: PersistentTokenKind, data: DeimosToken) -> Result<Self, String> {
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
    pub fn unprotect(&self) -> Result<DeimosToken, String>  {
        match self.kind {
            PersistentTokenKind::Plaintext => Ok(self.data.clone().into()),
            #[cfg(windows)]
            PersistentTokenKind::Dpapi => auth::dpapi::protect(&self.data).map(Into::into).map_err(|e| e.to_string()),
        }
    }
}
