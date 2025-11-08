/// Node management module
use std::time::Instant;

use crate::types::{NodeId, NodeInfo, NodeRole};

/// Node manager for tracking node identity and state
#[derive(Debug)]
pub struct Node {
    info: NodeInfo,
}

impl Node {
    /// Create a new node with the given ID and role
    pub fn new(id: NodeId, role: NodeRole) -> Self {
        Self {
            info: NodeInfo {
                id,
                role,
                connected_since: Instant::now(),
            },
        }
    }

    /// Get the node's ID
    pub fn id(&self) -> &NodeId {
        &self.info.id
    }

    /// Get the node's role
    pub fn role(&self) -> NodeRole {
        self.info.role
    }

    /// Get the node's full information
    pub fn info(&self) -> &NodeInfo {
        &self.info
    }

    /// Update the node's role
    pub fn set_role(&mut self, role: NodeRole) {
        self.info.role = role;
        self.info.connected_since = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_generation() {
        let id1 = NodeId::generate();
        let id2 = NodeId::generate();
        
        // Generated IDs should be different
        assert_ne!(id1, id2);
        
        // Should be non-empty
        assert!(!id1.as_str().is_empty());
    }

    #[test]
    fn test_node_id_from_name() {
        let result = NodeId::from_name("valid_name123".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "valid_name123");
    }

    #[test]
    fn test_node_id_invalid_characters() {
        // Test each invalid character
        assert!(NodeId::from_name("has/slash".to_string()).is_err());
        assert!(NodeId::from_name("has*star".to_string()).is_err());
        assert!(NodeId::from_name("has$dollar".to_string()).is_err());
        assert!(NodeId::from_name("has?question".to_string()).is_err());
        assert!(NodeId::from_name("has#hash".to_string()).is_err());
        assert!(NodeId::from_name("has@at".to_string()).is_err());
    }

    #[test]
    fn test_node_id_empty() {
        let result = NodeId::from_name("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_node_creation() {
        let id = NodeId::generate();
        let node = Node::new(id.clone(), NodeRole::Client);
        
        assert_eq!(node.id(), &id);
        assert_eq!(node.role(), NodeRole::Client);
    }

    #[test]
    fn test_node_role_update() {
        let id = NodeId::generate();
        let mut node = Node::new(id, NodeRole::Client);
        
        assert_eq!(node.role(), NodeRole::Client);
        
        node.set_role(NodeRole::Host);
        assert_eq!(node.role(), NodeRole::Host);
    }
}
