//! The app's editing commands: the core's commands (`outliner_core::commands`, the port of `commands.ts` with its
//! Dynalist rules) run on the open document, recorded for undo and sync, with the caret they leave.

use outliner_core::commands::{self as core, CheckOptions, CmdResult, Dir, Sel};
use outliner_core::view::{self, View};
use outliner_core::{item, text16, NodeId, Op, Tx, ROOT};

use crate::store::{self, Caret, State};

pub fn utf16_len(s: &str) -> u32 {
    text16::len(s) as u32
}

fn caret_of(sel: &Sel) -> Option<Caret> {
    match sel {
        Sel::Text { id, s, .. } => Some(Caret { id: id.clone(), at: *s as u32 }),
        Sel::Block { anchor, .. } => Some(Caret { id: anchor.clone(), at: 0 }),
        Sel::None => None,
    }
}

fn text_sel(id: &str, at: u32) -> Sel {
    core::caret(id, at as usize)
}

/// Run `f` on the open document as one undoable change; the caret goes where its result says.
fn run(before: Option<Caret>, f: impl FnOnce(&mut Tx<'_>, &View) -> Option<CmdResult>) -> bool {
    store::edit(|s| {
        let v = s.view.clone();
        let (res, ops, inverse) = {
            let mut tx = Tx::new(&mut s.session.live, store::now());
            let res = f(&mut tx, &v);
            (res, std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
        };
        let done = res.is_some() || !ops.is_empty();
        let after = match &res {
            Some(r) => caret_of(&r.sel),
            None => before.clone(),
        };
        s.session.commit(ops, inverse, before, after.clone(), true);
        if let Some(z) = res.and_then(|r| r.zoom) {
            s.view.zoom = z;
        }
        place(s, after);
        s.reflow();
        done
    })
}

fn place(s: &mut State, caret: Option<Caret>) {
    s.editing = caret;
    s.caret_rev += 1;
}

fn at(id: &str, caret: u32) -> Option<Caret> {
    Some(Caret { id: id.to_owned(), at: caret })
}

/// Typing: the row's text is now `text`.
pub fn set_text(id: &str, text: &str, caret: u32) {
    store::edit(|s| {
        if item::content(s.tree().get(id)) == text {
            return;
        }
        let c = at(id, caret);
        let op = Op::Set { id: id.to_owned(), props: store::text_props(text), ts: store::now() };
        s.session.transact(vec![op], c.clone(), c, true);
        if let Some(e) = s.editing.as_mut().filter(|e| e.id == id) {
            e.at = caret;
        }
    });
}

/// Enter with the selection `s..e`.
pub fn enter(id: &str, s: u32, e: u32) {
    run(at(id, s), |tx, v| core::enter(tx, v, id, s as usize, e as usize));
}

/// Backspace at the start of a row. Returns whether it did something (else the key goes on to the field).
pub fn backspace(id: &str) -> bool {
    run(at(id, 0), |tx, v| core::backspace_at_start(tx, v, id))
}

/// Delete at the end of a row.
pub fn delete_forward(id: &str, caret: u32) -> bool {
    run(at(id, caret), |tx, v| core::delete_at_end(tx, v, id))
}

pub fn indent(id: &str, caret: u32) {
    run(at(id, caret), |tx, v| core::indent(tx, v, &[id.to_owned()], text_sel(id, caret)));
}

pub fn outdent(id: &str, caret: u32) {
    run(at(id, caret), |tx, v| core::outdent(tx, v, &[id.to_owned()], text_sel(id, caret), false));
}

pub fn move_up(id: &str, caret: u32) {
    run(at(id, caret), |tx, v| core::move_up(tx, v, &[id.to_owned()], text_sel(id, caret)));
}

pub fn move_down(id: &str, caret: u32) {
    run(at(id, caret), |tx, v| core::move_down(tx, v, &[id.to_owned()], text_sel(id, caret)));
}

/// Mod+Shift+Backspace: delete the row, the caret going to the end of the row above.
pub fn delete_item(id: &str, caret: u32) {
    run(at(id, caret), |tx, v| core::delete_items(tx, v, &[id.to_owned()], Dir::Left));
}

/// Mod+Enter (moves on to the next row) or the toolbar's check (`stay`).
pub fn toggle_checked(id: &str, caret: u32, stay: bool) {
    run(at(id, caret), |tx, v| {
        core::toggle_checked(tx, v, &[id.to_owned()], text_sel(id, caret), &CheckOptions { stay })
    });
}

pub fn toggle_checkbox(id: &str, caret: u32) {
    run(at(id, caret), |tx, _| Some(core::toggle_prop(tx, &[id.to_owned()], "cb", text_sel(id, caret))));
}

/// Fold or unfold the row (all its descendants with `recursive`). Folding is not undoable.
pub fn toggle_collapse(id: &str, recursive: bool) {
    store::edit(|s| {
        let v = s.view.clone();
        let ops = {
            let mut tx = Tx::new(&mut s.session.live, store::now());
            core::toggle_collapse(&mut tx, &v, &[id.to_owned()], recursive);
            (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
        };
        let c = s.editing.clone();
        s.session.commit(ops.0, ops.1, c.clone(), c, true);
        s.reflow();
    });
}

/// Up / Down: edit the row above / below, at the start / end of its text.
pub fn step(id: &str, down: bool) {
    store::edit(|s| {
        let t = s.tree();
        let next = if down { view::next_visible(t, id, &s.view) } else { view::prev_visible(t, id, &s.view) };
        if let Some(n) = next.filter(|n| *n != s.view.zoom) {
            let at = if down { 0 } else { utf16_len(&store::content_of(s, &n)) };
            place(s, Some(Caret { id: n, at }));
        }
    });
}

/// Mod+]: show only this row and what is under it.
pub fn zoom_in(id: &str) {
    store::edit(|s| {
        if s.tree().contains(id) {
            s.view.zoom = id.to_owned();
            let first = s.tree().get(id).and_then(|n| n.children.first().cloned());
            place(s, first.map(|c| Caret { id: c, at: 0 }));
            s.reflow();
        }
    });
}

/// Mod+[: back to the parent of the zoom root.
pub fn zoom_out() {
    store::edit(|s| {
        let zoom = s.view.zoom.clone();
        if let Some(p) = s.tree().get(&zoom).and_then(|n| n.parent.clone()) {
            s.view.zoom = p;
            place(s, Some(Caret { id: zoom, at: 0 }));
            s.reflow();
        }
    });
}

pub fn zoom() -> NodeId {
    store::read(|s| s.view.zoom.clone())
}

pub fn is_zoomed() -> bool {
    store::read(|s| s.view.zoom != ROOT)
}

pub fn start_editing(id: &str, at: Option<u32>) {
    store::edit(|s| {
        let at = at.unwrap_or_else(|| utf16_len(&store::content_of(s, id)));
        place(s, Some(Caret { id: id.to_owned(), at }));
    });
}

pub fn undo(redo: bool) {
    store::edit(|s| {
        let (sel, change) = if redo { s.session.redo() } else { s.session.undo() };
        if change.is_some() {
            s.reflow();
            if let Some(Some(c)) = sel {
                if s.tree().is_attached(&c.id) {
                    place(s, Some(c));
                }
            }
        }
    });
}
