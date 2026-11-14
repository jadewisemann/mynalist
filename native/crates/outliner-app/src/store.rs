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
    /// Its number among the shown children of a numbered parent (`num`), shown in place of the bullet; 0 if none.
    pub num: u32,
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
        // the children of `parent`, numbered when it numbers them, last first (for the stack)
        let kids = |parent: &str, depth: u16| {
            let numbered = item::numbered(t.get(parent));
            let shown = visible_children(t, parent, v);
            shown.into_iter().enumerate().rev().map(move |(k, c)| (c, depth, if numbered { k as u32 + 1 } else { 0 }))
        };
        let mut stack: Vec<(NodeId, u16, u32)> = kids(&v.zoom, 0).collect();
        while let Some((id, depth, num)) = stack.pop() {
            let Some(n) = t.get(&id) else { continue };
            rows.push(Row { id: id.clone(), depth, has_kids: !n.children.is_empty(), collapsed: n.collapsed, num });
            stack.extend(kids(&id, depth + 1));
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
        "**Bold** and __italic__ and `code`",
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

/// The inline showcase (`OUTLINER_DEMO=inline`): every kind of inline syntax, a note and a numbered list. Its
/// dates are fixed past days, so their labels do not depend on today.
pub fn showcase() -> DocSession<Option<Caret>> {
    let mut s = DocSession::default();
    let ts = now();
    // content, note, other props
    type Item<'a> = (&'a str, &'a str, &'a [(&'a str, Value)]);
    let items: [Item<'_>; 13] = [
        ("**Bold**, __italic__, ~~strike~~ and ==highlight==", "", &[]),
        ("Code `let x = 1;`, an escaped \\*star\\* and $$e^{i\\pi}+1=0$$", "", &[]),
        ("A [link](https://example.com), https://example.org/docs and #tag @me", "", &[]),
        ("Due !(2024-01-05) and !(2024-03-10 15:30 - 2024-03-10 17:00)", "", &[]),
        ("Every week !(2024-01-08 | 1w135) and ![a picture](https://example.com/a.png)", "", &[]),
        ("Item with a note", "A note under the item, with **bold** and a #tag.", &[]),
        ("Code block:\n```js\nconst a = 1;\nconsole.log(a);\n```", "", &[]),
        ("Numbered list", "", &[("num", Value::Bool(true))]),
        ("First", "", &[]),
        ("Second with ==mark==", "", &[]),
        ("Third", "", &[]),
        ("Checked with a date !(2024-01-05)", "", &[("ck", Value::Bool(true))]),
        ("Heading with ~~gone~~", "", &[("h", Value::from(2))]),
    ];
    let mut ops = Vec::new();
    let mut after: Option<NodeId> = None;
    let mut kid_after: Option<NodeId> = None;
    for (i, (c, n, extra)) in items.iter().enumerate() {
        let id = format!("s{i}");
        let mut props = text_props(c);
        if !n.is_empty() {
            props.insert("n".into(), Value::String((*n).to_owned()));
        }
        for (k, v) in extra.iter() {
            props.insert((*k).to_owned(), v.clone());
        }
        // the three after "Numbered list" are its children
        if (8..=10).contains(&i) {
            ops.push(Op::Create { id: id.clone(), parent: "s7".into(), after: kid_after.replace(id.clone()), props, ts });
        } else {
            ops.push(Op::Create { id: id.clone(), parent: ROOT.into(), after: after.replace(id.clone()), props, ts });
        }
    }
    let mut title = Props::new();
    title.insert("c".into(), Value::String("Inline".into()));
    ops.insert(0, Op::Set { id: ROOT.to_owned(), props: title, ts });
    s.transact(ops, None, None, false);
    s
}

/// The text of an item.
pub fn content_of(s: &State, id: &str) -> String {
    item::content(s.tree().get(id)).to_owned()
}

/// The looks of items (for the look checks, `fixture`): headings of three levels, checked items, checkboxes on
/// and off, the six color labels, a folded item, a note under a checked item and a deeper level.
pub fn looks() -> DocSession<Option<Caret>> {
    let mut s = DocSession::default();
    let ts = now();
    // id, parent, content, note, other props
    type Item<'a> = (&'a str, &'a str, &'a str, &'a str, &'a [(&'a str, Value)]);
    let items: [Item<'_>; 19] = [
        ("l0", ROOT, "Heading one", "", &[("h", Value::from(1))]),
        ("l1", ROOT, "Heading two", "", &[("h", Value::from(2))]),
        ("l2", ROOT, "Heading three", "", &[("h", Value::from(3))]),
        ("l3", ROOT, "A checked item", "Its note is checked too.", &[("ck", Value::Bool(true))]),
        ("l4", ROOT, "A checkbox, off", "", &[("cb", Value::Bool(true))]),
        ("l5", ROOT, "A checkbox, on", "", &[("cb", Value::Bool(true)), ("ck", Value::Bool(true))]),
        ("l6", ROOT, "Color labels", "", &[]),
        ("l7", "l6", "Label 1", "", &[("cl", Value::from(1))]),
        ("l8", "l6", "Label 2", "", &[("cl", Value::from(2))]),
        ("l9", "l6", "Label 3", "", &[("cl", Value::from(3))]),
        ("l10", "l6", "Label 4", "", &[("cl", Value::from(4))]),
        ("l11", "l6", "Label 5", "", &[("cl", Value::from(5))]),
        ("l12", "l6", "Label 6", "", &[("cl", Value::from(6))]),
        ("l13", ROOT, "A folded item", "", &[("cl", Value::from(5))]),
        ("l14", "l13", "Hidden under the fold", "", &[]),
        ("l15", ROOT, "Levels", "", &[]),
        ("l16", "l15", "Second level", "", &[]),
        ("l17", "l16", "Third level with a **bold** word", "", &[]),
        ("l18", "l17", "Fourth level", "", &[]),
    ];
    let mut ops = Vec::new();
    let mut last: std::collections::HashMap<&str, NodeId> = std::collections::HashMap::new();
    for (id, parent, c, n, extra) in items.iter() {
        let mut props = text_props(c);
        if !n.is_empty() {
            props.insert("n".into(), Value::String((*n).to_owned()));
        }
        for (k, v) in extra.iter() {
            props.insert((*k).to_owned(), v.clone());
        }
        let after = last.insert(parent, (*id).to_owned());
        ops.push(Op::Create { id: (*id).to_owned(), parent: (*parent).to_owned(), after, props, ts });
    }
    ops.push(Op::Collapse { id: "l13".into(), v: true });
    let mut title = Props::new();
    title.insert("c".into(), Value::String("Looks".into()));
    ops.insert(0, Op::Set { id: ROOT.to_owned(), props: title, ts });
    s.transact(ops, None, None, false);
    s
}
