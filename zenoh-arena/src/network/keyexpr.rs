//! Key expression types for host discovery and connection

use crate::error::ArenaError;
use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Host keyexpr - represents a specific host or all hosts in the arena
///
/// Pattern: `<prefix>/host/<host_id>` (when host_id is Some)
/// Pattern: `<prefix>/host/*` (when host_id is None)
///
/// Used for:
/// - Declaring queryables to respond to connection requests
/// - Connecting to a specific host
/// - Discovering all available hosts (when host_id is None)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyexpr {
    prefix: String,
    host_id: Option<NodeId>,
}

impl HostKeyexpr {
    /// Create a new HostKeyexpr
    pub fn new(prefix: &KeyExpr, host_id: Option<NodeId>) -> Self {
        Self {
            prefix: prefix.to_string(),
            host_id,
        }
    }

    /// Get the host ID (None means wildcard)
    pub fn host_id(&self) -> &Option<NodeId> {
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
        
        // Expected pattern: [...prefix]/host/<host_id_or_wildcard>
        if parts.len() < 2 || parts[parts.len() - 2] != "host" {
            return Err(ArenaError::InvalidKeyexpr(
                format!("Invalid HostKeyexpr pattern: {}", keyexpr.as_str()),
            ));
        }

        let host_id_str = parts[parts.len() - 1];
        let host_id = if host_id_str == "*" {
            None
        } else {
            Some(NodeId::from_name(host_id_str.to_string())?)
        };
        let prefix = parts[..parts.len() - 2].join("/");

        Ok(Self { prefix, host_id })
    }
}

impl From<HostKeyexpr> for KeyExpr<'static> {
    fn from(host_keyexpr: HostKeyexpr) -> Self {
        let keyexpr_str = match host_keyexpr.host_id {
            Some(host_id) => format!("{}/host/{}", host_keyexpr.prefix, host_id.as_str()),
            None => format!("{}/host/*", host_keyexpr.prefix),
        };
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}


/// Host client keyexpr - used for initiating client connections or discovering clients
///
/// Pattern: `<prefix>/host/<host_id>/<client_id>` (when both are Some)
/// Pattern: `<prefix>/host/<host_id>/*` (when client_id is None)
/// Pattern: `<prefix>/host/*/<client_id>` (when host_id is None)
/// Pattern: `<prefix>/host/*/*` (when both are None)
///
/// Used when a client initiates a connection request to a specific host.
/// Supports glob patterns for flexible matching on host_id and/or client_id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClientKeyexpr {
    prefix: String,
    host_id: Option<NodeId>,
    client_id: Option<NodeId>,
}

impl HostClientKeyexpr {
    /// Create a new HostClientKeyexpr
    pub fn new(prefix: &KeyExpr, host_id: Option<NodeId>, client_id: Option<NodeId>) -> Self {
        Self { 
            prefix: prefix.to_string(), 
            host_id,
            client_id,
        }
    }

    /// Get the host ID (None means wildcard)
    pub fn host_id(&self) -> &Option<NodeId> {
        &self.host_id
    }

    /// Get the client ID (None means wildcard)
    pub fn client_id(&self) -> &Option<NodeId> {
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
        
        // Expected pattern: [...prefix]/host/<host_id_or_wildcard>/<client_id_or_wildcard>
        if parts.len() < 3 || parts[parts.len() - 3] != "host" {
            return Err(ArenaError::InvalidKeyexpr(
                format!("Invalid HostClientKeyexpr pattern: {}", keyexpr.as_str()),
            ));
        }

        let host_id_str = parts[parts.len() - 2];
        let client_id_str = parts[parts.len() - 1];
        
        let host_id = if host_id_str == "*" {
            None
        } else {
            Some(NodeId::from_name(host_id_str.to_string())?)
        };
        let client_id = if client_id_str == "*" {
            None
        } else {
            Some(NodeId::from_name(client_id_str.to_string())?)
        };
        let prefix = parts[..parts.len() - 3].join("/");

        Ok(Self { prefix, host_id, client_id })
    }
}

impl From<HostClientKeyexpr> for KeyExpr<'static> {
    fn from(client_keyexpr: HostClientKeyexpr) -> Self {
        let host_str = match &client_keyexpr.host_id {
            Some(host_id) => host_id.as_str().to_string(),
            None => "*".to_string(),
        };
        let client_str = match &client_keyexpr.client_id {
            Some(client_id) => client_id.as_str().to_string(),
            None => "*".to_string(),
        };
        let keyexpr_str = format!("{}/host/{}/{}", client_keyexpr.prefix, host_str, client_str);
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
        
        let host_keyexpr = HostKeyexpr::new(&prefix, Some(host_id.clone()));
        assert_eq!(host_keyexpr.host_id(), &Some(host_id));
        assert_eq!(host_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        
        let host_keyexpr = HostKeyexpr::new(&prefix, Some(host_id.clone()));
        let keyexpr: KeyExpr = host_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/host1");
        
        let parsed = HostKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &Some(host_id));
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_wildcard_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        
        let host_keyexpr = HostKeyexpr::new(&prefix, None);
        assert_eq!(host_keyexpr.host_id(), &None);
        assert_eq!(host_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_wildcard_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        
        let host_keyexpr = HostKeyexpr::new(&prefix, None);
        let keyexpr: KeyExpr = host_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/*");
        
        let parsed = HostKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &None);
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, Some(host_id.clone()), Some(client_id.clone()));
        assert_eq!(client_keyexpr.host_id(), &Some(host_id));
        assert_eq!(client_keyexpr.client_id(), &Some(client_id));
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, Some(host_id.clone()), Some(client_id.clone()));
        let keyexpr: KeyExpr = client_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/host1/client1");
        
        let parsed = HostClientKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &Some(host_id));
        assert_eq!(parsed.client_id(), &Some(client_id));
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_client_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, Some(host_id.clone()), None);
        assert_eq!(client_keyexpr.host_id(), &Some(host_id));
        assert_eq!(client_keyexpr.client_id(), &None);
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_client_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, Some(host_id.clone()), None);
        let keyexpr: KeyExpr = client_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/host1/*");
        
        let parsed = HostClientKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &Some(host_id));
        assert_eq!(parsed.client_id(), &None);
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_host_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, None, Some(client_id.clone()));
        assert_eq!(client_keyexpr.host_id(), &None);
        assert_eq!(client_keyexpr.client_id(), &Some(client_id));
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_host_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, None, Some(client_id.clone()));
        let keyexpr: KeyExpr = client_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/*/client1");
        
        let parsed = HostClientKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &None);
        assert_eq!(parsed.client_id(), &Some(client_id));
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_both_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, None, None);
        assert_eq!(client_keyexpr.host_id(), &None);
        assert_eq!(client_keyexpr.client_id(), &None);
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_both_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        
        let client_keyexpr = HostClientKeyexpr::new(&prefix, None, None);
        let keyexpr: KeyExpr = client_keyexpr.into();
        
        assert_eq!(keyexpr.as_str(), "arena/game1/host/*/*");
        
        let parsed = HostClientKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &None);
        assert_eq!(parsed.client_id(), &None);
        assert_eq!(parsed.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/invalid/host1").unwrap();
        let result = HostKeyexpr::try_from(keyexpr);
        assert!(result.is_err());
    }

    #[test]
    fn test_host_client_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/invalid/host1/client1").unwrap();
        let result = HostClientKeyexpr::try_from(keyexpr);
        assert!(result.is_err());
    }
}

