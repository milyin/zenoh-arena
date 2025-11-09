//! Key expression types for host discovery and connection

use crate::error::ArenaError;
use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Generic NodeId-based keyexpr - represents a node or all nodes with a specific nodeid_prefix
///
/// Pattern: `<prefix>/<nodeid_prefix>/<node_id>` (when node_id is Some)
/// Pattern: `<prefix>/<nodeid_prefix>/*` (when node_id is None)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeIdKeyexpr {
    prefix: KeyExpr<'static>,
    nodeid_prefix: KeyExpr<'static>,
    node_id: Option<NodeId>,
}

impl NodeIdKeyexpr {
    /// Create a new NodeIdKeyexpr with a given nodeid_prefix
    pub fn new<P: Into<KeyExpr<'static>>, NP: Into<KeyExpr<'static>>>(
        prefix: P,
        nodeid_prefix: NP,
        node_id: Option<NodeId>,
    ) -> Self {
        Self {
            prefix: prefix.into(),
            nodeid_prefix: nodeid_prefix.into(),
            node_id,
        }
    }

    /// Get the node ID (None means wildcard)
    pub fn node_id(&self) -> &Option<NodeId> {
        &self.node_id
    }

    /// Get the prefix as KeyExpr
    pub fn prefix(&self) -> &KeyExpr<'static> {
        &self.prefix
    }

    /// Get the nodeid_prefix as KeyExpr
    pub fn nodeid_prefix(&self) -> &KeyExpr<'static> {
        &self.nodeid_prefix
    }
}

impl TryFrom<KeyExpr<'_>> for NodeIdKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

        // Expected pattern: [...prefix]/<nodeid_prefix>/<node_id_or_wildcard>
        if parts.len() < 2 {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid NodeIdKeyexpr pattern: {}",
                keyexpr.as_str()
            )));
        }

        let nodeid_prefix_str = parts[parts.len() - 2];
        let nodeid_prefix = KeyExpr::try_from(nodeid_prefix_str)?.into_owned();
        let node_id_str = parts[parts.len() - 1];
        let node_id = if node_id_str == "*" {
            None
        } else {
            Some(NodeId::from_name(node_id_str.to_string())?)
        };
        let prefix_str = parts[..parts.len() - 2].join("/");
        let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

        Ok(Self {
            prefix,
            nodeid_prefix,
            node_id,
        })
    }
}

impl From<NodeIdKeyexpr> for KeyExpr<'static> {
    fn from(node_id_keyexpr: NodeIdKeyexpr) -> Self {
        let keyexpr_str = match node_id_keyexpr.node_id {
            Some(node_id) => format!(
                "{}/{}/{}",
                node_id_keyexpr.prefix.as_str(),
                node_id_keyexpr.nodeid_prefix,
                node_id.as_str()
            ),
            None => format!(
                "{}/{}/*",
                node_id_keyexpr.prefix.as_str(),
                node_id_keyexpr.nodeid_prefix
            ),
        };
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

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
    inner: NodeIdKeyexpr,
}

impl HostKeyexpr {
    /// Create a new HostKeyexpr
    pub fn new<P: Into<KeyExpr<'static>>>(prefix: P, host_id: Option<NodeId>) -> Self {
        let host_keyexpr = KeyExpr::try_from("host").unwrap();
        Self {
            inner: NodeIdKeyexpr::new(prefix, host_keyexpr, host_id),
        }
    }

    /// Get the host ID (None means wildcard)
    pub fn host_id(&self) -> &Option<NodeId> {
        self.inner.node_id()
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.inner.prefix()
    }
}

impl TryFrom<KeyExpr<'_>> for HostKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

        // Expected pattern: [...prefix]/host/<host_id_or_wildcard>
        if parts.len() < 2 || parts[parts.len() - 2] != "host" {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid HostKeyexpr pattern: {}",
                keyexpr.as_str()
            )));
        }

        let inner = NodeIdKeyexpr::try_from(keyexpr)?;
        Ok(Self { inner })
    }
}

impl From<HostKeyexpr> for KeyExpr<'static> {
    fn from(host_keyexpr: HostKeyexpr) -> Self {
        KeyExpr::from(host_keyexpr.inner)
    }
}

/// Node keyexpr - represents a specific node or all nodes in the arena
///
/// Pattern: `<prefix>/node/<node_id>` (when node_id is Some)
/// Pattern: `<prefix>/node/*` (when node_id is None)
///
/// Used for:
/// - Declaring queryables for node-related operations
/// - Connecting to a specific node
/// - Discovering all available nodes (when node_id is None)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeKeyexpr {
    inner: NodeIdKeyexpr,
}

impl NodeKeyexpr {
    /// Create a new NodeKeyexpr
    pub fn new<P: Into<KeyExpr<'static>>>(prefix: P, node_id: Option<NodeId>) -> Self {
        let node_keyexpr = KeyExpr::try_from("node").unwrap();
        Self {
            inner: NodeIdKeyexpr::new(prefix, node_keyexpr, node_id),
        }
    }

    /// Get the node ID (None means wildcard)
    pub fn node_id(&self) -> &Option<NodeId> {
        self.inner.node_id()
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.inner.prefix()
    }
}

impl TryFrom<KeyExpr<'_>> for NodeKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

        // Expected pattern: [...prefix]/node/<node_id_or_wildcard>
        if parts.len() < 2 || parts[parts.len() - 2] != "node" {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid NodeKeyexpr pattern: {}",
                keyexpr.as_str()
            )));
        }

        let inner = NodeIdKeyexpr::try_from(keyexpr)?;
        Ok(Self { inner })
    }
}

impl From<NodeKeyexpr> for KeyExpr<'static> {
    fn from(node_keyexpr: NodeKeyexpr) -> Self {
        KeyExpr::from(node_keyexpr.inner)
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
    prefix: KeyExpr<'static>,
    host_id: Option<NodeId>,
    client_id: Option<NodeId>,
}

impl HostClientKeyexpr {
    /// Create a new HostClientKeyexpr
    pub fn new(
        prefix: impl Into<KeyExpr<'static>>,
        host_id: Option<NodeId>,
        client_id: Option<NodeId>,
    ) -> Self {
        Self {
            prefix: prefix.into(),
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
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid HostClientKeyexpr pattern: {}",
                keyexpr.as_str()
            )));
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
        let prefix = KeyExpr::try_from(prefix).unwrap().into_owned();

        Ok(Self {
            prefix,
            host_id,
            client_id,
        })
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
        let prefix = KeyExpr::try_from("arena/game1").unwrap().into_owned();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();

        let host_keyexpr = HostKeyexpr::new(prefix, Some(host_id.clone()));
        assert_eq!(host_keyexpr.host_id(), &Some(host_id));
        assert_eq!(host_keyexpr.prefix().as_str(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap().into_owned();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();

        let host_keyexpr = HostKeyexpr::new(prefix, Some(host_id.clone()));
        let keyexpr: KeyExpr = host_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/host/host1");

        let parsed = HostKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &Some(host_id));
        assert_eq!(parsed.prefix().as_str(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_wildcard_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap().into_owned();

        let host_keyexpr = HostKeyexpr::new(prefix, None);
        assert_eq!(host_keyexpr.host_id(), &None);
        assert_eq!(host_keyexpr.prefix().as_str(), "arena/game1");
    }

    #[test]
    fn test_host_keyexpr_wildcard_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap().into_owned();

        let host_keyexpr = HostKeyexpr::new(prefix, None);
        let keyexpr: KeyExpr = host_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/host/*");

        let parsed = HostKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.host_id(), &None);
        assert_eq!(parsed.prefix().as_str(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();

        let client_keyexpr =
            HostClientKeyexpr::new(prefix, Some(host_id.clone()), Some(client_id.clone()));
        assert_eq!(client_keyexpr.host_id(), &Some(host_id));
        assert_eq!(client_keyexpr.client_id(), &Some(client_id));
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();

        let client_keyexpr =
            HostClientKeyexpr::new(prefix, Some(host_id.clone()), Some(client_id.clone()));
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

        let client_keyexpr = HostClientKeyexpr::new(prefix, Some(host_id.clone()), None);
        assert_eq!(client_keyexpr.host_id(), &Some(host_id));
        assert_eq!(client_keyexpr.client_id(), &None);
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_client_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let host_id = NodeId::from_name("host1".to_string()).unwrap();

        let client_keyexpr = HostClientKeyexpr::new(prefix, Some(host_id.clone()), None);
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

        let client_keyexpr = HostClientKeyexpr::new(prefix, None, Some(client_id.clone()));
        assert_eq!(client_keyexpr.host_id(), &None);
        assert_eq!(client_keyexpr.client_id(), &Some(client_id));
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_host_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let client_id = NodeId::from_name("client1".to_string()).unwrap();

        let client_keyexpr = HostClientKeyexpr::new(prefix, None, Some(client_id.clone()));
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

        let client_keyexpr = HostClientKeyexpr::new(prefix, None, None);
        assert_eq!(client_keyexpr.host_id(), &None);
        assert_eq!(client_keyexpr.client_id(), &None);
        assert_eq!(client_keyexpr.prefix(), "arena/game1");
    }

    #[test]
    fn test_host_client_keyexpr_wildcard_both_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();

        let client_keyexpr = HostClientKeyexpr::new(prefix, None, None);
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
