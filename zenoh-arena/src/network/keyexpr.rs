//! Key expression types for host discovery and connection

use crate::error::ArenaError;
use crate::node::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Node type in the keyexpr
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// Generic node type
    Node,
    /// Client node type
    Client,
    /// Host node type
    Host,
}

impl NodeType {
    /// Get the string representation of the node type
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeType::Node => "node",
            NodeType::Client => "client",
            NodeType::Host => "host",
        }
    }

    /// Parse a node type from a string
    pub fn from_str(s: &str) -> Result<Self, ArenaError> {
        match s {
            "node" => Ok(NodeType::Node),
            "client" => Ok(NodeType::Client),
            "host" => Ok(NodeType::Host),
            _ => Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid node type: {}",
                s
            ))),
        }
    }
}

/// Link type for link keyexpr
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    /// Handshake link type (for discovery and connection)
    Handshake,
    /// Action link type (for pub/sub)
    Action,
    /// State link type (for query/reply)
    State,
}

impl LinkType {
    /// Get the string representation of the link type
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::Handshake => "handshake",
            LinkType::Action => "action",
            LinkType::State => "state",
        }
    }

    /// Parse a link type from a string
    pub fn from_str(s: &str) -> Result<Self, ArenaError> {
        match s {
            "handshake" => Ok(LinkType::Handshake),
            "action" => Ok(LinkType::Action),
            "state" => Ok(LinkType::State),
            _ => Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid link type: {}",
                s
            ))),
        }
    }
}

/// Keyexpr for single node operations
/// Format: `<prefix>/<node_type>/<node_id|*>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyexprNode {
    prefix: KeyExpr<'static>,
    node_type: NodeType,
    node: Option<NodeId>,
}

impl KeyexprNode {
    /// Create a new KeyexprNode
    pub fn new<P: Into<KeyExpr<'static>>>(
        prefix: P,
        node_type: NodeType,
        node: Option<NodeId>,
    ) -> Self {
        Self {
            prefix: prefix.into(),
            node_type,
            node,
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        &self.prefix
    }

    /// Get the node type
    pub fn node_type(&self) -> NodeType {
        self.node_type
    }

    /// Get the node ID (None means wildcard)
    pub fn node(&self) -> &Option<NodeId> {
        &self.node
    }
}

impl From<KeyexprNode> for KeyExpr<'static> {
    fn from(keyexpr: KeyexprNode) -> Self {
        let node_str = match &keyexpr.node {
            Some(id) => id.as_str().to_string(),
            None => "*".to_string(),
        };
        let keyexpr_str = format!(
            "{}/{}/{}",
            keyexpr.prefix.as_str(),
            keyexpr.node_type.as_str(),
            node_str
        );
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

impl TryFrom<KeyExpr<'_>> for KeyexprNode {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

        if parts.len() < 3 {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid keyexpr pattern: {}",
                keyexpr.as_str()
            )));
        }

        let node_type_str = parts[parts.len() - 2];
        let node_type = NodeType::from_str(node_type_str)?;

        let node_str = parts[parts.len() - 1];
        let node = if node_str == "*" {
            None
        } else {
            Some(NodeId::from_name(node_str.to_string())?)
        };

        let prefix_str = parts[..parts.len() - 2].join("/");
        let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

        Ok(Self { prefix, node_type, node })
    }
}

/// Keyexpr for link operations (handshake, pub/sub, query/reply)
/// Format: `<prefix>/<link_type>/<node_src|*>/<node_dst|*>`
///
/// # Handshake Protocol Semantics
///
/// For handshake operations (LinkType::Handshake):
/// - `node_src` represents the **requesting side** (client)
/// - `node_dst` represents the **response side** (host)
///
/// Examples:
/// - Discovery query: `<prefix>/handshake/<client_id>/*` (src=client, dst=wildcard)
/// - Host queryable: `<prefix>/handshake/*/<host_id>` (src=wildcard, dst=host)
/// - Connection query: `<prefix>/handshake/<client_id>/<host_id>` (src=client, dst=host)
///
/// # Other Link Types
///
/// For Action and State link types:
/// - `node_src` represents the message sender
/// - `node_dst` represents the message receiver
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyexprLink {
    prefix: KeyExpr<'static>,
    link_type: LinkType,
    node_src: Option<NodeId>,
    node_dst: Option<NodeId>,
}

impl KeyexprLink {
    /// Create a new KeyexprLink
    pub fn new<P: Into<KeyExpr<'static>>>(
        prefix: P,
        link_type: LinkType,
        node_src: Option<NodeId>,
        node_dst: Option<NodeId>,
    ) -> Self {
        Self {
            prefix: prefix.into(),
            link_type,
            node_src,
            node_dst,
        }
    }

    /// Get the prefix
    pub fn prefix(&self) -> &KeyExpr<'static> {
        &self.prefix
    }

    /// Get the link type
    pub fn link_type(&self) -> LinkType {
        self.link_type
    }

    /// Get the source node ID (None means wildcard)
    pub fn node_src(&self) -> &Option<NodeId> {
        &self.node_src
    }

    /// Get the destination node ID (None means wildcard)
    pub fn node_dst(&self) -> &Option<NodeId> {
        &self.node_dst
    }
}

impl From<KeyexprLink> for KeyExpr<'static> {
    fn from(keyexpr: KeyexprLink) -> Self {
        let src_str = match &keyexpr.node_src {
            Some(id) => id.as_str().to_string(),
            None => "*".to_string(),
        };
        let dst_str = match &keyexpr.node_dst {
            Some(id) => id.as_str().to_string(),
            None => "*".to_string(),
        };
        let keyexpr_str = format!(
            "{}/{}/{}/{}",
            keyexpr.prefix.as_str(),
            keyexpr.link_type.as_str(),
            src_str,
            dst_str
        );
        KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
    }
}

impl TryFrom<KeyExpr<'_>> for KeyexprLink {
    type Error = ArenaError;

    fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

        if parts.len() < 4 {
            return Err(ArenaError::InvalidKeyexpr(format!(
                "Invalid keyexpr pattern: {}",
                keyexpr.as_str()
            )));
        }

        let link_type_str = parts[parts.len() - 3];
        let link_type = LinkType::from_str(link_type_str)?;

        let src_str = parts[parts.len() - 2];
        let node_src = if src_str == "*" {
            None
        } else {
            Some(NodeId::from_name(src_str.to_string())?)
        };

        let dst_str = parts[parts.len() - 1];
        let node_dst = if dst_str == "*" {
            None
        } else {
            Some(NodeId::from_name(dst_str.to_string())?)
        };

        let prefix_str = parts[..parts.len() - 3].join("/");
        let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

        Ok(Self {
            prefix,
            link_type,
            node_src,
            node_dst,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let node_id = NodeId::from_name("mynode".to_string()).unwrap();

        let node_keyexpr = KeyexprNode::new(prefix.clone(), NodeType::Node, Some(node_id.clone()));
        assert_eq!(node_keyexpr.node_type(), NodeType::Node);
        assert_eq!(node_keyexpr.node(), &Some(node_id.clone()));
        assert_eq!(node_keyexpr.prefix().as_str(), "arena/game1");

        let host_keyexpr = KeyexprNode::new(prefix.clone(), NodeType::Host, Some(node_id.clone()));
        assert_eq!(host_keyexpr.node_type(), NodeType::Host);

        let client_keyexpr = KeyexprNode::new(prefix, NodeType::Client, Some(node_id));
        assert_eq!(client_keyexpr.node_type(), NodeType::Client);
    }

    #[test]
    fn test_node_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let node_id = NodeId::from_name("mynode".to_string()).unwrap();

        let node_keyexpr = KeyexprNode::new(prefix, NodeType::Host, Some(node_id.clone()));
        let keyexpr: KeyExpr = node_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/host/mynode");

        let parsed = KeyexprNode::try_from(keyexpr).unwrap();
        assert_eq!(parsed.node_type(), NodeType::Host);
        assert_eq!(parsed.node(), &Some(node_id));
    }

    #[test]
    fn test_node_keyexpr_wildcard() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();

        let node_keyexpr = KeyexprNode::new(prefix, NodeType::Client, None);
        let keyexpr: KeyExpr = node_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/client/*");

        let parsed = KeyexprNode::try_from(keyexpr).unwrap();
        assert_eq!(parsed.node_type(), NodeType::Client);
        assert_eq!(parsed.node(), &None);
    }

    #[test]
    fn test_link_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let src = NodeId::from_name("host1".to_string()).unwrap();
        let dst = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprLink::new(
            prefix,
            LinkType::Handshake,
            Some(src.clone()),
            Some(dst.clone()),
        );
        assert_eq!(link_keyexpr.link_type(), LinkType::Handshake);
        assert_eq!(link_keyexpr.node_src(), &Some(src));
        assert_eq!(link_keyexpr.node_dst(), &Some(dst));
        assert_eq!(link_keyexpr.prefix().as_str(), "arena/game1");
    }

    #[test]
    fn test_link_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let src = NodeId::from_name("host1".to_string()).unwrap();
        let dst = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprLink::new(
            prefix,
            LinkType::Action,
            Some(src.clone()),
            Some(dst.clone()),
        );
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/action/host1/client1");

        let parsed = KeyexprLink::try_from(keyexpr).unwrap();
        assert_eq!(parsed.link_type(), LinkType::Action);
        assert_eq!(parsed.node_src(), &Some(src));
        assert_eq!(parsed.node_dst(), &Some(dst));
    }

    #[test]
    fn test_link_keyexpr_wildcard_dst() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let src = NodeId::from_name("host1".to_string()).unwrap();

        let link_keyexpr = KeyexprLink::new(prefix, LinkType::State, Some(src.clone()), None);
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/state/host1/*");

        let parsed = KeyexprLink::try_from(keyexpr).unwrap();
        assert_eq!(parsed.link_type(), LinkType::State);
        assert_eq!(parsed.node_src(), &Some(src));
        assert_eq!(parsed.node_dst(), &None);
    }

    #[test]
    fn test_link_keyexpr_wildcard_src() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let dst = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprLink::new(prefix, LinkType::Handshake, None, Some(dst.clone()));
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/handshake/*/client1");

        let parsed = KeyexprLink::try_from(keyexpr).unwrap();
        assert_eq!(parsed.link_type(), LinkType::Handshake);
        assert_eq!(parsed.node_src(), &None);
        assert_eq!(parsed.node_dst(), &Some(dst));
    }

    #[test]
    fn test_link_keyexpr_wildcard_both() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();

        let link_keyexpr = KeyexprLink::new(prefix, LinkType::Action, None, None);
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/action/*/*");

        let parsed = KeyexprLink::try_from(keyexpr).unwrap();
        assert_eq!(parsed.link_type(), LinkType::Action);
        assert_eq!(parsed.node_src(), &None);
        assert_eq!(parsed.node_dst(), &None);
    }

    #[test]
    fn test_link_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/invalid/host1/client1").unwrap();
        let result = KeyexprLink::try_from(keyexpr);
        assert!(result.is_err());
    }

    #[test]
    fn test_node_type_parsing() {
        assert_eq!(NodeType::from_str("node").unwrap(), NodeType::Node);
        assert_eq!(NodeType::from_str("host").unwrap(), NodeType::Host);
        assert_eq!(NodeType::from_str("client").unwrap(), NodeType::Client);
        assert!(NodeType::from_str("invalid").is_err());
    }

    #[test]
    fn test_link_type_parsing() {
        assert_eq!(LinkType::from_str("handshake").unwrap(), LinkType::Handshake);
        assert_eq!(LinkType::from_str("action").unwrap(), LinkType::Action);
        assert_eq!(LinkType::from_str("state").unwrap(), LinkType::State);
        assert!(LinkType::from_str("invalid").is_err());
    }
}
