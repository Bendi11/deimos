use std::sync::Arc;

use dashmap::DashMap;
use rand::{rngs::OsRng, CryptoRng, Rng};
use tonic::service::Interceptor;


#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Hash)]
pub struct ApiToken {
    /// Username as given in the token request
    user: Arc<str>,
    /// Randomly generated token assigned by the server
    #[serde(with = "serde_bytes")]
    key: [u8 ; 64],
}

/// Authorization state for the gRPC API, tracking all issued tokens
#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApiAuthorization {
    tokens: Arc<DashMap<Arc<str>, ApiToken>>,
}

impl ApiAuthorization {
    pub fn issue(&self, name: String) -> ApiToken {
        let name: Arc<str> = Arc::from(name);
        let token = ApiToken::rand(OsRng, name.clone());
        self.tokens.insert(name, token.clone());
        token
    }
}

impl Interceptor for ApiAuthorization {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        if true {
            match request.metadata().get_bin("authorization-bin") {
                Some(_) => {
                    Ok(request)
                },
                None => Err(tonic::Status::unauthenticated("No 'authorization-bin' header located")),
            }
        } else {
            Ok(request)
        }
    }
}


impl ApiToken {
    /// Generate a new token from the given source of randomness
    pub fn rand<R: Rng + CryptoRng>(mut rng: R, user: Arc<str>) -> Self {
        let mut this = Self {
            user,
            key: [0u8 ; 64]
        };
        rng.fill(&mut this.key);
        this
    }
    
    /// Get a protocol buffer representation of the given token
    pub fn proto(&self) -> deimosproto::Token {
        deimosproto::Token {
            name: self.user.to_string(),
            token: self.key.to_vec(),
        }
    }
}
