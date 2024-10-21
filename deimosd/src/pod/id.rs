use std::{borrow::Borrow, fmt::Display, ops::Deref, sync::Arc};

use serde::{Deserialize, Serialize};

/// User-assigned ID assigned to a managed container in deimos
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeimosId(Arc<str>);

/// ID of a docker container as retrieved from the Docker API
#[derive(Debug, Clone, PartialEq, Eq, Hash,)]
pub struct DockerId(Arc<str>);

impl DeimosId {
    /// Copy data from this shared ID to be used in external APIs
    pub fn owned(&self) -> String {
        String::from(&*self.0)
    }
}

impl Display for DeimosId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}'", self.0)
    }
}

impl Deref for DeimosId {
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

impl From<String> for DockerId {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl Display for DockerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0[0..self.len().max(8)])
    }
}

impl Deref for DockerId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl Borrow<str> for DockerId {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}
