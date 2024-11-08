use std::sync::Arc;

use base64::Engine;

/// A deimos token that serializes to base64
#[derive(Clone)]
pub struct DeimosTokenKey(Arc<[u8]>);

impl DeimosTokenKey {
    /// Create a new token from the given bytes
    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self(Arc::from(data))
    }
    
    /// Attempt to decode a token key from base64 as it is encoded in API calls
    pub fn from_base64(string: &str) -> Result<Self, base64::DecodeError> {
        let engine = Self::engine();
        engine.decode(string).map(Self::from_bytes)
    }
    
    /// Encode the token key in base64 for use in HTTP headers
    pub fn to_base64(&self) -> String {
        let engine = Self::engine();
        engine.encode(&self.0)
    }
    
    /// Get a new base64 engine to be used to serialize and deserialize the token
    const fn engine() -> base64::engine::GeneralPurpose {
        base64::engine::GeneralPurpose::new(&base64::alphabet::URL_SAFE, base64::engine::GeneralPurposeConfig::new())
    }
}

impl serde::Serialize for DeimosTokenKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let engine = Self::engine();
        serializer.serialize_str(&engine.encode(&self.0))
    }
}

impl<'de> serde::Deserialize<'de> for DeimosTokenKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
        struct Base64Visitor;

        impl<'d> serde::de::Visitor<'d> for Base64Visitor {
            type Value = DeimosTokenKey;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                std::write!(formatter, "URL safe base64 value")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: serde::de::Error, {
                DeimosTokenKey::from_base64(v)
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(v), &self))
            }
        }

        deserializer.deserialize_str(Base64Visitor)
    }
}

impl std::fmt::Debug for DeimosTokenKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LEAKED DEIMOS AUTH TOKEN")
    }
}

impl Drop for DeimosTokenKey {
    fn drop(&mut self) {
        if let Some(buf) = Arc::get_mut(&mut self.0) {
            zeroize::Zeroize::zeroize(buf);
        }
    }
}

