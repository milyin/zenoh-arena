//! Key expression types for host discovery and connection

use crate::error::ArenaError;
use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Role type for keyexpr
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Node role - `<prefix>/node/<own_id>`
    Node,
    /// Host role - `<prefix>/host/<own_id>`
    Host,
    /// Client role - `<prefix>/client/<own_id>`
    Client,
    /// Link role - `<prefix>/link/<own_id>/<remote_id>`
    Link,
}

impl Role {
    /// Get the string representation of the role
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Node => "node",
            Role::Host => "host",
            Role::Client => "client",
            Role::Link => "link",
        }
    }

    /// Parse a role from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "node" => Some(Role::Node),
            "host" => Some(Role::Host),
            "client" => Some(Role::Client),
            "link" => Some(Role::Link),
            _ => None,
        }
    }

    /// Whether this role can have remote_id (only Link does)
    pub fn has_remote_id(&self) -> bool {
        matches!(self, Role::Link)
    }
}

/// Unified keyexpr for nodes, hosts, clients, and links
///
/// Pattern: `<prefix>/<role>/<own_id>` (for Node, Host, Client with specific IDs)
/// Pattern: `<prefix>/<role>/*` (for Node, Host, Client with wildcards)
/// Pattern: `<prefix>/link/<own_id>/<remote_id>` (for Link with specific IDs)
/// Pattern: `<prefix>/link/<own_id>/*` (for Link with wildcard remote_id)
/// Pattern: `<prefix>/link/*/<remote_id>` (for Link with wildcard own_id)
/// Pattern: `<prefix>/link/*/*` (for Link with both wildcards)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeKeyexpr {
    prefix: KeyExpr<'static>,
    role: Role,
    own_id: Option<NodeId>,
    remote_id: Option<NodeId>,
}

impl NodeKeyexpr {
    /// Create a new NodeKeyexpr
    pub fn new<P: Into<KeyExpr<'static>>>(
        prefix: P,
        role: Role,
        own_id: Option<NodeId>,
        remote_id: Option<NodeId>,
    ) -> Self {
        // Validate: remote_id is only for Link role
        if !role.has_remote_id() && remote_id.is_some() {
            panic!("remote_id can only be used with Link role");
        }
        Self {
            prefix: prefix.into(),
            role,
            own_id,
            remote_id,
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        &self.prefix
    }

    /// Get the role
    pub fn role(&self) -> Role {
        self.role
    }

    /// Get the own ID (None means wildcard)
    pub fn own_id(&self) -> &Option<NodeId> {
        &self.own_id
    }

    /// Get the remote ID (only for Link role, None means wildcard)
    pub fn remote_id(&self) -> &Option<NodeId> {
        &self.remote_id
    }
}

impl TryFrom<KeyExpr<'_>> for NodeKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

        // Expected pattern: [...prefix]/<role>/<own_id>[/<remote_id>]
        // At minimum: prefix, role, own_id (3 parts total if we count parts as separate)
        // For "arena/game1/host/host1" we have parts: ["arena", "game1", "host", "host1"]
        if parts.len() < 3 {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid NodeKeyexpr pattern: {}",
                keyexpr.as_str()
            )));
        }

        // Try to determine the role by looking backwards
        // For Link: [...prefix]/link/<own_id>/<remote_id> - at least 4 parts
        // For others: [...prefix]/<role>/<own_id> - at least 3 parts
        
        // First, try to interpret as Link (4 parts minimum)
        if parts.len() >= 4 {
            let possible_role_str = parts[parts.len() - 3];
            if let Some(role) = Role::from_str(possible_role_str) {
                if role.has_remote_id() {
                    // This is a Link with 4+ parts: [...prefix]/link/<own_id>/<remote_id>
                    let own_id_str = parts[parts.len() - 2];
                    let remote_id_str = parts[parts.len() - 1];

                    let own_id = if own_id_str == "*" {
                        None
                    } else {
                        Some(NodeId::from_name(own_id_str.to_string())?)
                    };
                    let remote_id = if remote_id_str == "*" {
                        None
                    } else {
                        Some(NodeId::from_name(remote_id_str.to_string())?)
                    };

                    let prefix_str = parts[..parts.len() - 3].join("/");
                    let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

                    return Ok(Self {
                        prefix,
                        role,
                        own_id,
                        remote_id,
                    });
                }
            }
        }

        // Otherwise, interpret as 3-part pattern: [...prefix]/<role>/<own_id>
        let role_str = parts[parts.len() - 2];
        let role = Role::from_str(role_str).ok_or_else(|| {
            ArenaError::InvalidKeyexpr(format!(
                "Invalid role '{}' in keyexpr: {}",
                role_str,
                keyexpr.as_str()
            ))
        })?;

        if role.has_remote_id() {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Link role requires remote_id in keyexpr: {}",
                keyexpr.as_str()
            )));
        }

        let own_id_str = parts[parts.len() - 1];
        let own_id = if own_id_str == "*" {
            None
        } else {
            Some(NodeId::from_name(own_id_str.to_string())?)
        };

        let prefix_str = parts[..parts.len() - 2].join("/");
        let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

        Ok(Self {
            prefix,
            role,
            own_id,
            remote_id: None,
        })
    }
}

impl From<NodeKeyexpr> for KeyExpr<'static> {
    fn from(keyexpr: NodeKeyexpr) -> Self {
        let keyexpr_str = if keyexpr.role.has_remote_id() {
            // Link role: <prefix>/link/<own_id>/<remote_id>
            let own_id_str = match &keyexpr.own_id {
                Some(id) => id.as_str().to_string(),
                None => "*".to_string(),
            };
            let remote_id_str = match &keyexpr.remote_id {
                Some(id) => id.as_str().to_string(),
                None => "*".to_string(),
            };
            format!(
                "{}/{}/{}/{}",
                keyexpr.prefix.as_str(),
                keyexpr.role.as_str(),
                own_id_str,
                remote_id_str
            )
        } else {
            // Other roles: <prefix>/<role>/<own_id>
            let own_id_str = match &keyexpr.own_id {
                Some(id) => id.as_str().to_string(),
                None => "*".to_string(),
            };
            format!(
                "{}/{}/{}",
                keyexpr.prefix.as_str(),
                keyexpr.role.as_str(),
                own_id_str
            )
        };
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

/// Host keyexpr - wrapper for NodeKeyexpr with Host role
///
/// Pattern: `<prefix>/host/<host_id>` (when host_id is Some)
/// Pattern: `<prefix>/host/*` (when host_id is None)
///
/// Used for:
/// - Declaring queryables to respond to connection requests
/// - Connecting to a specific host
/// - Discovering all available hosts (when host_id is None)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyexpr(NodeKeyexpr);

impl HostKeyexpr {
    /// Create a new HostKeyexpr
    pub fn new<P: Into<KeyExpr<'static>>>(prefix: P, host_id: Option<NodeId>) -> Self {
        Self(NodeKeyexpr::new(prefix, Role::Host, host_id, None))
    }

    /// Get the host ID (None means wildcard)
    pub fn host_id(&self) -> &Option<NodeId> {
        self.0.own_id()
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.0.prefix()
    }
}

impl TryFrom<KeyExpr<'_>> for HostKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let node_keyexpr = NodeKeyexpr::try_from(keyexpr)?;
        if node_keyexpr.role() != Role::Host {
            return Err(ArenaError::InvalidKeyexpr(
                "Expected Host role in keyexpr".to_string(),
            ));
        }
        Ok(Self(node_keyexpr))
    }
}

impl From<HostKeyexpr> for KeyExpr<'static> {
    fn from(host_keyexpr: HostKeyexpr) -> Self {
        KeyExpr::from(host_keyexpr.0)
    }
}

/// Node keyexpr - wrapper for NodeKeyexpr with Node role
///
/// Pattern: `<prefix>/node/<node_id>` (when node_id is Some)
/// Pattern: `<prefix>/node/*` (when node_id is None)
///
/// Used for:
/// - Declaring queryables for node-related operations
/// - Connecting to a specific node
/// - Discovering all available nodes (when node_id is None)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeKeyexprWrapper(NodeKeyexpr);

impl NodeKeyexprWrapper {
    /// Create a new NodeKeyexprWrapper
    pub fn new<P: Into<KeyExpr<'static>>>(prefix: P, node_id: Option<NodeId>) -> Self {
        Self(NodeKeyexpr::new(prefix, Role::Node, node_id, None))
    }

    /// Get the node ID (None means wildcard)
    pub fn node_id(&self) -> &Option<NodeId> {
        self.0.own_id()
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.0.prefix()
    }
}

impl TryFrom<KeyExpr<'_>> for NodeKeyexprWrapper {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let node_keyexpr = NodeKeyexpr::try_from(keyexpr)?;
        if node_keyexpr.role() != Role::Node {
            return Err(ArenaError::InvalidKeyexpr(
                "Expected Node role in keyexpr".to_string(),
            ));
        }
        Ok(Self(node_keyexpr))
    }
}

impl From<NodeKeyexprWrapper> for KeyExpr<'static> {
    fn from(node_keyexpr: NodeKeyexprWrapper) -> Self {
        KeyExpr::from(node_keyexpr.0)
    }
}

/// Host client keyexpr - used for initiating client connections or discovering clients
///
/// Pattern: `<prefix>/link/<host_id>/<client_id>` (when both are Some)
/// Pattern: `<prefix>/link/<host_id>/*` (when client_id is None)
/// Pattern: `<prefix>/link/*/<client_id>` (when host_id is None)
/// Pattern: `<prefix>/link/*/*` (when both are None)
///
/// Used when a client initiates a connection request to a specific host.
/// Supports glob patterns for flexible matching on host_id and/or client_id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClientKeyexpr(NodeKeyexpr);

impl HostClientKeyexpr {
    /// Create a new HostClientKeyexpr
    pub fn new(
        prefix: impl Into<KeyExpr<'static>>,
        host_id: Option<NodeId>,
        client_id: Option<NodeId>,
    ) -> Self {
        Self(NodeKeyexpr::new(prefix, Role::Link, host_id, client_id))
    }

    /// Get the host ID (None means wildcard)
    pub fn host_id(&self) -> &Option<NodeId> {
        self.0.own_id()
    }

    /// Get the client ID (None means wildcard)
    pub fn client_id(&self) -> &Option<NodeId> {
        self.0.remote_id()
    }

    /// Get the prefix
    pub fn prefix(&self) -> &str {
        self.0.prefix().as_str()
    }
}

impl TryFrom<KeyExpr<'_>> for HostClientKeyexpr {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let node_keyexpr = NodeKeyexpr::try_from(keyexpr)?;
        if node_keyexpr.role() != Role::Link {
            return Err(ArenaError::InvalidKeyexpr(
                "Expected Link role in keyexpr".to_string(),
            ));
        }
        Ok(Self(node_keyexpr))
    }
}

impl From<HostClientKeyexpr> for KeyExpr<'static> {
    fn from(client_keyexpr: HostClientKeyexpr) -> Self {
        KeyExpr::from(client_keyexpr.0)
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

        assert_eq!(keyexpr.as_str(), "arena/game1/link/host1/client1");

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

        assert_eq!(keyexpr.as_str(), "arena/game1/link/host1/*");

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

        assert_eq!(keyexpr.as_str(), "arena/game1/link/*/client1");

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

        assert_eq!(keyexpr.as_str(), "arena/game1/link/*/*");

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
