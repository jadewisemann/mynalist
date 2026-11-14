//! The open document: a [`DocSession`] and the rows it shows, kept outside the reactive graph (a session is
//! large and not `Clone`) and announced through one revision signal. Views read with [`read`] (tracked) and
//! change it with [`edit`], which bumps the revision.

use std::cell::RefCell;
use std::rc::Rc;

use nsh_core::prelude::*;
use outliner_core::view::{visible_children, View};
use outliner_core::{item, DocSession, NodeId, Op, Props, Tree, ROOT};
use serde_json::Value;

/// Where the caret is: the item and a UTF-16 offset in its text (the unit the platforms' inputs use).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Caret {
    pub id: NodeId,
    pub at: u32,
}

/// One visible row of the outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: NodeId,
    pub depth: u16,
    pub has_kids: bool,
    pub collapsed: bool,
}

pub struct State {
    pub session: DocSession<Option<Caret>>,
    pub rows: Vec<Row>,
    /// The item being edited, and where its caret goes when the row's input appears.
    pub editing: Option<Caret>,
    /// Bumped when a command (not typing) places the caret, so the row's input moves its selection there.
    pub caret_rev: u64,
    /// The zoom root and the visibility options.
    pub view: View,
}

impl State {
    fn new(session: DocSession<Option<Caret>>) -> Self {
        let mut s = State { session, rows: Vec::new(), editing: None, caret_rev: 0, view: View::default() };
        s.reflow();
        s
    }

    pub fn tree(&self) -> &Tree {
        &self.session.live
    }

    /// Recompute the visible rows: the items under the zoom root that the view shows, depth first.
    pub fn reflow(&mut self) {
        let t = &self.session.live;
        if !t.is_attached(&self.view.zoom) {
            self.view.zoom = ROOT.to_owned();
        }
        let v = &self.view;
        let mut rows = Vec::with_capacity(self.rows.len().max(64));
        let mut stack: Vec<(NodeId, u16)> = visible_children(t, &v.zoom, v).into_iter().rev().map(|c| (c, 0)).collect();
        while let Some((id, depth)) = stack.pop() {
            let Some(n) = t.get(&id) else { continue };
            let shown = visible_children(t, &id, v);
            rows.push(Row { id: id.clone(), depth, has_kids: !n.children.is_empty(), collapsed: n.collapsed });
            stack.extend(shown.into_iter().rev().map(|c| (c, depth + 1)));
        }
        self.rows = rows;
    }

    pub fn row_index(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }
}

#[derive(Clone)]
struct Shared(Rc<RefCell<State>>);

impl Default for Shared {
    fn default() -> Self {
        Shared(Rc::new(RefCell::new(State::new(DocSession::default()))))
    }
}

/// The revision of the document: bumped by every change.
#[derive(Clone, Copy, Default, PartialEq)]
struct Rev(u64);

fn shared() -> Rc<RefCell<State>> {
    global::<Shared>().get_untracked().0
}

/// Read the document; the reader runs again when it changes.
pub fn read<R>(f: impl FnOnce(&State) -> R) -> R {
    global::<Rev>().get();
    f(&shared().borrow())
}

/// Read without tracking (in handlers).
pub fn peek<R>(f: impl FnOnce(&State) -> R) -> R {
    f(&shared().borrow())
}

/// Change the document and tell its readers.
pub fn edit<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let out = f(&mut shared().borrow_mut());
    global::<Rev>().update(|r| r.0 += 1);
    out
}

/// Replace the open document (a fresh session, e.g. from a snapshot or for a test).
pub fn open(session: DocSession<Option<Caret>>) {
    edit(|s| *s = State::new(session));
}

pub fn now() -> i64 {
    nsh_core::time::unix_ms() as i64
}

pub fn text_props(text: &str) -> Props {
    let mut p = Props::new();
    p.insert("c".into(), Value::String(text.to_owned()));
    p
}

/// A demo document: `n` items, a few levels deep, with inline formatting, for trying the list.
pub fn demo(n: usize) -> DocSession<Option<Caret>> {
    let mut s = DocSession::default();
    let ts = now();
    let samples = [
        "Plain item",
        "**Bold** and *italic* and `code`",
        "A [link](https://example.com) and a #tag",
        "한글 항목: 입력 테스트",
        "Mixed **굵게** `코드` text",
    ];
    let mut ops = Vec::with_capacity(n);
    // the last item made at each depth (the parent of the next depth), and the last child of each parent
    let mut path: Vec<NodeId> = Vec::new();
    let mut last_child: std::collections::HashMap<NodeId, NodeId> = std::collections::HashMap::new();
    for i in 0..n {
        let want = match i % 7 {
            0 => 0,
            3 => 2,
            _ => 1,
        };
        let depth = want.min(path.len());
        path.truncate(depth);
        let parent = path.last().cloned().unwrap_or_else(|| ROOT.to_owned());
        let id = format!("d{i}");
        let after = last_child.insert(parent.clone(), id.clone());
        let text = format!("{} {}", samples[i % samples.len()], i + 1);
        let mut props = text_props(&text);
        // a few checked items, checkboxes and color labels, to see how they look
        if i % 17 == 9 {
            props.insert("ck".into(), Value::Bool(true));
        }
        if i % 19 == 12 {
            props.insert("cb".into(), Value::Bool(true));
        }
        if i % 23 == 15 {
            props.insert("cl".into(), Value::from(4));
        }
        ops.push(Op::Create { id: id.clone(), parent, after, props, ts });
        path.push(id);
    }
    let mut title = Props::new();
    title.insert("c".into(), Value::String("Demo".into()));
    ops.insert(0, Op::Set { id: ROOT.to_owned(), props: title, ts });
    s.transact(ops, None, None, false);
    s
}

/// The text of an item.
pub fn content_of(s: &State, id: &str) -> String {
    item::content(s.tree().get(id)).to_owned()
}
