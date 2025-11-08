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
