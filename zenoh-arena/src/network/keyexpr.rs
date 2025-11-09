//! Key expression types for host discovery and connection

use crate::error::ArenaError;
use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Host keyexpr - represents a specific host in the arena
///
/// Pattern: `<prefix>/host/<host_id>`
///
/// Used for:
/// - Declaring queryables to respond to connection requests
/// - Connecting to a specific host
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyexpr {
    prefix: String,
    host_id: NodeId,
}

impl HostKeyexpr {
    /// Create a new HostKeyexpr
    pub fn new(prefix: &KeyExpr, host_id: NodeId) -> Self {
        Self {
            prefix: prefix.to_string(),
            host_id,
        }
    }

    /// Get the host ID
    pub fn host_id(&self) -> &NodeId {
        &self.host_id
    }

    /// Get the prefix
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

impl TryFrom<KeyExpr<'_>> for HostKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();
        
        // Expected pattern: [...prefix]/host/<host_id>
        if parts.len() < 2 || parts[parts.len() - 2] != "host" {
            return Err(ArenaError::InvalidKeyexpr(
                format!("Invalid HostKeyexpr pattern: {}", keyexpr.as_str()),
            ));
        }

        let host_id_str = parts[parts.len() - 1];
        let host_id = NodeId::from_name(host_id_str.to_string())?;
        let prefix = parts[..parts.len() - 2].join("/");

        Ok(Self { prefix, host_id })
    }
}

impl From<HostKeyexpr> for KeyExpr<'static> {
    fn from(host_keyexpr: HostKeyexpr) -> Self {
        let keyexpr_str = format!("{}/host/{}", host_keyexpr.prefix, host_keyexpr.host_id.as_str());
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

/// Host lookup keyexpr - used for discovering all available hosts
///
/// Pattern: `<prefix>/host/*`
///
/// Used to query all available hosts in the arena.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostLookupKeyexpr {
    prefix: String,
}

impl HostLookupKeyexpr {
    /// Create a new HostLookupKeyexpr
    pub fn new(prefix: &KeyExpr) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

impl TryFrom<KeyExpr<'_>> for HostLookupKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();
        
        // Expected pattern: [...prefix]/host/*
        if parts.len() < 2 || parts[parts.len() - 2] != "host" || parts[parts.len() - 1] != "*" {
            return Err(ArenaError::InvalidKeyexpr(
                format!("Invalid HostLookupKeyexpr pattern: {}", keyexpr.as_str()),
            ));
        }

        let prefix = parts[..parts.len() - 2].join("/");
        Ok(Self { prefix })
    }
}

impl From<HostLookupKeyexpr> for KeyExpr<'static> {
    fn from(lookup_keyexpr: HostLookupKeyexpr) -> Self {
        let keyexpr_str = format!("{}/host/*", lookup_keyexpr.prefix);
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

/// Host client keyexpr - used for initiating client connections
///
/// Pattern: `<prefix>/host/<host_id>/<client_id>`
///
/// Used when a client initiates a connection request to a specific host.
/// Contains both the target host ID and the requesting client ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClientKeyexpr {
    prefix: String,
    host_id: NodeId,
    client_id: NodeId,
}

impl HostClientKeyexpr {
    /// Create a new HostClientKeyexpr
    pub fn new(prefix: &KeyExpr, host_id: NodeId, client_id: NodeId) -> Self {
        Self { 
            prefix: prefix.to_string(), 
            host_id,
            client_id,
        }
    }

    /// Get the host ID
    pub fn host_id(&self) -> &NodeId {
        &self.host_id
    }

    /// Get the client ID
    pub fn client_id(&self) -> &NodeId {
        &self.client_id
    }

    /// Get the prefix
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

impl TryFrom<KeyExpr<'_>> for HostClientKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();
        
        // Expected pattern: [...prefix]/host/<host_id>/<client_id>
        if parts.len() < 3 || parts[parts.len() - 3] != "host" {
            return Err(ArenaError::InvalidKeyexpr(
                format!("Invalid HostClientKeyexpr pattern: {}", keyexpr.as_str()),
            ));
        }

        let host_id_str = parts[parts.len() - 2];
        let client_id_str = parts[parts.len() - 1];
        
        let host_id = NodeId::from_name(host_id_str.to_string())?;
        let client_id = NodeId::from_name(client_id_str.to_string())?;
        let prefix = parts[..parts.len() - 3].join("/");

        Ok(Self { prefix, host_id, client_id })
    }
}

impl From<HostClientKeyexpr> for KeyExpr<'static> {
    fn from(client_keyexpr: HostClientKeyexpr) -> Self {
        let keyexpr_str = format!("{}/host/{}/{}", client_keyexpr.prefix, client_keyexpr.host_id.as_str(), client_keyexpr.client_id.as_str());
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

/// Host client lookup keyexpr - used for discovering all clients connected to a specific host
///
/// Pattern: `<prefix>/host/<host_id>/*`
///
/// Used to query all clients that are connected to or communicating with a specific host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClientLookupKeyexpr {
    prefix: String,
    host_id: NodeId,
}

impl HostClientLookupKeyexpr {
    /// Create a new HostClientLookupKeyexpr
    pub fn new(prefix: &KeyExpr, host_id: NodeId) -> Self {
        Self {
            prefix: prefix.to_string(),
            host_id,
        }
    }

    /// Get the host ID
    pub fn host_id(&self) -> &NodeId {
        &self.host_id
    }

    /// Get the prefix
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

impl TryFrom<KeyExpr<'_>> for HostClientLookupKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();
        
        // Expected pattern: [...prefix]/host/<host_id>/*
        if parts.len() < 3 || parts[parts.len() - 3] != "host" || parts[parts.len() - 1] != "*" {
            return Err(ArenaError::InvalidKeyexpr(
                format!("Invalid HostClientLookupKeyexpr pattern: {}", keyexpr.as_str()),
            ));
        }

        let host_id_str = parts[parts.len() - 2];
        let host_id = NodeId::from_name(host_id_str.to_string())?;
        let prefix = parts[..parts.len() - 3].join("/");

        Ok(Self { prefix, host_id })
    }
}

impl From<HostClientLookupKeyexpr> for KeyExpr<'static> {
    fn from(lookup_keyexpr: HostClientLookupKeyexpr) -> Self {
        let keyexpr_str = format!("{}/host/{}/*", lookup_keyexpr.prefix, lookup_keyexpr.host_id.as_str());
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        
        let host_keyexpr = HostKeyexpr::new(&prefix, host_id.clone());
        assert_eq!(host_keyexpr.host_id(), &host_id);
        assert_eq!(host_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        
        let host_keyexpr = HostKeyexpr::new(&prefix, host_id.clone());
        let keyexpr: KeyExpr = host_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/host1");
        
        let parsed = HostKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &host_id);
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_lookup_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        
        let lookup_keyexpr = HostLookupKeyexpr::new(&prefix);
        assert_eq!(lookup_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_lookup_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        
        let lookup_keyexpr = HostLookupKeyexpr::new(&prefix);
        let keyexpr: KeyExpr = lookup_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/*");
        
        let parsed = HostLookupKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, host_id.clone(), client_id.clone());
        assert_eq!(client_keyexpr.host_id(), &host_id);
        assert_eq!(client_keyexpr.client_id(), &client_id);
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, host_id.clone(), client_id.clone());
        let keyexpr: KeyExpr = client_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/host1/client1");
        
        let parsed = HostClientKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &host_id);
        assert_eq!(parsed.client_id(), &client_id);
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/invalid/host1").unwrap();
        let result = HostKeyexpr::try_from(keyexpr);
        assert!(result.is_err());
    }

    #[test]
    fn test_host_lookup_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/host/host1").unwrap();
        let result = HostLookupKeyexpr::try_from(keyexpr);
        assert!(result.is_err());
    }

    #[test]
    fn test_host_client_lookup_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        
        let lookup_keyexpr = HostClientLookupKeyexpr::new(&prefix, host_id.clone());
        assert_eq!(lookup_keyexpr.host_id(), &host_id);
        assert_eq!(lookup_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_lookup_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        
        let lookup_keyexpr = HostClientLookupKeyexpr::new(&prefix, host_id.clone());
        let keyexpr: KeyExpr = lookup_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/host1/*");
        
        let parsed = HostClientLookupKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &host_id);
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_lookup_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/host/host1/client1").unwrap();
        let result = HostClientLookupKeyexpr::try_from(keyexpr);
        assert!(result.is_err());
    }
}

