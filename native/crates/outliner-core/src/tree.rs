//! Deterministic outline tree and operations.
//!
//! Port of `packages/core/src/tree.ts`. Every replica (server, web, native) applies the same ops in the same
//! order with the same rules, so they converge on identical state. `apply` never panics: an op that is invalid
//! against the current state (missing node, cycle, ...) is dropped and reported as not applied (`None`).
//!
//! The JSON shape of [`Op`], [`TNode`] and [`SerializedTree`] is the wire format the TS server and web client
//! use, and [`Tree::canonical`] produces byte-identical output to the TS `canonical()`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type NodeId = String;
/// Item properties. `serde_json::Map` keeps keys sorted, which `canonical` relies on.
pub type Props = serde_json::Map<String, Value>;

pub const ROOT: &str = "root";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TNode {
    pub id: NodeId,
    /// `None` = root, or detached (deleted; kept so undo can reattach it).
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub props: Props,
    pub collapsed: bool,
    pub created: i64,
    pub edited: i64,
    /// Last attach/detach time (purging deleted nodes goes by this).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<i64>,
}

/// `after` anchors a position among the parent's children: insert right after that sibling, `None` = first
/// child. A stale anchor (no longer a child of the parent) falls back to appending at the end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Op {
    Create { id: NodeId, parent: NodeId, after: Option<NodeId>, props: Props, ts: i64 },
    /// Shallow-merge props; a `null` (or other falsy) value deletes the key.
    Set { id: NodeId, props: Props, ts: i64 },
    /// `parent: None` = detach (delete).
    Move { id: NodeId, parent: Option<NodeId>, after: Option<NodeId>, ts: i64 },
    Collapse { id: NodeId, v: bool },
}

impl Op {
    pub fn id(&self) -> &str {
        match self {
            Op::Create { id, .. } | Op::Set { id, .. } | Op::Move { id, .. } | Op::Collapse { id, .. } => id,
        }
    }

    /// The same op with its timestamp replaced (collapse ops carry none).
    pub fn with_ts(&self, now: i64) -> Op {
        let mut op = self.clone();
        match &mut op {
            Op::Create { ts, .. } | Op::Set { ts, .. } | Op::Move { ts, .. } => *ts = now,
            Op::Collapse { .. } => {}
        }
        op
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerializedTree {
    pub nodes: Vec<TNode>,
}

/// A value that means "unset": every replica stores the same shape by dropping these.
pub fn is_falsy(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::String(s) => s.is_empty(),
        Value::Number(n) => n.as_f64() == Some(0.0),
        _ => false,
    }
}

/// Strict equality of two prop values as JS `===` sees primitives (`1` and `1.0` are the same number).
fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(_), _) | (Value::Object(_), _) => false, // objects compare by reference in JS
        _ => a == b,
    }
}

fn clean_props(p: &Props) -> Props {
    p.iter().filter(|(_, v)| !is_falsy(v)).map(|(k, v)| (k.clone(), v.clone())).collect()
}

#[derive(Debug, Clone)]
pub struct Tree {
    nodes: HashMap<NodeId, TNode>,
    /// Stamp of the last children change per node (see `kids_rev`); not serialized.
    revs: HashMap<NodeId, u64>,
    /// Strictly increasing: a stamp is never reused.
    stamp: u64,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new(Props::new(), 0)
    }
}

impl Tree {
    pub fn new(root_props: Props, ts: i64) -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(
            ROOT.to_string(),
            TNode {
                id: ROOT.to_string(),
                parent: None,
                children: Vec::new(),
                props: root_props,
                collapsed: false,
                created: ts,
                edited: ts,
                moved: None,
            },
        );
        Tree { nodes, revs: HashMap::new(), stamp: 0 }
    }

    pub fn from_serialized(s: &SerializedTree) -> Self {
        let nodes = s.nodes.iter().map(|n| (n.id.clone(), n.clone())).collect();
        Tree { nodes, revs: HashMap::new(), stamp: 0 }
    }

    /// Nodes in a stable order: attached ones breadth-first from the root, then detached ones by id.
    pub fn to_serialized(&self) -> SerializedTree {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut seen = std::collections::HashSet::with_capacity(self.nodes.len());
        let mut queue = std::collections::VecDeque::from([ROOT]);
        while let Some(id) = queue.pop_front() {
            if let Some(n) = self.nodes.get(id) {
                if seen.insert(id) {
                    out.push(n.clone());
                    queue.extend(n.children.iter().map(String::as_str));
                }
            }
        }
        let mut rest: Vec<&TNode> = self.nodes.values().filter(|n| !seen.contains(n.id.as_str())).collect();
        rest.sort_by(|a, b| a.id.cmp(&b.id));
        out.extend(rest.into_iter().cloned());
        SerializedTree { nodes: out }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&TNode> {
        self.nodes.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &TNode> {
        self.nodes.values()
    }

    pub fn root(&self) -> &TNode {
        &self.nodes[ROOT]
    }

    /// True if `id` is reachable from the root (i.e. not deleted).
    pub fn is_attached(&self, id: &str) -> bool {
        let mut n = self.nodes.get(id);
        while let Some(node) = n {
            if node.id == ROOT {
                return true;
            }
            match &node.parent {
                None => return false,
                Some(p) => n = self.nodes.get(p),
            }
        }
        false
    }

    pub fn is_ancestor(&self, anc: &str, id: &str) -> bool {
        let mut n = self.nodes.get(id);
        while let Some(Some(p)) = n.map(|n| &n.parent) {
            if p == anc {
                return true;
            }
            n = self.nodes.get(p);
        }
        false
    }

    fn siblings(&self, id: &str) -> Option<&[NodeId]> {
        let p = self.nodes.get(id)?.parent.as_ref()?;
        Some(&self.nodes.get(p)?.children)
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.siblings(id)?.iter().position(|c| c == id)
    }

    pub fn prev_sibling(&self, id: &str) -> Option<&NodeId> {
        let sibs = self.siblings(id)?;
        let i = sibs.iter().position(|c| c == id)?;
        if i > 0 { sibs.get(i - 1) } else { None }
    }

    pub fn next_sibling(&self, id: &str) -> Option<&NodeId> {
        let sibs = self.siblings(id)?;
        let i = sibs.iter().position(|c| c == id)?;
        sibs.get(i + 1)
    }

    /// Ancestors of `id`, root first.
    pub fn ancestors(&self, id: &str) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut n = self.nodes.get(id);
        while let Some(Some(p)) = n.map(|n| &n.parent) {
            out.push(p.clone());
            n = self.nodes.get(p);
        }
        out.reverse();
        out
    }

    /// Anchor describing the current position of `id` (for inverse moves): `(parent, after)`.
    pub fn anchor_of(&self, id: &str) -> (Option<NodeId>, Option<NodeId>) {
        match self.nodes.get(id) {
            Some(n) if n.parent.is_some() => (n.parent.clone(), self.prev_sibling(id).cloned()),
            _ => (None, None),
        }
    }

    /// Anchor for inserting at `index` in `parent`'s children.
    pub fn anchor_at(&self, parent: &str, index: usize) -> Option<NodeId> {
        let p = self.nodes.get(parent)?;
        if index == 0 || p.children.is_empty() {
            return None;
        }
        p.children.get(index.min(p.children.len()) - 1).cloned()
    }

    /// A stamp that changes whenever the children list of `id` changes, and with the collapsed / numbered flags
    /// (which the children block shows too). Lets a renderer tell in O(1) whether what it drew is still current.
    pub fn kids_rev(&self, id: &str) -> u64 {
        let Some(n) = self.nodes.get(id) else { return 0 };
        let num = n.props.get("num").is_some_and(|v| !is_falsy(v));
        self.revs.get(id).copied().unwrap_or(0) * 4 + u64::from(n.collapsed) + if num { 2 } else { 0 }
    }

    fn bump_rev(&mut self, id: &str) {
        self.stamp += 1;
        self.revs.insert(id.to_string(), self.stamp);
    }

    fn insert(&mut self, parent: &str, id: &str, after: Option<&str>) {
        self.bump_rev(parent);
        let ch = &mut self.nodes.get_mut(parent).expect("parent exists").children;
        match after {
            None => ch.insert(0, id.to_string()),
            // appending is the common case (loading a document, typing down a list): skip the linear search
            Some(a) if ch.last().is_some_and(|l| l == a) => ch.push(id.to_string()),
            Some(a) => match ch.iter().position(|c| c == a) {
                Some(i) => ch.insert(i + 1, id.to_string()),
                None => ch.push(id.to_string()),
            },
        }
    }

    fn detach(&mut self, id: &str) {
        let Some(parent) = self.nodes.get(id).and_then(|n| n.parent.clone()) else { return };
        if let Some(p) = self.nodes.get_mut(&parent) {
            if let Some(i) = p.children.iter().position(|c| c == id) {
                p.children.remove(i);
            }
            self.bump_rev(&parent);
        }
        self.nodes.get_mut(id).expect("node exists").parent = None;
    }

    /// Apply an op. Returns the inverse op (for undo), or `None` if the op was dropped as invalid or a no-op.
    pub fn apply(&mut self, op: &Op) -> Option<Op> {
        match op {
            Op::Create { id, parent, after, props, ts } => {
                if self.nodes.contains_key(id) || !self.nodes.contains_key(parent) {
                    return None;
                }
                self.nodes.insert(
                    id.clone(),
                    TNode {
                        id: id.clone(),
                        parent: Some(parent.clone()),
                        children: Vec::new(),
                        props: clean_props(props),
                        collapsed: false,
                        created: *ts,
                        edited: *ts,
                        moved: Some(*ts),
                    },
                );
                self.insert(parent, id, after.as_deref());
                Some(Op::Move { id: id.clone(), parent: None, after: None, ts: *ts })
            }
            Op::Set { id, props, ts } => {
                let n = self.nodes.get_mut(id)?;
                let mut old = Props::new();
                let mut changed = false;
                for (k, v) in props {
                    let prev = n.props.get(k);
                    if is_falsy(v) {
                        if let Some(prev) = n.props.remove(k) {
                            old.insert(k.clone(), prev);
                            changed = true;
                        }
                    } else if !prev.is_some_and(|p| same_value(p, v)) {
                        old.insert(k.clone(), prev.cloned().unwrap_or(Value::Null));
                        n.props.insert(k.clone(), v.clone());
                        changed = true;
                    }
                }
                if !changed {
                    return None;
                }
                n.edited = *ts;
                Some(Op::Set { id: id.clone(), props: old, ts: *ts })
            }
            Op::Move { id, parent, after, ts } => {
                if id == ROOT || !self.nodes.contains_key(id) {
                    return None;
                }
                let (before_parent, before_after) = self.anchor_of(id);
                match parent {
                    None => {
                        before_parent.as_ref()?;
                        self.detach(id);
                    }
                    Some(p) => {
                        if !self.nodes.contains_key(p) || p == id || self.is_ancestor(id, p) {
                            return None;
                        }
                        if after.as_deref() == Some(id.as_str()) {
                            return None;
                        }
                        if before_parent.as_deref() == Some(p.as_str()) && before_after == *after {
                            return None;
                        }
                        self.detach(id);
                        self.nodes.get_mut(id).expect("node exists").parent = Some(p.clone());
                        self.insert(p, id, after.as_deref());
                    }
                }
                self.nodes.get_mut(id).expect("node exists").moved = Some(*ts);
                Some(Op::Move { id: id.clone(), parent: before_parent, after: before_after, ts: *ts })
            }
            Op::Collapse { id, v } => {
                let n = self.nodes.get_mut(id)?;
                if n.collapsed == *v {
                    return None;
                }
                n.collapsed = *v;
                Some(Op::Collapse { id: id.clone(), v: !v })
            }
        }
    }

    /// Drop detached subtrees whose top was last edited / moved before `cutoff`.
    pub fn purge_detached(&mut self, cutoff: i64) {
        let tops: Vec<NodeId> = self
            .nodes
            .values()
            .filter(|n| n.id != ROOT && n.parent.is_none() && n.edited.max(n.moved.unwrap_or(0)) < cutoff)
            .map(|n| n.id.clone())
            .collect();
        for top in tops {
            let mut stack = vec![top];
            while let Some(id) = stack.pop() {
                if let Some(c) = self.nodes.remove(&id) {
                    stack.extend(c.children);
                }
            }
        }
    }

    /// Stable content hash input (attached nodes only, breadth-first) for divergence checks. Byte-identical to
    /// the TS `canonical()`.
    pub fn canonical(&self) -> String {
        let mut out = Vec::new();
        let mut queue = std::collections::VecDeque::from([ROOT]);
        while let Some(id) = queue.pop_front() {
            let n = &self.nodes[id];
            out.push(Value::Array(vec![
                Value::String(n.id.clone()),
                Value::Array(n.children.iter().cloned().map(Value::String).collect()),
                Value::Object(n.props.clone()),
            ]));
            queue.extend(n.children.iter().map(String::as_str));
        }
        serde_json::to_string(&out).expect("serializable")
    }
}
