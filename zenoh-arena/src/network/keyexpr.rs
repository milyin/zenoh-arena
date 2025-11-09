//! Key expression helper for liveliness tokens

use crate::types::NodeId;
use zenoh::key_expr::KeyExpr;

/// Build a node key expression
///
/// Pattern: `<prefix>/node/<node_id>`
///
/// Both prefix and node_id are guaranteed to be valid keyexprs,
/// so the result is always valid.
pub fn node_keyexpr(prefix: &KeyExpr, node_id: &NodeId) -> KeyExpr<'static> {
    let keyexpr_str = format!("{}/node/{}", prefix, node_id.as_str());
    // Safe to unwrap because both inputs are valid keyexprs
    KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
}

/// Build a query key expression for finding available hosts
///
/// Pattern: `<prefix>/node/*`
///
/// This allows querying for all nodes in the arena.
pub fn query_nodes_keyexpr(prefix: &KeyExpr) -> KeyExpr<'static> {
    let keyexpr_str = format!("{}/node/*", prefix);
    // Safe to unwrap because prefix is a valid keyexpr
    KeyExpr::try_from(keyexpr_str).unwrap().into_owned()
}
