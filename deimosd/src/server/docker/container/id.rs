use std::{borrow::Borrow, fmt::Display, ops::Deref, sync::Arc};

use serde::{Deserialize, Serialize};

/// Docker-assigned ID given to a Docker container
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DockerId(Arc<str>);

/// User-assigned ID assigned to a managed container in deimos
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeimosId(Arc<str>);

impl DockerId {
    /// Copy data from this shared ID to be used in external APIs
    pub fn owned(&self) -> String {
        String::from(&*self.0)
    }
}

impl DeimosId {
    /// Copy data from this shared ID to be used in external APIs
    pub fn owned(&self) -> String {
        String::from(&*self.0)
    }
}

impl Display for DockerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let length = self.0.len().max(8);
        write!(f, "{}", &self.0[0..length])
    }
}

impl Display for DeimosId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}'", self.0)
    }
}

impl From<String> for DockerId {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl Deref for DockerId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl Borrow<str> for DeimosId {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}
