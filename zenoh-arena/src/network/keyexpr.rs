//! Key expression types for host discovery and connection

use crate::error::ArenaError;
use crate::node::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Macro to define single-node keyexpr wrappers (Node, Host, Client)
/// These wrappers have only one node ID field
macro_rules! define_single_node_keyexpr {
    (
        $(#[$meta:meta])*
        $name:ident,
        $role_str:expr,
        $id_name:ident,
        $doc_comment:expr
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            prefix: KeyExpr<'static>,
            $id_name: Option<NodeId>,
        }

        impl $name {
            #[doc = $doc_comment]
            pub fn new<P: Into<KeyExpr<'static>>>(prefix: P, $id_name: Option<NodeId>) -> Self {
                Self {
                    prefix: prefix.into(),
                    $id_name,
                }
            }

            /// Get the prefix
            pub fn prefix(&self) -> &KeyExpr<'static> {
                &self.prefix
            }

            #[doc = concat!("Get the ", stringify!($id_name), " (None means wildcard)")]
            pub fn $id_name(&self) -> &Option<NodeId> {
                &self.$id_name
            }
        }

        impl From<$name> for KeyExpr<'static> {
            fn from(keyexpr: $name) -> Self {
                let id_str = match &keyexpr.$id_name {
                    Some(id) => id.as_str().to_string(),
                    None => "*".to_string(),
                };
                let keyexpr_str = format!(
                    "{}/{}/{}",
                    keyexpr.prefix.as_str(),
                    $role_str,
                    id_str
                );
                KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
            }
        }

        impl TryFrom<KeyExpr<'_>> for $name {
            type Error = ArenaError;

            fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
                let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

                if parts.len() < 3 {
                    return Err(ArenaError::InvalidKeyexpr(format!(
                        "Invalid keyexpr pattern: {}",
                        keyexpr.as_str()
                    )));
                }

                let role_str = parts[parts.len() - 2];
                if role_str != $role_str {
                    return Err(ArenaError::InvalidKeyexpr(format!(
                        "Expected {} role, found '{}'",
                        $role_str,
                        role_str
                    )));
                }

                let id_str = parts[parts.len() - 1];
                let $id_name = if id_str == "*" {
                    None
                } else {
                    Some(NodeId::from_name(id_str.to_string())?)
                };

                let prefix_str = parts[..parts.len() - 2].join("/");
                let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

                Ok(Self { prefix, $id_name })
            }
        }
    };
}

/// Macro to define dual-node keyexpr wrappers (Shake, Link)
/// These wrappers have two node ID fields
macro_rules! define_dual_node_keyexpr {
    (
        $(#[$meta:meta])*
        $name:ident,
        $role_str:expr,
        $id_a_name:ident,
        $id_b_name:ident,
        $doc_comment:expr
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            prefix: KeyExpr<'static>,
            $id_a_name: Option<NodeId>,
            $id_b_name: Option<NodeId>,
        }

        impl $name {
            #[doc = $doc_comment]
            pub fn new<P: Into<KeyExpr<'static>>>(
                prefix: P,
                $id_a_name: Option<NodeId>,
                $id_b_name: Option<NodeId>,
            ) -> Self {
                Self {
                    prefix: prefix.into(),
                    $id_a_name,
                    $id_b_name,
                }
            }

            /// Get the prefix
            pub fn prefix(&self) -> &KeyExpr<'static> {
                &self.prefix
            }

            #[doc = concat!("Get the ", stringify!($id_a_name), " (None means wildcard)")]
            pub fn $id_a_name(&self) -> &Option<NodeId> {
                &self.$id_a_name
            }

            #[doc = concat!("Get the ", stringify!($id_b_name), " (None means wildcard)")]
            pub fn $id_b_name(&self) -> &Option<NodeId> {
                &self.$id_b_name
            }
        }

        impl From<$name> for KeyExpr<'static> {
            fn from(keyexpr: $name) -> Self {
                let id_a_str = match &keyexpr.$id_a_name {
                    Some(id) => id.as_str().to_string(),
                    None => "*".to_string(),
                };
                let id_b_str = match &keyexpr.$id_b_name {
                    Some(id) => id.as_str().to_string(),
                    None => "*".to_string(),
                };
                let keyexpr_str = format!(
                    "{}/{}/{}/{}",
                    keyexpr.prefix.as_str(),
                    $role_str,
                    id_a_str,
                    id_b_str
                );
                KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
            }
        }

        impl TryFrom<KeyExpr<'_>> for $name {
            type Error = ArenaError;

            fn try_from(keyexpr: KeyExpr<'_>) -> Result<Self, Self::Error> {
                let parts: Vec<&str> = keyexpr.as_str().split('/').collect();

                if parts.len() < 4 {
                    return Err(ArenaError::InvalidKeyexpr(format!(
                        "Invalid keyexpr pattern: {}",
                        keyexpr.as_str()
                    )));
                }

                let role_str = parts[parts.len() - 3];
                if role_str != $role_str {
                    return Err(ArenaError::InvalidKeyexpr(format!(
                        "Expected {} role, found '{}'",
                        $role_str,
                        role_str
                    )));
                }

                let id_a_str = parts[parts.len() - 2];
                let $id_a_name = if id_a_str == "*" {
                    None
                } else {
                    Some(NodeId::from_name(id_a_str.to_string())?)
                };

                let id_b_str = parts[parts.len() - 1];
                let $id_b_name = if id_b_str == "*" {
                    None
                } else {
                    Some(NodeId::from_name(id_b_str.to_string())?)
                };

                let prefix_str = parts[..parts.len() - 3].join("/");
                let prefix = KeyExpr::try_from(prefix_str)?.into_owned();

                Ok(Self {
                    prefix,
                    $id_a_name,
                    $id_b_name,
                })
            }
        }
    };
}

// Define single-node keyexpr types
define_single_node_keyexpr!(
    /// Wrapper for Node role keyexpr: `<prefix>/node/<node_id>`
    KeyexprNode,
    "node",
    node_id,
    "Create a new Node keyexpr"
);

define_single_node_keyexpr!(
    /// Wrapper for Host role keyexpr: `<prefix>/host/<host_id>`
    KeyexprHost,
    "host",
    host_id,
    "Create a new Host keyexpr"
);

define_single_node_keyexpr!(
    /// Wrapper for Client role keyexpr: `<prefix>/client/<client_id>`
    KeyexprClient,
    "client",
    client_id,
    "Create a new Client keyexpr"
);

// Define dual-node keyexpr types
define_dual_node_keyexpr!(
    /// Wrapper for Shake role keyexpr: `<prefix>/shake/<host_id>/<client_id>`
    KeyexprShake,
    "shake",
    host_id,
    client_id,
    "Create a new Shake keyexpr for handshake"
);

define_dual_node_keyexpr!(
    /// Wrapper for Link role keyexpr: `<prefix>/link/<sender_id>/<receiver_id>`
    KeyexprLink,
    "link",
    sender_id,
    receiver_id,
    "Create a new Link keyexpr for data communication"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_keyexpr_creation() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let sender = NodeId::from_name("host1".to_string()).unwrap();
        let receiver = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprLink::new(
            prefix,
            Some(sender.clone()),
            Some(receiver.clone()),
        );
        assert_eq!(link_keyexpr.sender_id(), &Some(sender));
        assert_eq!(link_keyexpr.receiver_id(), &Some(receiver));
        assert_eq!(link_keyexpr.prefix().as_str(), "arena/game1");
    }

    #[test]
    fn test_link_keyexpr_roundtrip() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let sender = NodeId::from_name("host1".to_string()).unwrap();
        let receiver = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprLink::new(
            prefix,
            Some(sender.clone()),
            Some(receiver.clone()),
        );
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/host1/client1");

        let parsed = KeyexprLink::try_from(keyexpr).unwrap();
        assert_eq!(parsed.sender_id(), &Some(sender));
        assert_eq!(parsed.receiver_id(), &Some(receiver));
    }

    #[test]
    fn test_link_keyexpr_wildcard_remote() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let sender = NodeId::from_name("host1".to_string()).unwrap();

        let link_keyexpr = KeyexprLink::new(prefix, Some(sender.clone()), None);
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/host1/*");

        let parsed = KeyexprLink::try_from(keyexpr).unwrap();
        assert_eq!(parsed.sender_id(), &Some(sender));
        assert_eq!(parsed.receiver_id(), &None);
    }

    #[test]
    fn test_link_keyexpr_wildcard_own() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();
        let receiver = NodeId::from_name("client1".to_string()).unwrap();

        let link_keyexpr = KeyexprLink::new(prefix, None, Some(receiver.clone()));
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/*/client1");

        let parsed = KeyexprLink::try_from(keyexpr).unwrap();
        assert_eq!(parsed.sender_id(), &None);
        assert_eq!(parsed.receiver_id(), &Some(receiver));
    }

    #[test]
    fn test_link_keyexpr_wildcard_both() {
        let prefix = KeyExpr::try_from("arena/game1").unwrap();

        let link_keyexpr = KeyexprLink::new(prefix, None, None);
        let keyexpr: KeyExpr = link_keyexpr.into();

        assert_eq!(keyexpr.as_str(), "arena/game1/link/*/*");

        let parsed = KeyexprLink::try_from(keyexpr).unwrap();
        assert_eq!(parsed.sender_id(), &None);
        assert_eq!(parsed.receiver_id(), &None);
    }

    #[test]
    fn test_link_keyexpr_invalid_pattern() {
        let keyexpr = KeyExpr::try_from("arena/game1/invalid/host1/client1").unwrap();
        let result = KeyexprLink::try_from(keyexpr);
        assert!(result.is_err());
    }
}
