//! The app's editing commands: the core's commands (`outliner_core::commands`, the port of `commands.ts` with its
//! Dynalist rules) run on the open document, recorded for undo and sync, with the caret or the selected items
//! they leave. Commands on items act on the selected items, or on the item being edited (`targets`).

use outliner_core::commands::{self as core, CheckOptions, CmdResult, Dir, Field, Recurring, Sel};
use outliner_core::view::{self, View};
use outliner_core::{item, text16, NodeId, Op, Tree, Tx, ROOT};
use serde_json::Value;

use crate::store::{self, Block, Caret, Mark, State};

pub fn utf16_len(s: &str) -> u32 {
    text16::len(s) as u32
}

fn mark_of(sel: &Sel) -> Option<Mark> {
    match sel {
        Sel::Text { id, f, s, e } => Some(Mark::Caret(Caret {
            id: id.clone(),
            at: *s as u32,
            end: *e as u32,
            note: *f == Field::Note,
        })),
        Sel::Block { ids, anchor, focus } => {
            Some(Mark::Block(Block { ids: ids.clone(), anchor: anchor.clone(), focus: focus.clone() }))
        }
        Sel::None => None,
    }
}

fn sel_of(m: &Option<Mark>) -> Sel {
    match m {
        Some(Mark::Caret(c)) => Sel::Text {
            id: c.id.clone(),
            f: if c.note { Field::Note } else { Field::Content },
            s: c.at.min(c.end) as usize,
            e: c.at.max(c.end) as usize,
        },
        Some(Mark::Block(b)) => Sel::Block { ids: b.ids.clone(), anchor: b.anchor.clone(), focus: b.focus.clone() },
        None => Sel::None,
    }
}

/// The selection now: the selected items, or the caret.
fn current(s: &State) -> Option<Mark> {
    match (&s.block, &s.editing) {
        (Some(b), _) => Some(Mark::Block(b.clone())),
        (None, Some(c)) => Some(Mark::Caret(c.clone())),
        (None, None) => None,
    }
}

/// The items a command on items acts on: the selected ones, or the one being edited (not the zoom root).
fn targets(s: &State) -> Vec<NodeId> {
    match (&s.block, &s.editing) {
        (Some(b), _) => b.ids.clone(),
        (None, Some(c)) if c.id != s.view.zoom => vec![c.id.clone()],
        _ => Vec::new(),
    }
}

/// Run `f` on the open document's selection as one undoable change; the selection becomes what its result says.
/// Returns whether it did something.
fn run(f: impl FnOnce(&mut Tx<'_>, &View, &[NodeId], Sel) -> Option<CmdResult>) -> bool {
    store::edit(|s| {
        let v = s.view.clone();
        let before = current(s);
        let ids = targets(s);
        let (res, ops, inverse) = {
            let mut tx = Tx::new(&mut s.session.live, store::now());
            let res = f(&mut tx, &v, &ids, sel_of(&before));
            (res, std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
        };
        let done = res.is_some() || !ops.is_empty();
        let after = match &res {
            Some(r) => mark_of(&r.sel),
            None => before.clone(),
        };
        s.session.commit(ops, inverse, before, after.clone(), true);
        if let Some(z) = res.and_then(|r| r.zoom) {
            s.view.zoom = z;
        }
        s.reflow();
        place(s, after);
        done
    })
}

/// Run `f` on the items the selection targets, when there are any.
fn run_items(f: impl FnOnce(&mut Tx<'_>, &View, &[NodeId], Sel) -> Option<CmdResult>) -> bool {
    !store::peek(|s| targets(s).is_empty()) && run(f)
}

/// Make `mark` the selection: the caret (an item edited) or selected items, or neither.
fn place(s: &mut State, mark: Option<Mark>) {
    match mark {
        Some(Mark::Caret(c)) => {
            s.editing = Some(c);
            s.block = None;
        }
        Some(Mark::Block(b)) => {
            s.editing = None;
            s.block = Some(b);
        }
        None => {
            s.editing = None;
            s.block = None;
        }
    }
    s.caret_rev += 1;
}

fn place_caret(s: &mut State, c: Caret) {
    place(s, Some(Mark::Caret(c)));
}

/// The input reports where its caret is now (typing, a press in it): kept for the commands, without making the
/// views again.
pub fn caret_moved(id: &str, note: bool, at: u32, end: u32) {
    store::quiet(|s| {
        if let Some(c) = s.editing.as_mut().filter(|c| c.id == id && c.note == note) {
            c.at = at;
            c.end = end;
        }
    });
}

/// Typing: the content (or the note) of `id` is now `text`.
pub fn set_text(id: &str, note: bool, text: &str, caret: u32) {
    store::edit(|s| {
        let n = s.tree().get(id);
        let old = if note { item::note(n) } else { item::content(n) };
        if old == text {
            return;
        }
        let c = Some(Mark::Caret(Caret { note, ..Caret::text(id, caret) }));
        let mut props = outliner_core::Props::new();
        props.insert(if note { "n" } else { "c" }.into(), Value::String(text.to_owned()));
        let op = Op::Set { id: id.to_owned(), props, ts: store::now() };
        s.session.transact(vec![op], c.clone(), c, true);
        if let Some(e) = s.editing.as_mut().filter(|e| e.id == id && e.note == note) {
            e.at = caret;
            e.end = caret;
        }
    });
}

/// Replace `s..e` of the field the caret is in with `with`, the caret going to `caret` (one undo step).
pub fn replace(id: &str, note: bool, s: u32, e: u32, with: &str, caret: u32) {
    let key = if note { "n" } else { "c" };
    run(|tx, _, _, _| {
        let n = tx.tree.get(id)?;
        let src = n.props.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
        let (a, b) = (text16::byte_at(&src, s as usize), text16::byte_at(&src, e as usize));
        let text = format!("{}{with}{}", &src[..a], &src[b..]);
        let mut props = outliner_core::Props::new();
        props.insert(key.into(), Value::String(text));
        tx.set(id, props);
        let f = if note { Field::Note } else { Field::Content };
        Some(CmdResult { sel: Sel::Text { id: id.to_owned(), f, s: caret as usize, e: caret as usize }, zoom: None })
    });
}

/// Enter with the selection `s..e`.
pub fn enter(id: &str, s: u32, e: u32) {
    run(|tx, v, _, _| core::enter(tx, v, id, s as usize, e as usize));
}

/// Backspace at the start of a row. Returns whether it did something (else the key goes on to the field).
pub fn backspace(id: &str) -> bool {
    run(|tx, v, _, _| core::backspace_at_start(tx, v, id))
}

/// Delete at the end of a row.
pub fn delete_forward(id: &str) -> bool {
    run(|tx, v, _, _| core::delete_at_end(tx, v, id))
}

pub fn indent() {
    run_items(core::indent);
}

pub fn outdent() {
    // "Unindent items in-place": the siblings after it become its children
    let in_place = crate::settings::get().flag("unindent_in_place");
    run_items(move |tx, v, ids, sel| core::outdent(tx, v, ids, sel, in_place));
}

pub fn move_up() {
    run_items(core::move_up);
}

pub fn move_down() {
    run_items(core::move_down);
}

/// Mod+Shift+Backspace (and Backspace on selected items): delete the items, the caret going to the end of the
/// row above; Delete on selected items (`Dir::Right`): to the start of the row below.
pub fn delete_items(dir: Dir) {
    run_items(|tx, v, ids, _| core::delete_items(tx, v, ids, dir));
}

/// Enter on selected items: they are replaced by one empty item, edited.
pub fn replace_with_empty() {
    run_items(|tx, v, ids, _| core::replace_with_empty(tx, v, ids));
}

/// Mod+Enter (moves on to the next row) or the toolbar's check (`stay`).
/// What checking an item with a recurring date does (the "Recurring date" setting).
fn recurring() -> Recurring {
    if crate::settings::get().pick("recurrence_check_behavior") == "update" { Recurring::Update } else { Recurring::NewInstance }
}

pub fn toggle_checked(stay: bool) {
    run_items(|tx, v, ids, sel| {
        // a recurring date moves on: a checked copy staying above, or the date itself (`recurrence_check_behavior`)
        let opts = CheckOptions { stay, recurring: Some(recurring()), now: None };
        core::toggle_checked(tx, v, ids, sel, &opts)
    });
}

pub fn toggle_checkbox() {
    run_items(|tx, _, ids, sel| Some(core::toggle_prop(tx, ids, "cb", sel)));
}

/// Fold or unfold the items (all their descendants with `recursive`). Folding is not undoable.
pub fn toggle_collapse(recursive: bool) {
    let ids = store::peek(targets);
    collapse(&ids, recursive);
}

/// Fold or unfold `ids` (the bullet's press).
pub fn collapse(ids: &[NodeId], recursive: bool) {
    if ids.is_empty() {
        return;
    }
    store::edit(|s| {
        let v = s.view.clone();
        let ops = {
            let mut tx = Tx::new(&mut s.session.live, store::now());
            core::toggle_collapse(&mut tx, &v, ids, recursive);
            (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
        };
        let c = current(s);
        s.session.commit(ops.0, ops.1, c.clone(), c, true);
        s.reflow();
        // a selected item hidden under a fold leaves the selection
        if let Some(b) = &mut s.block {
            let shown: Vec<NodeId> = b.ids.iter().filter(|id| s.rows.iter().any(|r| &r.id == *id)).cloned().collect();
            if shown.is_empty() {
                s.block = None;
            } else if shown.len() != b.ids.len() {
                *b = Block { anchor: shown[0].clone(), focus: shown[shown.len() - 1].clone(), ids: shown };
            }
        }
    });
}

fn content_len(t: &Tree, id: &str) -> u32 {
    utf16_len(item::content(t.get(id)))
}

/// Up / Down from the input's first / last line: the row above (at the end of its text) or below (at its start).
/// From a note, Up goes to its item's content and Down to the next row.
pub fn step(id: &str, down: bool, from_note: bool) {
    store::edit(|s| {
        let t = s.tree();
        if from_note && !down {
            let at = content_len(t, id);
            place_caret(s, Caret::text(id, at));
            return;
        }
        let next = if down { view::next_visible(t, id, &s.view) } else { view::prev_visible(t, id, &s.view) };
        if let Some(n) = next.filter(|n| *n != s.view.zoom) {
            let at = if down { 0 } else { content_len(t, &n) };
            place_caret(s, Caret::text(&n, at));
        }
    });
}

/// Shift+Enter (the web's `goto-note`), and the phone toolbar's note button: from the content to the end of the
/// note, from the note to the end of the content. Text selected in the content of an item without a note moves
/// into the note.
pub fn goto_note() {
    let Some(c) = store::peek(|s| s.editing.clone()) else { return };
    if c.note {
        store::edit(|s| {
            let at = content_len(s.tree(), &c.id);
            place_caret(s, Caret::text(&c.id, at));
        });
        return;
    }
    let (note, text) = store::peek(|s| {
        let n = s.tree().get(&c.id);
        (item::note(n).to_owned(), item::content(n).to_owned())
    });
    let (a, b) = (c.at.min(c.end), c.at.max(c.end));
    if a != b && note.is_empty() {
        run(|tx, _, _, _| {
            let (x, y) = (text16::byte_at(&text, a as usize), text16::byte_at(&text, b as usize));
            let mut props = outliner_core::Props::new();
            props.insert("c".into(), Value::String(format!("{}{}", &text[..x], &text[y..])));
            props.insert("n".into(), Value::String(text[x..y].to_owned()));
            tx.set(&c.id, props);
            Some(CmdResult { sel: Sel::Text { id: c.id.clone(), f: Field::Note, s: (b - a) as usize, e: (b - a) as usize }, zoom: None })
        });
        return;
    }
    store::edit(|s| place_caret(s, Caret::note(&c.id, utf16_len(&note))));
}

/// Backspace or Left at the start of a note, or Backspace in an empty one: to the end of its item's content.
pub fn leave_note(id: &str) {
    store::edit(|s| {
        let at = content_len(s.tree(), id);
        place_caret(s, Caret::text(id, at));
    });
}

/// Mod+]: show only this row and what is under it.
pub fn zoom_in() {
    let Some(id) = store::peek(|s| targets(s).into_iter().next()) else { return };
    zoom_into(&id);
}

/// Show only item `id` and what is under it (the caret on its first child).
pub fn zoom_into(id: &str) {
    let id = id.to_owned();
    store::edit(|s| {
        if s.tree().contains(&id) {
            s.view.zoom = id.clone();
            let first = s.tree().get(&id).and_then(|n| n.children.first().cloned());
            s.reflow();
            place(s, first.map(|c| Mark::Caret(Caret::text(&c, 0))));
        }
    });
}

/// Mod+[: back to the parent of the zoom root.
pub fn zoom_out() {
    store::edit(|s| {
        let zoom = s.view.zoom.clone();
        if let Some(p) = s.tree().get(&zoom).and_then(|n| n.parent.clone()) {
            s.view.zoom = p;
            s.reflow();
            place_caret(s, Caret::text(&zoom, 0));
        }
    });
}

pub fn zoom() -> NodeId {
    store::read(|s| s.view.zoom.clone())
}

pub fn is_zoomed() -> bool {
    store::read(|s| s.view.zoom != ROOT)
}

/// Edit `id`: its content at `at` (its end when `None`), or its note.
pub fn start_editing(id: &str, at: Option<u32>) {
    store::edit(|s| {
        let at = at.unwrap_or_else(|| content_len(s.tree(), id));
        place_caret(s, Caret::text(id, at));
    });
}

/// Edit the note of `id`, at its end.
pub fn start_editing_note(id: &str) {
    store::edit(|s| {
        let at = utf16_len(item::note(s.tree().get(id)));
        place_caret(s, Caret::note(id, at));
    });
}

/// A press below the rows (`Editor.focusEnd`): the caret at the end of the last row, or the first item of an
/// empty document, made and edited.
pub fn focus_end() {
    if let Some(last) = store::peek(|s| s.rows.last().map(|r| r.id.clone())) {
        start_editing(&last, None);
        return;
    }
    run(|tx, v, _, _| {
        let id = tx.create(&v.zoom, None, outliner_core::Props::new());
        Some(CmdResult { sel: core::caret(&id, 0), zoom: None })
    });
}

pub fn undo(redo: bool) {
    store::edit(|s| {
        let (sel, change) = if redo { s.session.redo() } else { s.session.undo() };
        if change.is_some() {
            s.reflow();
            match sel {
                Some(Some(Mark::Caret(c))) if s.tree().is_attached(&c.id) => place_caret(s, c),
                Some(Some(Mark::Block(b))) if b.ids.iter().all(|id| s.tree().is_attached(id)) => {
                    place(s, Some(Mark::Block(b)))
                }
                _ => {}
            }
        }
    });
}

// ---- the selected items (block selection) ----------------------------------------------------------------

/// Escape while editing: nothing edited, the caret remembered for a second Escape.
pub fn stop_editing() {
    store::edit(|s| {
        s.last_sel = s.editing.take();
        s.caret_rev += 1;
    });
}

/// Escape with nothing edited and nothing selected: back where the last Escape left. Returns whether it did.
pub fn resume() -> bool {
    store::edit(|s| match s.last_sel.take() {
        Some(c) if s.rows.iter().any(|r| r.id == c.id) => {
            place_caret(s, c);
            true
        }
        _ => false,
    })
}

/// Escape on selected items: nothing selected; a second Escape edits the first of them, at its start (Dynalist).
pub fn clear_block() {
    store::edit(|s| {
        if let Some(b) = s.block.take() {
            s.last_sel = b.ids.first().map(|id| Caret::text(id, 0));
        }
        s.caret_rev += 1;
    });
}

/// Shift+Up / Shift+Down (`extendBlock`): from the row being edited, selects it (the caller checks the caret is
/// on the input's first / last line); with items selected, moves the selection's focus to the sibling above or
/// below, or out to the parent / the next item after it.
pub fn extend(down: bool) {
    store::edit(|s| {
        let v = s.view.clone();
        let t = &s.session.live;
        let mark = match (&s.block, &s.editing) {
            (Some(b), _) => {
                let next = if down {
                    view::next_visible_sibling(t, &b.focus, &v).or_else(|| view::next_visible_non_descendant(t, &b.focus, &v))
                } else {
                    view::prev_visible_sibling(t, &b.focus, &v)
                        .or_else(|| t.get(&b.focus).and_then(|n| n.parent.clone()).filter(|p| *p != v.zoom))
                };
                let Some(next) = next else { return };
                let ids = view::select_between(t, &b.anchor, &next, &v);
                Block { ids, anchor: b.anchor.clone(), focus: next }
            }
            (None, Some(c)) if c.id != v.zoom => Block { ids: vec![c.id.clone()], anchor: c.id.clone(), focus: c.id.clone() },
            _ => return,
        };
        place(s, Some(Mark::Block(mark)));
    });
}

/// Mod+Shift+A: every item at the top of the view selected.
pub fn select_all() {
    store::edit(|s| {
        let kids = view::visible_children(s.tree(), &s.view.zoom, &s.view);
        if let (Some(first), Some(last)) = (kids.first().cloned(), kids.last().cloned()) {
            place(s, Some(Mark::Block(Block { ids: kids, anchor: first, focus: last })));
        }
    });
}

/// Mod+A on selected items (`progressiveSelectAll`): all their siblings, then their parent.
pub fn select_more() {
    store::edit(|s| {
        let Some(b) = s.block.clone() else { return };
        let t = s.tree();
        let Some(parent) = b.ids.first().and_then(|id| t.get(id)).and_then(|n| n.parent.clone()) else { return };
        let sibs = view::visible_children(t, &parent, &s.view);
        let mark = if sibs.len() != b.ids.len() {
            Block { anchor: sibs[0].clone(), focus: sibs[sibs.len() - 1].clone(), ids: sibs }
        } else if parent != s.view.zoom {
            Block { ids: vec![parent.clone()], anchor: parent.clone(), focus: parent }
        } else {
            return;
        };
        place(s, Some(Mark::Block(mark)));
    });
}

/// Arrows on selected items: Up edits the row above the first, Down the row after the last (or the last), at
/// their start; Left the first at its start, Right the end of the last row under the selection.
pub fn leave_block(key: &str) {
    store::edit(|s| {
        let Some(b) = s.block.clone() else { return };
        let (t, v) = (&s.session.live, &s.view);
        let (first, last) = (b.ids[0].clone(), b.ids[b.ids.len() - 1].clone());
        let c = match key {
            "up" => match view::prev_visible(t, &first, v).filter(|p| *p != v.zoom) {
                Some(p) => Caret::text(&p, 0),
                None => return,
            },
            "down" => Caret::text(&view::next_visible_non_descendant(t, &last, v).unwrap_or(last), 0),
            "left" => Caret::text(&first, 0),
            _ => {
                let end = view::last_visible_descendant(t, &last, v);
                let at = content_len(t, &end);
                Caret::text(&end, at)
            }
        };
        place_caret(s, c);
    });
}

/// A press on a row in select mode (and Mod+press on desktops): the item joins the selection or leaves it.
pub fn toggle_in_block(id: &str) {
    store::edit(|s| {
        if id == s.view.zoom {
            return;
        }
        let cur: Vec<NodeId> = match (&s.block, &s.editing) {
            (Some(b), _) => b.ids.clone(),
            (None, Some(c)) if c.id != s.view.zoom => vec![c.id.clone()],
            _ => Vec::new(),
        };
        let ids = if cur.iter().any(|x| x == id) {
            cur.into_iter().filter(|x| x != id).collect()
        } else {
            let mut all = cur;
            all.push(id.to_owned());
            view::normalize_block(s.tree(), &all)
        };
        let mark = match (ids.first().cloned(), ids.last().cloned()) {
            (Some(a), Some(f)) => Some(Mark::Block(Block { ids, anchor: a, focus: f })),
            _ => None,
        };
        place(s, mark);
    });
}

/// The phone's "Select multiple items" button: select mode on or off, nothing selected or edited.
pub fn set_select_mode(on: bool) {
    store::edit(|s| {
        s.select_mode = on;
        place(s, None);
    });
}

/// The plain text of the selected items, as the web copies it (`portableToPlain`): one line per item, its note
/// on the next, no indentation, children of numbered items as `N. `.
pub fn block_text() -> Option<String> {
    store::peek(|s| {
        let b = s.block.as_ref()?;
        let t = s.tree();
        let items: Vec<core::Portable> = b.ids.iter().map(|id| core::to_portable(t, id, Some(&s.view))).collect();
        let numbered = b.ids.first().and_then(|id| t.get(id)).and_then(|n| n.parent.as_deref()).is_some_and(|p| item::numbered(t.get(p)));
        let mut out = String::new();
        plain(&items, numbered, &mut out);
        Some(out)
    })
}

fn plain(items: &[core::Portable], numbered: bool, out: &mut String) {
    for (i, it) in items.iter().enumerate() {
        if numbered {
            out.push_str(&format!("{}. ", i + 1));
        }
        out.push_str(it.props.get("c").and_then(Value::as_str).unwrap_or(""));
        out.push('\n');
        if let Some(n) = it.props.get("n").and_then(Value::as_str).filter(|n| !n.is_empty()) {
            out.push_str(n);
            out.push('\n');
        }
        let num = it.props.get("num").is_some_and(|v| v.as_bool().unwrap_or(false) || v.as_i64().is_some_and(|x| x != 0));
        plain(&it.children, num, out);
    }
}

/// Mod+C / Mod+X on selected items: their text to the clipboard (cut: the items deleted).
pub fn copy_block(cut: bool) {
    if let Some(text) = block_text() {
        nsh_core::prelude::set_clipboard(text);
        if cut {
            delete_items(Dir::Left);
        }
    }
}

/// A press on an item's checkbox: checks it or unchecks it, the selection staying.
pub fn check(id: &str) {
    run(|tx, v, _, sel| {
        let opts = CheckOptions { stay: true, recurring: Some(recurring()), now: None };
        core::toggle_checked(tx, v, &[id.to_owned()], sel, &opts)
    });
}
