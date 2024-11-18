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
    pub user: Arc<str>,
    pub issued: DateTime<Utc>,
    pub key: DeimosTokenKey,
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
#[derive(Clone,)]
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
    pub fn new(user: Arc<str>, issued: DateTime<Utc>, key: DeimosTokenKey) -> Self {
        Self {
            user,
            issued,
            base64: key.to_base64().into(),
            key,
        }
    }

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
}


#[derive(Debug, thiserror::Error)]
pub enum DeimosTokenConvertError {
    #[error("Out of range UNIX timestamp")]
    DateTime,
}

impl PersistentToken {
    /// Encrypt the given token's key using platform specific APIs and return it
    pub fn protect(kind: PersistentTokenKind, data: DeimosToken) -> Result<Self, String> {
        Ok(Self {
            kind,
            issued: data.issued,
            user: data.user,
            key: match kind {
                PersistentTokenKind::Plaintext => data.key,
                #[cfg(windows)]
                PersistentTokenKind::Dpapi => dpapi::protect(data.key.as_bytes()).map(DeimosTokenKey::from_bytes).map_err(|e| e.to_string())?,
            },
        })
    }
    
    /// Decrypt the contents of this token using platform-specific APIs specified in the [PersistentTokenKind]
    pub fn unprotect(&self) -> Result<DeimosToken, String>  {
        Ok(DeimosToken::new(
            self.user.clone(),
            self.issued,
            match self.kind {
                PersistentTokenKind::Plaintext => self.key.clone(),
                #[cfg(windows)]
                PersistentTokenKind::Dpapi => dpapi::unprotect(self.key.as_bytes()).map(DeimosTokenKey::from_bytes).map_err(|e| e.to_string())?,
            }
        ))
    }
}

impl Drop for TokenStatus {
    fn drop(&mut self) {
        if let Self::Requested { user: _, cancel } = self {
            cancel.notify_waiters();
        }
    }
}

impl std::fmt::Debug for DeimosToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f
            .debug_struct("PersistentToken")
            .field("user", &self.user)
            .field("issued", &self.issued)
            .field("token", &self.key)
            .finish_non_exhaustive()
    }
}

impl Default for PersistentTokenKind {
    fn default() -> Self {
        #[cfg(windows)]
        { Self::Dpapi }

        #[cfg(not(windows))]
        { Self::Plaintext }
    }
}
