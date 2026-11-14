//! Editing commands of the spike, on the open document. Simplified versions of `packages/core/src/commands.ts`
//! (which the full app will port with its Dynalist parity rules): split, merge, indent, outdent, moving the caret
//! between rows, folding, undo and redo.

use outliner_core::{item, Op, Props, Tx};
use serde_json::Value;

use crate::store::{self, Caret, State};

/// Byte offset of a UTF-16 offset in `s` (clamped to the end).
pub fn byte_at(s: &str, utf16: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in s.char_indices() {
        if units >= utf16 {
            return i;
        }
        units += c.len_utf16() as u32;
    }
    s.len()
}

pub fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

fn commit(s: &mut State, f: impl FnOnce(&mut Tx<'_>), before: Option<Caret>, after: Option<Caret>) {
    let (ops, inverse) = {
        let mut tx = Tx::new(&mut s.session.live, store::now());
        f(&mut tx);
        (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
    };
    s.session.commit(ops, inverse, before, after.clone(), true);
    s.editing = after;
    s.reflow();
}

/// Typing: the row's text is now `text`.
pub fn set_text(id: &str, text: &str, caret: u32) {
    store::edit(|s| {
        if item::content(s.tree().get(id)) == text {
            return;
        }
        let at = Caret { id: id.to_owned(), at: caret };
        let op = Op::Set { id: id.to_owned(), props: store::text_props(text), ts: store::now() };
        s.session.transact(vec![op], Some(at.clone()), Some(at), true);
    });
}

/// Enter: split the row at the caret; the text after it goes to a new sibling below (or the first child, when
/// the row's children show), which takes the caret. At the very start of a non-empty row, a new empty row goes
/// above instead and the caret stays.
pub fn split(id: &str, caret: u32) {
    store::edit(|s| {
        let Some(n) = s.tree().get(id) else { return };
        let text = item::content(Some(n)).to_owned();
        let parent = n.parent.clone().unwrap_or_default();
        let open_kids = !n.collapsed && !n.children.is_empty();
        let cut = byte_at(&text, caret);
        let before = Some(Caret { id: id.to_owned(), at: caret });
        if cut == 0 && !text.is_empty() {
            let prev = s.tree().prev_sibling(id).cloned();
            commit(s, |tx| {
                tx.create(&parent, prev.as_deref(), Props::new());
            }, before, Some(Caret { id: id.to_owned(), at: 0 }));
            return;
        }
        let (head, tail) = text.split_at(cut);
        let (head, tail) = (head.to_owned(), tail.to_owned());
        let new_id = outliner_core::new_id();
        let after = Some(Caret { id: new_id.clone(), at: 0 });
        commit(s, |tx| {
            tx.set(id, store::text_props(&head));
            if open_kids {
                tx.create_with_id(id, None, store::text_props(&tail), new_id.clone());
            } else {
                tx.create_with_id(&parent, Some(id), store::text_props(&tail), new_id.clone());
            }
        }, before, after);
    });
}

/// Backspace at the start of a row: merge it into the row above (its children follow). Returns whether it did.
pub fn merge_up(id: &str) -> bool {
    store::edit(|s| {
        let Some(i) = s.row_index(id).filter(|&i| i > 0) else { return false };
        let prev = s.rows[i - 1].id.clone();
        let Some(n) = s.tree().get(id) else { return false };
        // a row whose children are not the one above's to take stays put
        if !n.children.is_empty() && s.tree().get(&prev).is_some_and(|p| !p.children.is_empty()) {
            return false;
        }
        let text = item::content(Some(n)).to_owned();
        let kids = n.children.clone();
        let prev_text = store::content_of(s, &prev);
        let at = utf16_len(&prev_text);
        let after = Some(Caret { id: prev.clone(), at });
        commit(s, |tx| {
            tx.set(&prev, store::text_props(&(prev_text.clone() + &text)));
            let mut after: Option<String> = None;
            for k in &kids {
                tx.move_to(k, Some(&prev), after.as_deref());
                after = Some(k.clone());
            }
            tx.move_to(id, None, None);
        }, Some(Caret { id: id.to_owned(), at: 0 }), after);
        true
    })
}

/// Tab: make the row the last child of the sibling above.
pub fn indent(id: &str, caret: u32) {
    store::edit(|s| {
        let Some(prev) = s.tree().prev_sibling(id).cloned() else { return };
        let last = s.tree().get(&prev).and_then(|p| p.children.last().cloned());
        let at = Some(Caret { id: id.to_owned(), at: caret });
        commit(s, |tx| {
            tx.collapse(&prev, false);
            tx.move_to(id, Some(&prev), last.as_deref());
        }, at.clone(), at);
    });
}

/// Shift+Tab: make the row the sibling after its parent; the siblings below it become its children.
pub fn outdent(id: &str, caret: u32) {
    store::edit(|s| {
        let Some(n) = s.tree().get(id) else { return };
        let Some(parent) = n.parent.clone() else { return };
        let Some(grand) = s.tree().get(&parent).and_then(|p| p.parent.clone()) else { return };
        let sibs = &s.tree().get(&parent).expect("parent").children;
        let below: Vec<String> = sibs.iter().skip_while(|c| *c != id).skip(1).cloned().collect();
        let mut after = n.children.last().cloned();
        let at = Some(Caret { id: id.to_owned(), at: caret });
        commit(s, |tx| {
            tx.move_to(id, Some(&grand), Some(&parent));
            for b in &below {
                tx.move_to(b, Some(id), after.as_deref());
                after = Some(b.clone());
            }
        }, at.clone(), at);
    });
}

/// Up / Down: edit the row above / below, at the start / end of its text.
pub fn step(id: &str, down: bool) {
    store::edit(|s| {
        let Some(i) = s.row_index(id) else { return };
        let j = if down { i + 1 } else { i.wrapping_sub(1) };
        if let Some(r) = s.rows.get(j) {
            let at = if down { 0 } else { utf16_len(&store::content_of(s, &r.id)) };
            s.editing = Some(Caret { id: r.id.clone(), at });
        }
    });
}

pub fn start_editing(id: &str, at: Option<u32>) {
    store::edit(|s| {
        let at = at.unwrap_or_else(|| utf16_len(&store::content_of(s, id)));
        s.editing = Some(Caret { id: id.to_owned(), at });
    });
}

pub fn toggle(id: &str) {
    store::edit(|s| {
        let Some(n) = s.tree().get(id) else { return };
        if n.children.is_empty() {
            return;
        }
        let op = Op::Collapse { id: id.to_owned(), v: !n.collapsed };
        s.session.transact(vec![op], s.editing.clone(), s.editing.clone(), true);
        s.reflow();
    });
}

pub fn undo(redo: bool) {
    store::edit(|s| {
        let (sel, change) = if redo { s.session.redo() } else { s.session.undo() };
        if change.is_some() {
            s.reflow();
            if let Some(sel) = sel {
                s.editing = sel.or(s.editing.clone());
            }
        }
    });
}

/// Checkbox on / off, as Mod+Enter would.
pub fn toggle_checked(id: &str) {
    store::edit(|s| {
        let checked = item::is_checked(s.tree().get(id));
        let mut p = Props::new();
        p.insert("ck".into(), Value::Bool(!checked));
        let at = s.editing.clone();
        s.session.transact(vec![Op::Set { id: id.to_owned(), props: p, ts: store::now() }], at.clone(), at, true);
    });
}
