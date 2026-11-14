//! Restoring an older version in place. Port of `packages/core/src/history.ts`.
//!
//! The ops that turn the live tree into `target`, applied as one transaction (so the restore is undoable and
//! syncs like any edit; nothing is lost, only detached).

use std::collections::HashSet;

use serde_json::Value;

use crate::tree::{NodeId, Props, Tree, ROOT};
use crate::tx::Tx;

fn props_patch(cur: &Props, want: &Props) -> Option<Props> {
    let mut patch = Props::new();
    for k in cur.keys() {
        if !want.contains_key(k) {
            patch.insert(k.clone(), Value::Null);
        }
    }
    for (k, v) in want {
        if cur.get(k) != Some(v) {
            patch.insert(k.clone(), v.clone());
        }
    }
    (!patch.is_empty()).then_some(patch)
}

pub fn restore_tree(tx: &mut Tx<'_>, target: &Tree) {
    let mut keep = HashSet::new();
    sync(tx, target, ROOT, &mut keep);
    prune(tx, ROOT, &keep);
}

fn sync(tx: &mut Tx<'_>, target: &Tree, id: &str, keep: &mut HashSet<NodeId>) {
    let want = target.get(id).expect("target node");
    let cur = tx.tree.get(id).expect("live node");
    let collapsed = cur.collapsed;
    if let Some(patch) = props_patch(&cur.props, &want.props) {
        tx.set(id, patch);
    }
    if collapsed != want.collapsed {
        tx.collapse(id, want.collapsed);
    }
    let mut prev: Option<NodeId> = None;
    for c in &want.children {
        keep.insert(c.clone());
        match tx.tree.get(c) {
            None => {
                let props = target.get(c).expect("target child").props.clone();
                tx.create_with_id(id, prev.as_deref(), props, c.clone());
            }
            Some(n) => {
                if n.parent.as_deref() != Some(id) || tx.tree.prev_sibling(c) != prev.as_ref() {
                    tx.move_to(c, Some(id), prev.as_deref());
                }
            }
        }
        sync(tx, target, c, keep);
        prev = Some(c.clone());
    }
}

fn prune(tx: &mut Tx<'_>, id: &str, keep: &HashSet<NodeId>) {
    let children = tx.tree.get(id).expect("live node").children.clone();
    for c in children {
        if keep.contains(&c) {
            prune(tx, &c, keep);
        } else {
            tx.move_to(&c, None, None);
        }
    }
}
