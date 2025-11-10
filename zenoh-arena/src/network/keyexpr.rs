//! Key expression types for host discovery and connection

use crate::error::ArenaError;
use crate::node::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Role type for keyexpr
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Node role - `<prefix>/node/<node_id>` (node_a: node_id)
    Node,
    /// Host role - `<prefix>/host/<host_id>` (node_a: host_id)
    Host,
    /// Client role - `<prefix>/client/<client_id>` (node_a: client_id)
    Client,
    /// Shake role - `<prefix>/shake/<host_id>/<client_id>` (node_a: host_id, node_b: client_id)
    Shake,
    /// Link role - `<prefix>/link/<sender_id>/<receiver_id>` (node_a: sender_id, node_b: receiver_id)
    Link,
}

impl Role {
    /// Get the string representation of the role
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Node => "node",
            Role::Host => "host",
            Role::Client => "client",
            Role::Shake => "shake",
            Role::Link => "link",
        }
    }

    /// Parse a role from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "node" => Some(Role::Node),
            "host" => Some(Role::Host),
            "client" => Some(Role::Client),
            "shake" => Some(Role::Shake),
            "link" => Some(Role::Link),
            _ => None,
        }
    }

    /// Whether this role can have node_b (only Shake and Link do)
    pub fn has_remote_id(&self) -> bool {
        matches!(self, Role::Shake | Role::Link)
    }
}

/// Unified keyexpr for nodes, hosts, clients, and links
///
/// Pattern: `<prefix>/<role>/<node_a>` (for Node, Host, Client with specific IDs)
/// Pattern: `<prefix>/<role>/*` (for Node, Host, Client with wildcards)
/// Pattern: `<prefix>/shake/<node_a>/<node_b>` (for Shake: host_id/client_id)
/// Pattern: `<prefix>/link/<node_a>/<node_b>` (for Link: sender_id/receiver_id)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyexprTemplate {
    prefix: KeyExpr<'static>,
    role: Role,
    node_a: Option<NodeId>,
    node_b: Option<NodeId>,
}

impl KeyexprTemplate {
    /// Create a new NodeKeyexpr
    pub fn new<P: Into<KeyExpr<'static>>>(
        prefix: P,
        role: Role,
        node_a: Option<NodeId>,
        node_b: Option<NodeId>,
    ) -> Self {
        // Validate: node_b is only for Shake and Link roles
        if !role.has_remote_id() && node_b.is_some() {
            panic!("node_b can only be used with Shake or Link role");
        }
        Self {
            prefix: prefix.into(),
            role,
            node_a,
            node_b,
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

    /// Get node_a (None means wildcard)
    /// - Node: node_id
    /// - Host: host_id
    /// - Client: client_id
    /// - Shake: host_id
    /// - Link: sender_id
    pub fn node_a(&self) -> &Option<NodeId> {
        &self.node_a
    }

    /// Get node_b (only for Shake and Link roles, None means wildcard)
    /// - Shake: client_id
    /// - Link: receiver_id
    pub fn node_b(&self) -> &Option<NodeId> {
        &self.node_b
    }
}

impl TryFrom<KeyExpr<'_>> for KeyexprTemplate {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

        // Expected pattern: [...prefix]/<role>/<node_a>[/<node_b>]
        // At minimum: prefix, role, node_a (3 parts total if we count parts as separate)
        // For "arena/game1/host/host1" we have parts: ["arena", "game1", "host", "host1"]
        if parts.len() < 3 {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid NodeKeyexpr pattern: {}",
                keyexpr.as_str()
            )));
        }

        // Try to determine the role by looking backwards
        // For Shake/Link: [...prefix]/shake|link/<node_a>/<node_b> - at least 4 parts
        // For others: [...prefix]/<role>/<node_a> - at least 3 parts

        // First, try to interpret as Shake/Link (4 parts minimum)
        if parts.len() >= 4 {
            let possible_role_str = parts[parts.len() - 3];
            if let Some(role) = Role::from_str(possible_role_str) {
                if role.has_remote_id() {
                    // This is a Shake/Link with 4+ parts: [...prefix]/shake|link/<node_a>/<node_b>
                    let node_a_str = parts[parts.len() - 2];
                    let node_b_str = parts[parts.len() - 1];

                    let node_a = if node_a_str == "*" {
                        None
                    } else {
                        Some(NodeId::from_name(node_a_str.to_string())?)
                    };
                    let node_b = if node_b_str == "*" {
                        None
                    } else {
                        Some(NodeId::from_name(node_b_str.to_string())?)
                    };

                    let prefix_str = parts[..parts.len() - 3].join("/");
                    let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

                    return Ok(Self {
                        prefix,
                        role,
                        node_a,
                        node_b,
                    });
                }
            }
        }

        // Otherwise, interpret as 3-part pattern: [...prefix]/<role>/<node_a>
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
                "Shake/Link role requires node_b in keyexpr: {}",
                keyexpr.as_str()
            )));
        }

        let node_a_str = parts[parts.len() - 1];
        let node_a = if node_a_str == "*" {
            None
        } else {
            Some(NodeId::from_name(node_a_str.to_string())?)
        };

        let prefix_str = parts[..parts.len() - 2].join("/");
        let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

        Ok(Self {
            prefix,
            role,
            node_a,
            node_b: None,
        })
    }
}

impl From<KeyexprTemplate> for KeyExpr<'static> {
    fn from(keyexpr: KeyexprTemplate) -> Self {
        let keyexpr_str = if keyexpr.role.has_remote_id() {
            // Shake/Link role: <prefix>/shake|link/<node_a>/<node_b>
            let node_a_str = match &keyexpr.node_a {
                Some(id) => id.as_str().to_string(),
                None => "*".to_string(),
            };
            let node_b_str = match &keyexpr.node_b {
                Some(id) => id.as_str().to_string(),
                None => "*".to_string(),
            };
            format!(
                "{}/{}/{}/{}",
                keyexpr.prefix.as_str(),
                keyexpr.role.as_str(),
                node_a_str,
                node_b_str
            )
        } else {
            // Other roles: <prefix>/<role>/<node_a>
            let node_a_str = match &keyexpr.node_a {
                Some(id) => id.as_str().to_string(),
                None => "*".to_string(),
            };
            format!(
                "{}/{}/{}",
                keyexpr.prefix.as_str(),
                keyexpr.role.as_str(),
                node_a_str
            )
        };
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

/// Wrapper for Node role keyexpr: `<prefix>/node/<node_id>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyexprNode {
    template: KeyexprTemplate,
}

impl KeyexprNode {
    /// Create a new Node keyexpr
    pub fn new<P: Into<KeyExpr<'static>>>(prefix: P, node_id: Option<NodeId>) -> Self {
        Self {
            template: KeyexprTemplate::new(prefix, Role::Node, node_id, None),
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.template.prefix()
    }

    /// Get the node ID (None means wildcard)
    pub fn node_id(&self) -> &Option<NodeId> {
        self.template.node_a()
    }
}

impl From<KeyexprNode> for KeyExpr<'static> {
    fn from(keyexpr: KeyexprNode) -> Self {
        keyexpr.template.into()
    }
}

/// Wrapper for Host role keyexpr: `<prefix>/host/<host_id>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyexprHost {
    template: KeyexprTemplate,
}

impl KeyexprHost {
    /// Create a new Host keyexpr
    pub fn new<P: Into<KeyExpr<'static>>>(prefix: P, host_id: Option<NodeId>) -> Self {
        Self {
            template: KeyexprTemplate::new(prefix, Role::Host, host_id, None),
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.template.prefix()
    }

    /// Get the host ID (None means wildcard)
    pub fn host_id(&self) -> &Option<NodeId> {
        self.template.node_a()
    }
}

impl From<KeyexprHost> for KeyExpr<'static> {
    fn from(keyexpr: KeyexprHost) -> Self {
        keyexpr.template.into()
    }
}

/// Wrapper for Client role keyexpr: `<prefix>/client/<client_id>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyexprClient {
    template: KeyexprTemplate,
}

impl KeyexprClient {
    /// Create a new Client keyexpr
    pub fn new<P: Into<KeyExpr<'static>>>(prefix: P, client_id: Option<NodeId>) -> Self {
        Self {
            template: KeyexprTemplate::new(prefix, Role::Client, client_id, None),
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.template.prefix()
    }

    /// Get the client ID (None means wildcard)
    pub fn client_id(&self) -> &Option<NodeId> {
        self.template.node_a()
    }
}

impl From<KeyexprClient> for KeyExpr<'static> {
    fn from(keyexpr: KeyexprClient) -> Self {
        keyexpr.template.into()
    }
}

/// Wrapper for Shake role keyexpr: `<prefix>/shake/<host_id>/<client_id>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyexprShake {
    template: KeyexprTemplate,
}

impl KeyexprShake {
    /// Create a new Shake keyexpr for handshake
    pub fn new<P: Into<KeyExpr<'static>>>(
        prefix: P,
        host_id: Option<NodeId>,
        client_id: Option<NodeId>,
    ) -> Self {
        Self {
            template: KeyexprTemplate::new(prefix, Role::Shake, host_id, client_id),
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.template.prefix()
    }

    /// Get the host ID (None means wildcard)
    pub fn host_id(&self) -> &Option<NodeId> {
        self.template.node_a()
    }

    /// Get the client ID (None means wildcard)
    pub fn client_id(&self) -> &Option<NodeId> {
        self.template.node_b()
    }
}

impl From<KeyexprShake> for KeyExpr<'static> {
    fn from(keyexpr: KeyexprShake) -> Self {
        keyexpr.template.into()
    }
}

impl TryFrom<KeyExpr<'_>> for KeyexprShake {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let template = KeyexprTemplate::try_from(keyexpr)?;
        if template.role() != Role::Shake {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Expected Shake role, found {:?}",
                template.role()
            )));
        }
        Ok(Self { template })
    }
}

/// Wrapper for Link role keyexpr: `<prefix>/link/<sender_id>/<receiver_id>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyexprLink {
    template: KeyexprTemplate,
}

impl KeyexprLink {
    /// Create a new Link keyexpr for data communication
    pub fn new<P: Into<KeyExpr<'static>>>(
        prefix: P,
        sender_id: Option<NodeId>,
        receiver_id: Option<NodeId>,
    ) -> Self {
        Self {
            template: KeyexprTemplate::new(prefix, Role::Link, sender_id, receiver_id),
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        self.template.prefix()
    }

    /// Get the sender ID (None means wildcard)
    pub fn sender_id(&self) -> &Option<NodeId> {
        self.template.node_a()
    }

    /// Get the receiver ID (None means wildcard)
    pub fn receiver_id(&self) -> &Option<NodeId> {
        self.template.node_b()
    }
}

impl From<KeyexprLink> for KeyExpr<'static> {
    fn from(keyexpr: KeyexprLink) -> Self {
        keyexpr.template.into()
    }
}

impl TryFrom<KeyExpr<'_>> for KeyexprLink {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let template = KeyexprTemplate::try_from(keyexpr)?;
        if template.role() != Role::Link {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Expected Link role, found {:?}",
                template.role()
            )));
        }
        Ok(Self { template })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let node_a = NodeId::from_name("host1".to_string()).unwrap();
        let node_b = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprTemplate::new(
            prefix,
            Role::Link,
            Some(node_a.clone()),
            Some(node_b.clone()),
        );
        assert_eq!(link_keyexpr.node_a(), &Some(node_a));
        assert_eq!(link_keyexpr.node_b(), &Some(node_b));
        assert_eq!(link_keyexpr.prefix().as_str(), "arena/game1");
    }

    #[test]
    fn test_link_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let node_a = NodeId::from_name("host1".to_string()).unwrap();
        let node_b = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprTemplate::new(
            prefix,
            Role::Link,
            Some(node_a.clone()),
            Some(node_b.clone()),
        );
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/host1/client1");

        let parsed = KeyexprTemplate::try_from(keyexpr).unwrap();
        assert_eq!(parsed.node_a(), &Some(node_a));
        assert_eq!(parsed.node_b(), &Some(node_b));
        assert_eq!(parsed.role(), Role::Link);
    }

    #[test]
    fn test_link_keyexpr_wildcard_remote() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let node_a = NodeId::from_name("host1".to_string()).unwrap();

        let link_keyexpr = KeyexprTemplate::new(prefix, Role::Link, Some(node_a.clone()), None);
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/host1/*");

        let parsed = KeyexprTemplate::try_from(keyexpr).unwrap();
        assert_eq!(parsed.node_a(), &Some(node_a));
        assert_eq!(parsed.node_b(), &None);
        assert_eq!(parsed.role(), Role::Link);
    }

    #[test]
    fn test_link_keyexpr_wildcard_own() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let node_b = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprTemplate::new(prefix, Role::Link, None, Some(node_b.clone()));
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/*/client1");

        let parsed = KeyexprTemplate::try_from(keyexpr).unwrap();
        assert_eq!(parsed.node_a(), &None);
        assert_eq!(parsed.node_b(), &Some(node_b));
        assert_eq!(parsed.role(), Role::Link);
    }

    #[test]
    fn test_link_keyexpr_wildcard_both() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();

        let link_keyexpr = KeyexprTemplate::new(prefix, Role::Link, None, None);
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/*/*");

        let parsed = KeyexprTemplate::try_from(keyexpr).unwrap();
        assert_eq!(parsed.node_a(), &None);
        assert_eq!(parsed.node_b(), &None);
        assert_eq!(parsed.role(), Role::Link);
    }

    #[test]
    fn test_link_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/invalid/host1/client1").unwrap();
        let result = KeyexprTemplate::try_from(keyexpr);
        assert!(result.is_err());
    }
}
