//! What the user is looking at, and visibility over the tree. Port of `packages/core/src/view.ts`.

use std::collections::{HashMap, HashSet};

use crate::tree::{NodeId, Tree, ROOT};

/// What the user is currently looking at; drives visibility.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub zoom: NodeId,
    pub show_checked: bool,
    /// Active search filter: ids that match or are ancestors of a match. When set, collapse is ignored for these
    /// nodes and everything else hides.
    pub filter: Option<HashSet<NodeId>>,
}

impl Default for View {
    fn default() -> Self {
        View { zoom: ROOT.to_owned(), show_checked: true, filter: None }
    }
}

fn checked(t: &Tree, id: &str) -> bool {
    t.get(id).and_then(|n| n.props.get("ck")).is_some_and(|v| !crate::tree::is_falsy(v))
}

pub fn is_hidden_by_check(t: &Tree, id: &str, v: &View) -> bool {
    !v.show_checked && checked(t, id)
}

/// Children shown under `id` (none while it is collapsed, except for the zoom root).
pub fn visible_children(t: &Tree, id: &str, v: &View) -> Vec<NodeId> {
    let Some(n) = t.get(id) else { return Vec::new() };
    if let Some(f) = &v.filter {
        return n.children.iter().filter(|c| f.contains(*c)).cloned().collect();
    }
    if n.collapsed && id != v.zoom {
        return Vec::new();
    }
    if v.show_checked {
        return n.children.clone();
    }
    n.children.iter().filter(|c| !checked(t, c)).cloned().collect()
}

pub fn has_visible_children(t: &Tree, id: &str, v: &View) -> bool {
    !visible_children(t, id, v).is_empty()
}

/// All visible items under the zoom root in document order (the zoom root excluded).
pub fn flatten(t: &Tree, v: &View) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = visible_children(t, &v.zoom, v).into_iter().rev().collect();
    while let Some(id) = stack.pop() {
        stack.extend(visible_children(t, &id, v).into_iter().rev());
        out.push(id);
    }
    out
}

fn position(sibs: &[NodeId], id: &str) -> Option<usize> {
    sibs.iter().position(|c| c == id)
}

/// Previous visible item in document order (the zoom root counts as the item before its first child).
pub fn prev_visible(t: &Tree, id: &str, v: &View) -> Option<NodeId> {
    let n = t.get(id)?;
    let parent = n.parent.as_ref()?;
    if id == v.zoom {
        return None;
    }
    let sibs = visible_children(t, parent, v);
    match position(&sibs, id) {
        Some(i) if i > 0 => Some(last_visible_descendant(t, &sibs[i - 1], v)),
        _ => Some(parent.clone()),
    }
}

pub fn next_visible(t: &Tree, id: &str, v: &View) -> Option<NodeId> {
    visible_children(t, id, v).into_iter().next().or_else(|| next_visible_non_descendant(t, id, v))
}

pub fn next_visible_non_descendant(t: &Tree, id: &str, v: &View) -> Option<NodeId> {
    let mut cur = id.to_owned();
    while cur != v.zoom {
        let parent = t.get(&cur)?.parent.clone()?;
        let sibs = visible_children(t, &parent, v);
        if let Some(i) = position(&sibs, &cur) {
            if i + 1 < sibs.len() {
                return Some(sibs[i + 1].clone());
            }
        }
        cur = parent;
    }
    None
}

pub fn last_visible_descendant(t: &Tree, id: &str, v: &View) -> NodeId {
    let mut cur = id.to_owned();
    loop {
        match visible_children(t, &cur, v).pop() {
            Some(last) => cur = last,
            None => return cur,
        }
    }
}

pub fn prev_visible_sibling(t: &Tree, id: &str, v: &View) -> Option<NodeId> {
    let parent = t.get(id)?.parent.as_ref()?;
    let sibs = visible_children(t, parent, v);
    let i = position(&sibs, id)?;
    (i > 0).then(|| sibs[i - 1].clone())
}

pub fn next_visible_sibling(t: &Tree, id: &str, v: &View) -> Option<NodeId> {
    let parent = t.get(id)?.parent.as_ref()?;
    let sibs = visible_children(t, parent, v);
    let i = position(&sibs, id)?;
    sibs.get(i + 1).cloned()
}

/// Depth relative to the zoom root (its children are depth 0).
pub fn depth(t: &Tree, id: &str, v: &View) -> usize {
    let mut d = 0;
    let mut n = t.get(id);
    while let Some(Some(p)) = n.map(|n| &n.parent) {
        if *p == v.zoom {
            break;
        }
        d += 1;
        n = t.get(p);
    }
    d
}

/// Normalize a block selection between `a` and `b` to contiguous visible siblings under their deepest common
/// ancestor (Dynalist's "select between").
pub fn select_between(t: &Tree, a: &str, b: &str, v: &View) -> Vec<NodeId> {
    if a == b {
        return vec![a.to_owned()];
    }
    let mut pa = t.ancestors(a);
    pa.push(a.to_owned());
    let mut pb = t.ancestors(b);
    pb.push(b.to_owned());
    let mut i = 0;
    while i < pa.len() && i < pb.len() && pa[i] == pb[i] {
        i += 1;
    }
    if i == pa.len() {
        return vec![a.to_owned()];
    }
    if i == pb.len() {
        return vec![b.to_owned()];
    }
    if i == 0 {
        return vec![a.to_owned()];
    }
    let sibs = visible_children(t, &pa[i - 1], v);
    let (Some(ix), Some(iy)) = (position(&sibs, &pa[i]), position(&sibs, &pb[i])) else {
        return vec![a.to_owned()];
    };
    sibs[ix.min(iy)..=ix.max(iy)].to_vec()
}

/// Sort ids into document order and drop ids that are descendants of other ids.
pub fn normalize_block(t: &Tree, ids: &[NodeId]) -> Vec<NodeId> {
    let set: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut order = HashMap::new();
    let mut stack = vec![ROOT.to_owned()];
    while let Some(id) = stack.pop() {
        if let Some(n) = t.get(&id) {
            stack.extend(n.children.iter().rev().cloned());
        }
        let k = order.len();
        order.insert(id, k);
    }
    let mut seen = HashSet::new();
    let mut top: Vec<NodeId> = ids
        .iter()
        .filter(|id| !t.ancestors(id).iter().any(|a| set.contains(a.as_str())))
        .filter(|id| seen.insert((*id).clone()))
        .cloned()
        .collect();
    top.sort_by_key(|id| order.get(id).copied().unwrap_or(0));
    top
}
