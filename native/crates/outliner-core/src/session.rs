//! One document on a client. Port of `packages/core/src/session.ts`.
//!
//! The live tree = confirmed (server state at `version`) + inflight + pending local ops. Undo history is local and
//! produces normal ops, so undo / redo sync like any other edit.
//!
//! Where the TS class calls listeners, the methods here return a [`Change`] describing what to redraw.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::tree::{NodeId, Op, SerializedTree, Tree};

const UNDO_TTL: i64 = 48 * 3600 * 1000;
const MERGE_WINDOW: i64 = 1000;
const PURGE_AFTER: i64 = 30 * 24 * 3600 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocSnapshot {
    pub version: u64,
    pub confirmed: SerializedTree,
    pub pending: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    pub base_version: u64,
    pub ops: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncResponse {
    /// Server version after our batch (or the current version if we sent nothing).
    pub version: u64,
    /// Ops committed by others between `base_version` and our batch, in order.
    pub remote: Vec<Op>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Local,
    Remote,
    Undo,
}

/// What changed: `touched = None` means "anything may have changed" (redraw everything).
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub kind: ChangeKind,
    pub touched: Option<HashSet<NodeId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Merge {
    id: NodeId,
    field: String,
    dir: i8,
}

#[derive(Debug, Clone)]
struct Txn<S> {
    /// Inverse ops, in the order they must be applied to undo.
    undo: Vec<Op>,
    sel_before: S,
    sel_after: S,
    /// Selection right after the entry's first edit (kept when later keystrokes coalesce into it).
    sel_first: Option<S>,
    ts: i64,
    merge: Option<Merge>,
}

fn system_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// A document session. `S` is the editor's selection snapshot stored with undo entries (opaque here).
pub struct DocSession<S: Clone> {
    pub live: Tree,
    confirmed: Tree,
    pub version: u64,
    pending: Vec<Op>,
    inflight: Option<Vec<Op>>,
    undo_stack: Vec<Txn<S>>,
    redo_stack: Vec<Txn<S>>,
    /// `sel_first` of the entry the last `undo()` replayed.
    pub undone_first: Option<S>,
    clock: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl<S: Clone> Default for DocSession<S> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<S: Clone> DocSession<S> {
    /// New docs always start from an empty tree; content (title included) arrives as ops.
    pub fn new(snap: Option<&DocSnapshot>) -> Self {
        let confirmed = snap.map_or_else(Tree::default, |s| Tree::from_serialized(&s.confirmed));
        let pending = snap.map_or_else(Vec::new, |s| s.pending.clone());
        let mut live = confirmed.clone();
        for op in &pending {
            live.apply(op);
        }
        DocSession {
            live,
            confirmed,
            version: snap.map_or(0, |s| s.version),
            pending,
            inflight: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undone_first: None,
            clock: Box::new(system_now),
        }
    }

    /// Replace the clock (milliseconds since the epoch) used for undo coalescing, expiry and stamps.
    pub fn set_clock(&mut self, clock: impl Fn() -> i64 + Send + Sync + 'static) {
        self.clock = Box::new(clock);
    }

    pub fn snapshot(&self) -> DocSnapshot {
        let mut pending = self.inflight.clone().unwrap_or_default();
        pending.extend(self.pending.iter().cloned());
        DocSnapshot { version: self.version, confirmed: self.confirmed.to_serialized(), pending }
    }

    pub fn dirty(&self) -> bool {
        !self.pending.is_empty() || self.inflight.is_some()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Apply ops as one undoable transaction. Returns the ops that took effect and what changed.
    pub fn transact(&mut self, ops: Vec<Op>, before: S, after: S, undoable: bool) -> (Vec<Op>, Option<Change>) {
        let mut applied = Vec::new();
        let mut inverse = Vec::new();
        for op in ops {
            if let Some(inv) = self.live.apply(&op) {
                applied.push(op);
                inverse.insert(0, inv);
            }
        }
        let change = self.commit(applied.clone(), inverse, before, after, undoable);
        (applied, change)
    }

    /// Record ops that were already applied to `live` (by a [`crate::Tx`]), with their inverses in undo order.
    pub fn commit(&mut self, ops: Vec<Op>, inverse: Vec<Op>, before: S, after: S, undoable: bool) -> Option<Change> {
        if ops.is_empty() {
            return None;
        }
        let mut touched = HashSet::new();
        for op in &ops {
            touched.insert(op.id().to_string());
            if let Op::Create { .. } | Op::Move { .. } = op {
                if let Some(p) = self.live.get(op.id()).and_then(|n| n.parent.clone()) {
                    touched.insert(p);
                }
                if let Op::Move { parent: Some(p), .. } = op {
                    touched.insert(p.clone());
                }
            }
        }
        for inv in &inverse {
            if let Op::Move { parent: Some(p), .. } = inv {
                touched.insert(p.clone());
            }
        }
        let inverse: Vec<Op> = inverse.into_iter().filter(|op| !matches!(op, Op::Collapse { .. })).collect();
        let merge = merge_info(&ops, &inverse);
        self.pending.extend(ops);
        if undoable && !inverse.is_empty() {
            let now = (self.clock)();
            match self.undo_stack.last_mut() {
                Some(top)
                    if merge.is_some() && top.merge.is_some() && now - top.ts < MERGE_WINDOW && top.merge == merge =>
                {
                    // keep the oldest "before" values: the original inverse stays, refresh time
                    top.ts = now;
                    top.sel_after = after;
                }
                _ => self.undo_stack.push(Txn {
                    undo: inverse,
                    sel_before: before,
                    sel_after: after.clone(),
                    sel_first: Some(after),
                    ts: now,
                    merge,
                }),
            }
            self.redo_stack.clear();
        }
        Some(Change { kind: ChangeKind::Local, touched: Some(touched) })
    }

    /// Undo the last transaction; returns the selection to restore (if any) and what changed.
    pub fn undo(&mut self) -> (Option<S>, Option<Change>) {
        self.replay(true)
    }

    pub fn redo(&mut self) -> (Option<S>, Option<Change>) {
        self.replay(false)
    }

    fn replay(&mut self, undo: bool) -> (Option<S>, Option<Change>) {
        let now = (self.clock)();
        let (from, to) =
            if undo { (&mut self.undo_stack, &mut self.redo_stack) } else { (&mut self.redo_stack, &mut self.undo_stack) };
        if from.last().is_some_and(|t| now - t.ts > UNDO_TTL) {
            from.clear();
        }
        let txn = from.pop();
        self.undone_first = if undo { txn.as_ref().and_then(|t| t.sel_first.clone()) } else { None };
        let Some(txn) = txn else { return (None, None) };
        let mut inverse = Vec::new();
        let mut touched = HashSet::new();
        for op in &txn.undo {
            let stamped = op.with_ts(now);
            let Some(inv) = self.live.apply(&stamped) else { continue };
            touch_op(&self.live, &stamped, &inv, &mut touched);
            self.pending.push(stamped);
            inverse.insert(0, inv);
        }
        let sel = if undo { txn.sel_before.clone() } else { txn.sel_after.clone() };
        to.push(Txn {
            undo: inverse,
            sel_before: txn.sel_before,
            sel_after: txn.sel_after,
            sel_first: None,
            ts: txn.ts,
            merge: None,
        });
        (Some(sel), Some(Change { kind: ChangeKind::Undo, touched: Some(touched) }))
    }

    // ---- sync -------------------------------------------------------------

    /// Take pending ops for sending. Returns `None` if a sync is already in flight.
    pub fn begin_sync(&mut self) -> Option<SyncRequest> {
        if self.inflight.is_some() {
            return None;
        }
        let ops = compact(&std::mem::take(&mut self.pending));
        self.inflight = Some(ops.clone());
        Some(SyncRequest { base_version: self.version, ops })
    }

    /// The server accepted our inflight batch (after the given remote ops).
    pub fn end_sync(&mut self, res: &SyncResponse) -> Option<Change> {
        let sent = self.inflight.take().unwrap_or_default();
        for op in res.remote.iter().chain(&sent) {
            self.confirmed.apply(op);
        }
        self.version = res.version;
        if res.remote.is_empty() {
            return None;
        }
        // Rebase: live = confirmed + still-pending local ops.
        self.rebuild_live();
        Some(Change { kind: ChangeKind::Remote, touched: None })
    }

    /// Network failure: put the batch back in front of newer pending ops.
    pub fn fail_sync(&mut self) {
        if let Some(mut batch) = self.inflight.take() {
            batch.append(&mut self.pending);
            self.pending = batch;
        }
    }

    /// The server says our state is unusable (e.g. history compacted): reload from its snapshot.
    pub fn reset(&mut self, confirmed: &SerializedTree, version: u64) -> Change {
        self.confirmed = Tree::from_serialized(confirmed);
        self.version = version;
        // The server did not apply the inflight batch: it becomes pending again and is re-sent against the new
        // base (otherwise begin_sync() would stay blocked).
        self.fail_sync();
        self.rebuild_live();
        Change { kind: ChangeKind::Remote, touched: None }
    }

    fn rebuild_live(&mut self) {
        self.live = self.confirmed.clone();
        for op in &self.pending {
            self.live.apply(op);
        }
    }

    /// Drop long-deleted nodes from the confirmed tree. Uses the server's horizon (not the 48 h undo TTL) so a
    /// late op from another client can't reference a node we already forgot.
    pub fn purge(&mut self, now: i64) {
        self.confirmed.purge_detached(now - PURGE_AFTER);
    }
}

fn touch_op(t: &Tree, op: &Op, inv: &Op, touched: &mut HashSet<NodeId>) {
    touched.insert(op.id().to_string());
    if let Op::Create { .. } | Op::Move { .. } = op {
        if let Some(p) = t.get(op.id()).and_then(|n| n.parent.clone()) {
            touched.insert(p);
        }
        if let Op::Move { parent: Some(p), .. } = inv {
            touched.insert(p.clone());
        }
    }
}

/// UTF-16 length, as JS `String.length` counts.
fn js_len(v: Option<&serde_json::Value>) -> i64 {
    match v {
        Some(serde_json::Value::String(s)) => s.encode_utf16().count() as i64,
        Some(serde_json::Value::Null) | None => 0,
        Some(other) => other.to_string().encode_utf16().count() as i64,
    }
}

/// A single-character edit of one text field: a candidate for typing coalescing.
fn merge_info(applied: &[Op], inverse: &[Op]) -> Option<Merge> {
    let ([Op::Set { id, props, .. }], [Op::Set { props: old, .. }]) = (applied, inverse) else { return None };
    if props.len() != 1 {
        return None;
    }
    let field = props.keys().next()?;
    if field != "c" && field != "n" {
        return None;
    }
    let d = js_len(props.get(field)) - js_len(old.get(field));
    (d == 1 || d == -1).then(|| Merge { id: id.clone(), field: field.clone(), dir: d as i8 })
}

/// Collapse runs of `set` ops on the same node into one (keystrokes).
pub fn compact(ops: &[Op]) -> Vec<Op> {
    let mut out: Vec<Op> = Vec::with_capacity(ops.len());
    for op in ops {
        if let (Op::Set { id, props, ts }, Some(Op::Set { id: last_id, props: last_props, ts: last_ts })) =
            (op, out.last_mut())
        {
            if id == last_id {
                for (k, v) in props {
                    last_props.insert(k.clone(), v.clone());
                }
                *last_ts = *ts;
                continue;
            }
        }
        out.push(op.clone());
    }
    out
}
