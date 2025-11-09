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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let own_id = NodeId::from_name("host1".to_string()).unwrap();
        let remote_id = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = NodeKeyexpr::new(prefix, Role::Link, Some(own_id.clone()), Some(remote_id.clone()));
        assert_eq!(link_keyexpr.own_id(), &Some(own_id));
        assert_eq!(link_keyexpr.remote_id(), &Some(remote_id));
        assert_eq!(link_keyexpr.prefix().as_str(), "arena/game1");
    }

    #[test]
    fn test_link_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let own_id = NodeId::from_name("host1".to_string()).unwrap();
        let remote_id = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = NodeKeyexpr::new(prefix, Role::Link, Some(own_id.clone()), Some(remote_id.clone()));
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/host1/client1");

        let parsed = NodeKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.own_id(), &Some(own_id));
        assert_eq!(parsed.remote_id(), &Some(remote_id));
        assert_eq!(parsed.role(), Role::Link);
    }

    #[test]
    fn test_link_keyexpr_wildcard_remote() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let own_id = NodeId::from_name("host1".to_string()).unwrap();

        let link_keyexpr = NodeKeyexpr::new(prefix, Role::Link, Some(own_id.clone()), None);
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/host1/*");

        let parsed = NodeKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.own_id(), &Some(own_id));
        assert_eq!(parsed.remote_id(), &None);
        assert_eq!(parsed.role(), Role::Link);
    }

    #[test]
    fn test_link_keyexpr_wildcard_own() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let remote_id = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = NodeKeyexpr::new(prefix, Role::Link, None, Some(remote_id.clone()));
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/*/client1");

        let parsed = NodeKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.own_id(), &None);
        assert_eq!(parsed.remote_id(), &Some(remote_id));
        assert_eq!(parsed.role(), Role::Link);
    }

    #[test]
    fn test_link_keyexpr_wildcard_both() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();

        let link_keyexpr = NodeKeyexpr::new(prefix, Role::Link, None, None);
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/*/*");

        let parsed = NodeKeyexpr::try_from(keyexpr).unwrap();
        assert_eq!(parsed.own_id(), &None);
        assert_eq!(parsed.remote_id(), &None);
        assert_eq!(parsed.role(), Role::Link);
    }

    #[test]
    fn test_link_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/invalid/host1/client1").unwrap();
        let result = NodeKeyexpr::try_from(keyexpr);
        assert!(result.is_err());
    }
}
