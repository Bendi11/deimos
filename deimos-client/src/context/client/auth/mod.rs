use std::sync::Arc;

use chrono::{DateTime, Utc};
use deimosproto::auth::DeimosTokenKey;
use tokio::sync::Notify;

#[cfg(windows)]
mod dpapi;

/// A token stored in the context save file - this may be encrypted with platform-specific APIs
/// and may need to be decrypted before use with an [AuthenticationInterceptor]
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PersistentToken {
    pub kind: PersistentTokenKind,
    pub data: DeimosToken,
}


#[derive(Debug, Clone)]
pub enum TokenStatus {
    None,
    Requested {
        user: String,
        cancel: Arc<Notify>,
    },
    Denied {
        reason: String,
    },
    Token(DeimosToken),
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

impl TokenStatus {
    /// Get the token if one is stored
    pub const fn token(&self) -> Option<&DeimosToken> {
        match self {
            Self::Token(ref tok) => Some(tok),
            _ => None,
        }
    }
    
    /// Convert an optional token into a [TokenStatus]
    pub fn from_token(tok: Option<DeimosToken>) -> Self {
        match tok {
            Some(tok) => Self::Token(tok),
            None => Self::None,
        }
    }
}

impl DeimosToken {
    /// Get a chached base64 string representing the token
    pub fn base64(&self) -> &str {
        &self.base64
    }
    
    /// Decode a protobuf containing a token
    pub fn from_proto(proto: deimosproto::Token) -> Result<Self, DeimosTokenConvertError>  {
        let user = proto.name.into();
        let issued = DateTime::<Utc>::from_timestamp(proto.issued, 0).ok_or(DeimosTokenConvertError::DateTime)?;
        let key = DeimosTokenKey::from_bytes(proto.key);
        let base64 = key.to_base64().into();

        Ok(Self {
            user,
            issued,
            key,
            base64,
        })
    }
    
    /// Apply a fallible function to the contained key and return the new token if the map succeeds
    pub fn try_map<E, F: FnOnce(&[u8]) -> Result<Vec<u8>, E>>(&self, f: F) -> Result<Self, E> {
        f(self.key.as_bytes())
            .map(DeimosTokenKey::from_bytes)
            .map(|key| 
                Self {
                    user: self.user.clone(),
                    issued: self.issued,
                    base64: key.to_base64().into(),
                    key,
                }
            )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeimosTokenConvertError {
    #[error("Out of range UNIX timestamp")]
    DateTime,
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
        Ok(Self {
            data: match kind {
                PersistentTokenKind::Plaintext => data,
                #[cfg(windows)]
                PersistentTokenKind::Dpapi => data.try_map(|data| auth::dpapi::protect(data).map_err(|e| e.to_string()))?,
            },
            kind,
        })
    }
    
    /// Decrypt the contents of this token using platform-specific APIs specified in the [PersistentTokenKind]
    pub fn unprotect(&self) -> Result<DeimosToken, String>  {
        match self.kind {
            PersistentTokenKind::Plaintext => Ok(self.data.clone()),
            #[cfg(windows)]
            PersistentTokenKind::Dpapi => self.data.try_map(|data| auth::dpapi::unprotect(data).map_err(|e| e.to_string())),
        }
    }
}

impl Drop for TokenStatus {
    fn drop(&mut self) {
        if let Self::Requested { user: _, cancel } = self {
            cancel.notify_waiters();
        }
    }
}
