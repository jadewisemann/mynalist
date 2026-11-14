//! Editor commands. Port of `packages/core/src/commands.ts`.
//!
//! Each command runs against a [`Tx`] (ops applied to the live tree at once, recorded for sync and undo) and
//! returns the selection to show afterwards, or `None` when it does nothing. Text offsets are UTF-16 units.
//!
//! Not yet ported (they need `dates.ts` and `inline.ts`): advancing a recurring date when an item is checked
//! (`CheckOptions::recurring`) and the date sort modes, which take the dates through [`DateFn`] meanwhile.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::item::meta_of;
use crate::text16;
use crate::tree::{is_falsy, NodeId, Props, Tree, ROOT};
use crate::tx::Tx;
use crate::view::{
    next_visible, next_visible_non_descendant, next_visible_sibling, prev_visible, prev_visible_sibling,
    visible_children, View,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Field {
    #[default]
    #[serde(rename = "c")]
    Content,
    #[serde(rename = "n")]
    Note,
}

/// The editor's selection, in the TS JSON shape (`{ k: 'text', id, f, s, e }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "lowercase")]
pub enum Sel {
    None,
    Text { id: NodeId, f: Field, s: usize, e: usize },
    Block { ids: Vec<NodeId>, anchor: NodeId, focus: NodeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CmdResult {
    pub sel: Sel,
    /// Change the zoom (e.g. outdenting out of the zoom root).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoom: Option<NodeId>,
}

impl CmdResult {
    fn sel(sel: Sel) -> Self {
        CmdResult { sel, zoom: None }
    }
}

pub fn caret(id: &str, s: usize) -> Sel {
    Sel::Text { id: id.to_owned(), f: Field::Content, s, e: s }
}

fn text(t: &Tree, id: &str) -> String {
    t.get(id).and_then(|n| n.props.get("c")).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn note(t: &Tree, id: &str) -> String {
    t.get(id).and_then(|n| n.props.get("n")).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn prop_true(t: &Tree, id: &str, key: &str) -> bool {
    t.get(id).and_then(|n| n.props.get(key)).is_some_and(|v| !is_falsy(v))
}

fn children(t: &Tree, id: &str) -> Vec<NodeId> {
    t.get(id).map(|n| n.children.clone()).unwrap_or_default()
}

fn parent(t: &Tree, id: &str) -> Option<NodeId> {
    t.get(id).and_then(|n| n.parent.clone())
}

fn props(pairs: &[(&str, Value)]) -> Props {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect()
}

fn str_props(pairs: &[(&str, &str)]) -> Props {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), Value::String((*v).to_owned()))).collect()
}

/// Replace all non-text props of `id` with `meta`.
fn set_meta(tx: &mut Tx<'_>, id: &str, meta: &Props) {
    let cur = meta_of(&tx.tree.get(id).expect("item").props);
    let mut patch = Props::new();
    for k in cur.keys() {
        if !meta.contains_key(k) {
            patch.insert(k.clone(), Value::Null);
        }
    }
    for (k, v) in meta {
        patch.insert(k.clone(), v.clone());
    }
    tx.set(id, patch);
}

// ---- Enter / Backspace / Delete ------------------------------------------

pub fn enter(tx: &mut Tx<'_>, v: &View, id: &str, s: usize, e: usize) -> Option<CmdResult> {
    let n = tx.tree.get(id)?;
    let parent = n.parent.clone();
    let str = text(tx.tree, id);
    let cb = if prop_true(tx.tree, id, "cb") { props(&[("cb", Value::Bool(true))]) } else { Props::new() };

    if str.is_empty()
        && id != ROOT
        && id != v.zoom
        && parent.as_deref() != Some(v.zoom.as_str())
        && next_visible_sibling(tx.tree, id, v).is_none()
    {
        return outdent(tx, v, &[id.to_owned()], caret(id, 0), false);
    }
    let Some(parent) = parent.filter(|_| id != v.zoom) else {
        // the title: its head stays, the tail becomes its first child
        let tail = text16::slice_from(&str, e).to_owned();
        if !tail.is_empty() {
            tx.set(id, str_props(&[("c", text16::slice(&str, 0, s))]));
        }
        let nid = tx.create(id, None, str_props(&[("c", &tail)]));
        return Some(CmdResult::sel(caret(&nid, 0)));
    };
    if s == e && s >= text16::len(&str) {
        let as_child = !visible_children(tx.tree, id, v).is_empty();
        let nid = if as_child { tx.create(id, None, cb) } else { tx.create(&parent, Some(id), cb) };
        return Some(CmdResult::sel(caret(&nid, 0)));
    }
    let head = text16::slice(&str, 0, s).to_owned();
    let tail = text16::slice_from(&str, e).to_owned();
    let prev = tx.tree.prev_sibling(id).cloned();
    let mut p = cb;
    p.insert("c".into(), Value::String(head));
    let nid = tx.create(&parent, prev.as_deref(), p);
    tx.set(id, str_props(&[("c", &tail)]));
    Some(CmdResult::sel(if s == 0 { caret(&nid, 0) } else { caret(id, 0) }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    A,
    B,
}

/// Join `b`'s text onto `a`'s. The item with children survives (in its place); when neither has any, `keep`
/// decides: Backspace keeps `a` (the merged item stays where the caret was), Delete keeps `b` (Dynalist).
/// Returns the survivor and the caret offset at the join.
pub fn merge(tx: &mut Tx<'_>, a: &str, b: &str, keep: Keep) -> Option<(NodeId, usize)> {
    let t = &*tx.tree;
    let na = t.get(a)?;
    let nb = t.get(b)?;
    let a_kids = na.children.iter().any(|c| c != b);
    let b_kids = !nb.children.is_empty();
    if a_kids && b_kids {
        return None;
    }
    // b is a's first child and has children of its own: b can't survive (it would go down with a), and a can't
    // take b's children without reordering them, so nothing happens (Dynalist)
    let b_under_a = nb.parent.as_deref() == Some(a);
    if b_kids && b_under_a {
        return None;
    }
    let (ta, tb) = (text(t, a), text(t, b));
    let (ma, mb) = (meta_of(&na.props), meta_of(&nb.props));
    let meta = if !ta.is_empty() && tb.is_empty() {
        ma
    } else if !tb.is_empty() && ta.is_empty() {
        mb
    } else {
        let mut m = ma;
        m.extend(mb);
        m
    };
    let (note_a, note_b) = (note(t, a), note(t, b));
    let survivor = if b_under_a || a_kids {
        a
    } else if b_kids || keep == Keep::B {
        b
    } else {
        a
    };
    let victim = if survivor == a { b } else { a };
    let n = if survivor == a {
        if note_a.is_empty() { note_b } else { note_a }
    } else if note_b.is_empty() {
        note_a
    } else {
        note_b
    };
    let at = text16::len(&ta);
    tx.move_to(victim, None, None);
    tx.set(survivor, str_props(&[("c", &(ta + &tb)), ("n", &n)]));
    set_meta(tx, survivor, &meta);
    Some((survivor.to_owned(), at))
}

pub fn backspace_at_start(tx: &mut Tx<'_>, v: &View, id: &str) -> Option<CmdResult> {
    let n = tx.tree.get(id)?;
    if id == v.zoom || n.parent.is_none() {
        return None;
    }
    let has_kids = !n.children.is_empty();
    let prev = prev_visible(tx.tree, id, v);
    if prev.as_deref() == Some(v.zoom.as_str()) {
        // a childless, note-less first item merges into the title (Dynalist); the title keeps its own props
        let prev = prev.expect("the zoom root");
        if has_kids || prop_true(tx.tree, id, "n") {
            return None;
        }
        let title = text(tx.tree, &prev);
        let at = text16::len(&title);
        let own = text(tx.tree, id);
        tx.move_to(id, None, None);
        tx.set(&prev, str_props(&[("c", &(title + &own))]));
        return Some(CmdResult::sel(caret(&prev, at)));
    }
    if let (false, Some(prev)) = (has_kids, &prev) {
        return merge(tx, prev, id, Keep::A).map(|(s, at)| CmdResult::sel(caret(&s, at)));
    }
    let ps = prev_visible_sibling(tx.tree, id, v)?;
    merge(tx, &ps, id, Keep::A).map(|(s, at)| CmdResult::sel(caret(&s, at)))
}

pub fn delete_at_end(tx: &mut Tx<'_>, v: &View, id: &str) -> Option<CmdResult> {
    let next = next_visible(tx.tree, id, v)?;
    if id != v.zoom && text(tx.tree, id).is_empty() && children(tx.tree, id).is_empty() {
        // an empty item goes away instead of pulling the next one in (which keeps its own place, as Dynalist)
        tx.move_to(id, None, None);
        return Some(CmdResult::sel(caret(&next, 0)));
    }
    merge(tx, id, &next, Keep::B).map(|(s, at)| CmdResult::sel(caret(&s, at)))
}

// ---- structure -----------------------------------------------------------

pub fn indent(tx: &mut Tx<'_>, v: &View, ids: &[NodeId], sel: Sel) -> Option<CmdResult> {
    let prev = prev_visible_sibling(tx.tree, ids.first()?, v)?;
    tx.collapse(&prev, false);
    for id in ids {
        let last = children(tx.tree, &prev).last().cloned();
        tx.move_to(id, Some(&prev), last.as_deref());
    }
    Some(CmdResult::sel(sel))
}

/// Unindent: the items become siblings right after their parent. `in_place` (Workflowy-style): the siblings
/// that followed them become children of the last one, so the visual order stays.
pub fn outdent(tx: &mut Tx<'_>, v: &View, ids: &[NodeId], sel: Sel, in_place: bool) -> Option<CmdResult> {
    let p = parent(tx.tree, ids.first()?)?;
    let gp = parent(tx.tree, &p)?;
    if in_place {
        let last = ids.last()?.clone();
        let sibs = children(tx.tree, &p);
        let from = sibs.iter().position(|c| *c == last).map_or(0, |i| i + 1);
        let following: Vec<NodeId> = sibs[from..].iter().filter(|x| !ids.contains(x)).cloned().collect();
        let mut anchor = children(tx.tree, &last).last().cloned();
        for f in &following {
            tx.move_to(f, Some(&last), anchor.as_deref());
            anchor = Some(f.clone());
        }
        if !following.is_empty() && tx.tree.get(&last).is_some_and(|n| n.collapsed) {
            tx.collapse(&last, false);
        }
    }
    let mut after = p.clone();
    for id in ids {
        tx.move_to(id, Some(&gp), Some(&after));
        after = id.clone();
    }
    Some(CmdResult { sel, zoom: (p == v.zoom).then_some(gp) })
}

fn move_all(tx: &mut Tx<'_>, ids: &[NodeId], target: &str, mut after: Option<NodeId>) {
    for id in ids {
        tx.move_to(id, Some(target), after.as_deref());
        after = Some(id.clone());
    }
}

pub fn move_up(tx: &mut Tx<'_>, v: &View, ids: &[NodeId], sel: Sel) -> Option<CmdResult> {
    let first = ids.first()?;
    let p = parent(tx.tree, first)?;
    let (target, after) = match prev_visible_sibling(tx.tree, first, v) {
        Some(prev) => (p, tx.tree.prev_sibling(&prev).cloned()),
        None => {
            if p == v.zoom {
                return None;
            }
            let pp = prev_visible_sibling(tx.tree, &p, v);
            let gp = parent(tx.tree, &p)?;
            // into the parent's previous sibling (also a childless one), unless it's collapsed (Dynalist)
            match pp.filter(|pp| {
                !visible_children(tx.tree, pp, v).is_empty() || children(tx.tree, pp).is_empty()
            }) {
                Some(pp) => {
                    let last = children(tx.tree, &pp).last().cloned();
                    (pp, last)
                }
                None => (gp, tx.tree.prev_sibling(&p).cloned()),
            }
        }
    };
    move_all(tx, ids, &target, after);
    (!tx.ops.is_empty()).then(|| CmdResult::sel(sel))
}

pub fn move_down(tx: &mut Tx<'_>, v: &View, ids: &[NodeId], sel: Sel) -> Option<CmdResult> {
    let last = ids.last()?;
    let p = parent(tx.tree, last)?;
    let (target, after) = match next_visible_sibling(tx.tree, last, v) {
        Some(next) => (p, Some(next)),
        None => {
            if p == v.zoom {
                return None;
            }
            let pn = next_visible_sibling(tx.tree, &p, v);
            let gp = parent(tx.tree, &p)?;
            match pn.filter(|pn| {
                !visible_children(tx.tree, pn, v).is_empty() || children(tx.tree, pn).is_empty()
            }) {
                Some(pn) => (pn, None),
                None => (gp, Some(p)),
            }
        }
    };
    move_all(tx, ids, &target, after);
    (!tx.ops.is_empty()).then(|| CmdResult::sel(sel))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dir {
    #[default]
    Left,
    Right,
}

pub fn delete_items(tx: &mut Tx<'_>, v: &View, ids: &[NodeId], dir: Dir) -> Option<CmdResult> {
    if ids.iter().any(|id| *id == v.zoom || id == ROOT) {
        return None;
    }
    let prev = prev_visible(tx.tree, ids.first()?, v);
    let before = prev.filter(|p| *p != v.zoom); // the title never takes the caret here
    let after = next_visible_non_descendant(tx.tree, ids.last()?, v);
    for id in ids {
        tx.move_to(id, None, None);
    }
    if let Some(b) = before.as_ref().filter(|_| dir == Dir::Left || after.is_none()) {
        return Some(CmdResult::sel(caret(b, text16::len(&text(tx.tree, b)))));
    }
    if let Some(a) = after {
        return Some(CmdResult::sel(caret(&a, 0)));
    }
    Some(CmdResult::sel(Sel::None))
}

/// Block Enter: replace the selection with one empty item.
pub fn replace_with_empty(tx: &mut Tx<'_>, _v: &View, ids: &[NodeId]) -> Option<CmdResult> {
    let first = ids.first()?;
    let p = parent(tx.tree, first)?;
    let prev = tx.tree.prev_sibling(first).cloned();
    let nid = tx.create(&p, prev.as_deref(), Props::new());
    for id in ids {
        tx.move_to(id, None, None);
    }
    Some(CmdResult::sel(caret(&nid, 0)))
}

// ---- item-level toggles ----------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    /// Keep the selection on the item instead of moving on to the next one (Dynalist's mobile toolbar).
    pub stay: bool,
}

pub fn toggle_checked(tx: &mut Tx<'_>, v: &View, ids: &[NodeId], sel: Sel, opts: &CheckOptions) -> Option<CmdResult> {
    let targets: Vec<NodeId> = ids.iter().filter(|id| *id != ROOT).cloned().collect();
    let last = targets.last()?.clone();
    let all = targets.iter().all(|id| prop_true(tx.tree, id, "ck"));
    for id in &targets {
        tx.set(id, props(&[("ck", if all { Value::Null } else { Value::Bool(true) })]));
    }
    if !opts.stay && matches!(sel, Sel::Text { .. } | Sel::Block { .. }) {
        // checking or unchecking moves on to the start of the next item (Dynalist)
        if let Some(next) = next_visible_non_descendant(tx.tree, &last, v) {
            return Some(CmdResult::sel(caret(&next, 0)));
        }
    }
    Some(CmdResult::sel(sel))
}

pub fn toggle_prop(tx: &mut Tx<'_>, ids: &[NodeId], key: &str, sel: Sel) -> CmdResult {
    let all = ids.iter().all(|id| prop_true(tx.tree, id, key));
    for id in ids {
        tx.set(id, props(&[(key, if all { Value::Null } else { Value::Bool(true) })]));
    }
    CmdResult::sel(sel)
}

fn level(t: &Tree, id: &str, key: &str) -> f64 {
    t.get(id).and_then(|n| n.props.get(key)).and_then(Value::as_f64).unwrap_or(0.0)
}

fn level_value(v: i64) -> Value {
    if v == 0 { Value::Null } else { json!(v) }
}

/// Set heading / color to `value`; if every item already has it, clear instead.
pub fn set_level(tx: &mut Tx<'_>, ids: &[NodeId], key: &str, value: i64, sel: Sel) -> CmdResult {
    let all = ids.iter().all(|id| level(tx.tree, id, key) == value as f64);
    for id in ids {
        tx.set(id, props(&[(key, if all { Value::Null } else { level_value(value) })]));
    }
    CmdResult::sel(sel)
}

pub fn cycle_level(tx: &mut Tx<'_>, ids: &[NodeId], key: &str, max: i64, sel: Sel) -> CmdResult {
    let cur = ids.first().map_or(0, |id| level(tx.tree, id, key) as i64);
    let next = (cur + 1) % (max + 1);
    for id in ids {
        tx.set(id, props(&[(key, level_value(next))]));
    }
    CmdResult::sel(sel)
}

pub fn uncheck_descendants(tx: &mut Tx<'_>, ids: &[NodeId], sel: Sel) -> CmdResult {
    fn walk(tx: &mut Tx<'_>, id: &str) {
        tx.set(id, props(&[("ck", Value::Null)]));
        for c in children(tx.tree, id) {
            walk(tx, &c);
        }
    }
    for id in ids {
        walk(tx, id);
    }
    CmdResult::sel(sel)
}

pub fn set_children_checkbox(tx: &mut Tx<'_>, ids: &[NodeId], sel: Sel) -> CmdResult {
    let kids: Vec<NodeId> = ids.iter().flat_map(|id| children(tx.tree, id)).collect();
    let all = !kids.is_empty() && kids.iter().all(|c| prop_true(tx.tree, c, "cb"));
    fn walk(tx: &mut Tx<'_>, id: &str, all: bool) {
        for c in children(tx.tree, id) {
            tx.set(&c, props(&[("cb", if all { Value::Null } else { Value::Bool(true) })]));
            walk(tx, &c, all);
        }
    }
    for id in ids {
        walk(tx, id, all);
    }
    CmdResult::sel(sel)
}

/// Collapse / expand. Not undoable (collapse ops are left out of undo).
pub fn toggle_collapse(tx: &mut Tx<'_>, v: &View, ids: &[NodeId], recursive: bool) {
    let targets: Vec<NodeId> =
        ids.iter().flat_map(|id| if *id == v.zoom { children(tx.tree, id) } else { vec![id.clone()] }).collect();
    let with_kids: Vec<NodeId> = targets.into_iter().filter(|id| !children(tx.tree, id).is_empty()).collect();
    if with_kids.is_empty() {
        return;
    }
    let collapse = !with_kids.iter().all(|id| tx.tree.get(id).is_some_and(|n| n.collapsed));
    fn walk(tx: &mut Tx<'_>, id: &str, collapse: bool, recursive: bool) {
        let kids = children(tx.tree, id);
        if kids.is_empty() {
            return;
        }
        tx.collapse(id, collapse);
        if recursive {
            for c in kids {
                walk(tx, &c, collapse, recursive);
            }
        }
    }
    for id in &with_kids {
        walk(tx, id, collapse, recursive);
    }
}

pub fn expand_to_level(tx: &mut Tx<'_>, ids: &[NodeId], level: usize) {
    fn walk(tx: &mut Tx<'_>, id: &str, d: usize, level: usize) {
        let kids = children(tx.tree, id);
        if kids.is_empty() {
            return;
        }
        tx.collapse(id, d >= level);
        for c in kids {
            walk(tx, &c, d + 1, level);
        }
    }
    for id in ids {
        walk(tx, id, 0, level);
    }
}

pub fn toggle_siblings(tx: &mut Tx<'_>, id: &str) {
    let Some(p) = parent(tx.tree, id) else { return };
    let sibs: Vec<NodeId> = children(tx.tree, &p).into_iter().filter(|c| !children(tx.tree, c).is_empty()).collect();
    let collapse = !sibs.iter().all(|c| tx.tree.get(c).is_some_and(|n| n.collapsed));
    for c in &sibs {
        tx.collapse(c, collapse);
    }
}

// ---- copy / insert ---------------------------------------------------------

/// A portable item tree (clipboard, import, duplicate).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Portable {
    pub props: Props,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    pub children: Vec<Portable>,
}

pub fn to_portable(t: &Tree, id: &str, v: Option<&View>) -> Portable {
    let n = t.get(id).expect("item");
    let kids = match v {
        Some(v) => visible_children(t, id, &View { zoom: String::new(), ..v.clone() }),
        None => n.children.clone(),
    };
    Portable {
        props: n.props.clone(),
        collapsed: Some(n.collapsed),
        children: kids.iter().map(|c| to_portable(t, c, v)).collect(),
    }
}

/// Insert items as siblings after `after` under `parent`; returns the new top-level ids.
pub fn insert_portable(tx: &mut Tx<'_>, parent: &str, after: Option<&str>, items: &[Portable]) -> Vec<NodeId> {
    insert_deep(tx, parent, after, items).0
}

/// Like `insert_portable`, also reporting the last created item in document order.
fn insert_deep(tx: &mut Tx<'_>, parent: &str, after: Option<&str>, items: &[Portable]) -> (Vec<NodeId>, Option<NodeId>) {
    let mut ids = Vec::new();
    let mut last = None;
    let mut anchor = after.map(str::to_owned);
    for it in items {
        let id = tx.create(parent, anchor.as_deref(), it.props.clone());
        let (_, sub_last) = insert_deep(tx, &id, None, &it.children);
        if it.collapsed == Some(true) && !it.children.is_empty() {
            tx.collapse(&id, true);
        }
        last = Some(sub_last.unwrap_or_else(|| id.clone()));
        anchor = Some(id.clone());
        ids.push(id);
    }
    (ids, last)
}

pub fn duplicate(tx: &mut Tx<'_>, ids: &[NodeId]) -> Option<CmdResult> {
    let last = ids.last()?;
    let p = parent(tx.tree, last)?;
    let copies: Vec<Portable> = ids.iter().map(|id| to_portable(tx.tree, id, None)).collect();
    let nids = insert_portable(tx, &p, Some(last), &copies);
    Some(CmdResult::sel(Sel::Block {
        anchor: nids.first()?.clone(),
        focus: nids.last()?.clone(),
        ids: nids,
    }))
}

/// Paste structured items at a caret: the first item's text is spliced into the current item at the caret, the
/// text after the caret goes to the end of the last pasted line, and the first item's children go under the
/// current item.
pub fn paste_items(tx: &mut Tx<'_>, v: &View, id: &str, s: usize, e: usize, items: &[Portable]) -> Option<CmdResult> {
    let n = tx.tree.get(id)?;
    let (first, rest) = items.split_first()?;
    let own_props = n.props.clone();
    let parent = n.parent.clone();
    let str = text(tx.tree, id);
    let head = text16::slice(&str, 0, s).to_owned();
    let tail = text16::slice_from(&str, e).to_owned();
    let first_text = first.props.get("c").and_then(Value::as_str).unwrap_or("");
    let Some(parent) = parent.filter(|_| id != v.zoom) else {
        // into the title: everything goes in as its first children
        let ids = insert_portable(tx, id, None, items);
        let last = ids.last()?.clone();
        return Some(CmdResult::sel(caret(&last, text16::len(&text(tx.tree, &last)))));
    };
    tx.set(id, str_props(&[("c", &(head.clone() + first_text))]));
    if head.is_empty() {
        let mut meta = meta_of(&own_props);
        meta.extend(meta_of(&first.props));
        set_meta(tx, id, &meta);
    }
    let (_, sub_last) = insert_deep(tx, id, None, &first.children);
    let (_, sibs_last) = insert_deep(tx, &parent, Some(id), rest);
    let last = sibs_last.or(sub_last).unwrap_or_else(|| id.to_owned());
    let last_text = text(tx.tree, &last);
    if !tail.is_empty() {
        tx.set(&last, str_props(&[("c", &(last_text.clone() + &tail))]));
    }
    Some(CmdResult::sel(caret(&last, text16::len(&last_text))))
}

// ---- inline formatting on raw text ----------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub text: String,
    pub s: usize,
    pub e: usize,
}

/// Toggle a symmetric delimiter (e.g. `**`) around `[s, e)`.
pub fn toggle_wrap(str: &str, s: usize, e: usize, d: &str) -> TextEdit {
    let l = text16::len(d);
    let sl = |a: usize, b: usize| text16::slice(str, a, b);
    if s >= l && sl(s - l, s) == d && sl(e, e + l) == d {
        return TextEdit {
            text: format!("{}{}{}", sl(0, s - l), sl(s, e), text16::slice_from(str, e + l)),
            s: s - l,
            e: e - l,
        };
    }
    let sel = sl(s, e);
    if text16::len(sel) >= 2 * l && sel.starts_with(d) && sel.ends_with(d) {
        let inner = text16::slice(sel, l, text16::len(sel) - l);
        return TextEdit {
            text: format!("{}{}{}", sl(0, s), inner, text16::slice_from(str, e)),
            s,
            e: s + text16::len(inner),
        };
    }
    let (mut a, mut b) = (s, e);
    let unit = |i: usize| str.encode_utf16().nth(i);
    if b.saturating_sub(a) > 1 && unit(a) == Some(u16::from(b' ')) {
        a += 1;
    }
    if b.saturating_sub(a) > 1 && unit(b - 1) == Some(u16::from(b' ')) {
        b -= 1;
    }
    TextEdit { text: format!("{}{d}{}{d}{}", sl(0, a), sl(a, b), text16::slice_from(str, b)), s: a + l, e: b + l }
}

/// Mod+K: `[sel]()` with the caret inside `()`; without a selection `[]()` with the caret in `[]`.
pub fn wrap_link(str: &str, s: usize, e: usize) -> TextEdit {
    let sel = text16::slice(str, s, e);
    let out = format!("{}[{sel}](){}", text16::slice(str, 0, s), text16::slice_from(str, e));
    let at = if sel.is_empty() { s + 1 } else { s + text16::len(sel) + 3 };
    TextEdit { text: out, s: at, e: at }
}

/// Toggle a delimiter around the whole text of every item (block formatting).
pub fn toggle_wrap_items(tx: &mut Tx<'_>, ids: &[NodeId], d: &str) {
    let l = text16::len(d);
    let wrapped = |s: &str| text16::len(s) >= 2 * l && s.starts_with(d) && s.ends_with(d);
    let all = ids.iter().all(|id| wrapped(&text(tx.tree, id)));
    for id in ids {
        let s = text(tx.tree, id);
        if all {
            let inner = text16::slice(&s, l, text16::len(&s) - l).to_owned();
            tx.set(id, str_props(&[("c", &inner)]));
        } else if !wrapped(&s) && !s.is_empty() {
            tx.set(id, str_props(&[("c", &format!("{d}{s}{d}"))]));
        }
    }
}

// ---- sorting ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortMode {
    TitleAsc,
    TitleDesc,
    DateNew,
    DateOld,
    UncheckedFirst,
    CheckedFirst,
    EditedNew,
    EditedOld,
    CreatedNew,
    CreatedOld,
    Reverse,
}

pub const SORT_LABELS: [(SortMode, &str); 11] = [
    (SortMode::TitleAsc, "Title A→Z"),
    (SortMode::TitleDesc, "Title Z→A"),
    (SortMode::DateNew, "Date new→old"),
    (SortMode::DateOld, "Date old→new"),
    (SortMode::UncheckedFirst, "Unchecked first"),
    (SortMode::CheckedFirst, "Checked first"),
    (SortMode::EditedNew, "Edited new→old"),
    (SortMode::EditedOld, "Edited old→new"),
    (SortMode::CreatedNew, "Created new→old"),
    (SortMode::CreatedOld, "Created old→new"),
    (SortMode::Reverse, "Reverse current"),
];

/// The first inline date of an item's text, in ms (for the date sorts).
pub type DateFn<'a> = &'a dyn Fn(&str) -> Option<f64>;

/// Compare titles as `localeCompare(_, _, { numeric: true })` does for plain text: case-insensitive, runs of
/// digits by their value.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = x.next_if(char::is_ascii_digit) {
                    na.push(c);
                }
                let mut nb = String::new();
                while let Some(c) = y.next_if(char::is_ascii_digit) {
                    nb.push(c);
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                if c != d {
                    return c.cmp(&d);
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Orders two items.
pub type Comparator<'a> = Box<dyn Fn(&NodeId, &NodeId) -> Ordering + 'a>;

/// A comparator for a sort mode (stable when used with a stable sort); `None` for `Reverse`.
pub fn sort_comparator<'a>(
    t: &'a Tree,
    mode: SortMode,
    first_date: DateFn<'a>,
) -> Option<Comparator<'a>> {
    let node = move |x: &NodeId| t.get(x).expect("item");
    let ck = move |x: &NodeId| prop_true(t, x, "ck");
    Some(match mode {
        SortMode::TitleAsc => Box::new(move |a, b| natural_cmp(&text(t, a), &text(t, b))),
        SortMode::TitleDesc => Box::new(move |a, b| natural_cmp(&text(t, b), &text(t, a))),
        SortMode::DateNew | SortMode::DateOld => {
            let newest_first = mode == SortMode::DateNew;
            // items without a date go last either way
            Box::new(move |a, b| match (first_date(&text(t, a)), first_date(&text(t, b))) {
                (Some(x), Some(y)) => {
                    let o = x.partial_cmp(&y).unwrap_or(Ordering::Equal);
                    if newest_first { o.reverse() } else { o }
                }
                (x, y) => x.is_none().cmp(&y.is_none()),
            })
        }
        SortMode::UncheckedFirst => Box::new(move |a, b| ck(a).cmp(&ck(b))),
        SortMode::CheckedFirst => Box::new(move |a, b| ck(b).cmp(&ck(a))),
        SortMode::EditedNew => Box::new(move |a, b| node(b).edited.cmp(&node(a).edited)),
        SortMode::EditedOld => Box::new(move |a, b| node(a).edited.cmp(&node(b).edited)),
        SortMode::CreatedNew => Box::new(move |a, b| node(b).created.cmp(&node(a).created)),
        SortMode::CreatedOld => Box::new(move |a, b| node(a).created.cmp(&node(b).created)),
        SortMode::Reverse => return None,
    })
}

/// Sort the children of each of `ids`.
pub fn sort_children(tx: &mut Tx<'_>, ids: &[NodeId], mode: SortMode, sel: Sel, first_date: DateFn<'_>) -> CmdResult {
    for p in ids {
        let mut kids = children(tx.tree, p);
        match sort_comparator(tx.tree, mode, first_date) {
            Some(cmp) => kids.sort_by(|a, b| cmp(a, b)),
            None => kids.reverse(),
        }
        let mut after: Option<NodeId> = None;
        for k in &kids {
            tx.move_to(k, Some(p), after.as_deref());
            after = Some(k.clone());
        }
    }
    CmdResult::sel(sel)
}
