//! Outline item properties. Port of `packages/core/src/item.ts`.
//!
//! Keys are short because they are stored and synced for every node. Unset = falsy (see `Tree::apply`).
//! `c` content (inline markdown source), `n` note, `cb` shows a checkbox, `ck` completed, `h` heading 1..3,
//! `cl` color label 1..6, `num` children are numbered.

use crate::tree::{Props, TNode};

fn str_prop<'a>(n: Option<&'a TNode>, k: &str) -> &'a str {
    n.and_then(|n| n.props.get(k)).and_then(|v| v.as_str()).unwrap_or("")
}

fn int_prop(n: Option<&TNode>, k: &str) -> i64 {
    n.and_then(|n| n.props.get(k)).and_then(|v| v.as_f64()).map_or(0, |f| f as i64)
}

fn bool_prop(n: Option<&TNode>, k: &str) -> bool {
    n.and_then(|n| n.props.get(k)).is_some_and(|v| !crate::tree::is_falsy(v))
}

pub fn content(n: Option<&TNode>) -> &str {
    str_prop(n, "c")
}
pub fn note(n: Option<&TNode>) -> &str {
    str_prop(n, "n")
}
pub fn is_checked(n: Option<&TNode>) -> bool {
    bool_prop(n, "ck")
}
pub fn has_checkbox(n: Option<&TNode>) -> bool {
    bool_prop(n, "cb")
}
pub fn heading(n: Option<&TNode>) -> i64 {
    int_prop(n, "h")
}
pub fn color(n: Option<&TNode>) -> i64 {
    int_prop(n, "cl")
}
pub fn numbered(n: Option<&TNode>) -> bool {
    bool_prop(n, "num")
}

pub const COLOR_NAMES: [&str; 7] = ["none", "red", "orange", "yellow", "green", "blue", "purple"];

/// Item-level metadata copied by split / merge (everything except text).
pub fn meta_of(p: &Props) -> Props {
    p.iter().filter(|(k, _)| *k != "c" && *k != "n").map(|(k, v)| (k.clone(), v.clone())).collect()
}
