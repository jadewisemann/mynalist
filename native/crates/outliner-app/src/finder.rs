//! The web's fuzzy finder (`finder` in `apps/web/src/ui/overlay.ts`, `.modal.finder`): an input over a list of
//! choices ranked by the fuzzy search of the core (`fuzzy_filter`), the first 50; Up and Down move the active one,
//! Return or a press takes it, Escape or a press outside closes it. The file finder (Mod+O, the Files pane's
//! search button), the item finder (Mod+Shift+O), "Move item to…", "Insert template…" and the inbox picker of the
//! settings use it.

use std::rc::Rc;

use nsh_core::keymap::When;
use nsh_core::prelude::*;
use nsh_core::LineHeight;

use outliner_core::{commands, item, NodeId, Tx, ROOT};

use crate::dialog;
use crate::sidebar::ui_text;
use crate::store::{self, Mark};
use crate::style::{self, color, num};

/// A choice of the finder: its label, a muted line under it, and what taking it does.
#[derive(Clone)]
pub struct Choice {
    pub label: String,
    pub sub: String,
    pub run: Rc<dyn Fn()>,
}

impl Choice {
    pub fn new(label: impl Into<String>, sub: impl Into<String>, run: impl Fn() + 'static) -> Choice {
        Choice { label: label.into(), sub: sub.into(), run: Rc::new(run) }
    }
}

/// How many choices show (`slice(0, 50)`).
const SHOWN: usize = 50;

/// The choices `all` matching `q`, best first (`fuzzyFilter`), the first [`SHOWN`].
pub fn rank(q: &str, all: &[Choice]) -> Vec<Choice> {
    outliner_core::search::fuzzy_filter(q, all, |c| c.label.as_str())
        .into_iter()
        .take(SHOWN)
        .map(|(c, _)| c.clone())
        .collect()
}

#[derive(Clone, Default)]
struct State {
    query: String,
    active: usize,
    shown: Vec<Choice>,
}

impl PartialEq for State {
    fn eq(&self, o: &Self) -> bool {
        self.query == o.query
            && self.active == o.active
            && self.shown.len() == o.shown.len()
            && self.shown.iter().zip(&o.shown).all(|(a, b)| Rc::ptr_eq(&a.run, &b.run))
    }
}

/// Opens the finder with `placeholder` in its input over `all` (made when it opens).
pub fn open(placeholder: &'static str, all: Vec<Choice>) {
    let all = Rc::new(all);
    // under no view's owner: what opened it (a menu, a key) may go away
    let state = Owner::detached().run(|| signal(State { query: String::new(), active: 0, shown: rank("", &all) }));
    dialog::show(move || finder(placeholder, all.clone(), state).into_element());
}

fn take(state: Signal<State>) {
    let chosen = state.with_untracked(|s| s.shown.get(s.active).cloned());
    dialog::close();
    if let Some(c) = chosen {
        (c.run)();
    }
}

fn finder(placeholder: &'static str, all: Rc<Vec<Choice>>, state: Signal<State>) -> impl IntoElement {
    let opts = KeyOptions { when: When::Always, ..Default::default() };
    on_key_with("down", opts, move |_| state.update(|s| s.active = (s.active + 1).min(s.shown.len().saturating_sub(1))));
    on_key_with("up", opts, move |_| state.update(|s| s.active = s.active.saturating_sub(1)));
    on_key_with("enter", opts, move |_| take(state));
    let size = num("type.finder");
    let input = nsh_base::input()
        .test_id("finder-input")
        .placeholder(placeholder)
        .auto_focus(true)
        .auto_correct(false)
        .font_family(style::font())
        .text_size(px(size))
        .line_height(LineHeight::Points(px(size * num("type.ui_line"))))
        .text_color(color::fg())
        .bg(AdaptiveColor::TRANSPARENT)
        .border(px(0.0))
        .border_b(px(num("space.border")))
        .border_color(color::line())
        .px(px(num("space.finder_x")))
        .py(px(num("space.finder_y")))
        .value(move || state.with(|s| s.query.clone()))
        .on_change(move |q: &str| {
            let shown = rank(q, &all);
            state.set(State { query: q.to_owned(), active: 0, shown });
        })
        .on_submit(move |_: &str| take(state));
    let (size, line) = style::ui();
    let tiny = style::ui_sized("type.ui_tiny");
    let sel = color::sel();
    let rows = move || {
        let (shown, active) = state.with(|s| (s.shown.clone(), s.active));
        shown
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let sub = (!c.sub.is_empty()).then(|| ui_text(c.sub.clone(), tiny).text_color(color::muted()));
                nsh_base::button()
                    .accessibility_label(Some(c.label.clone().into()))
                    .flex_col()
                    .items_start()
                    .px(px(num("space.finder_x")))
                    .py(px(num("space.finder_res_y")))
                    .bg(if i == active { sel.into() } else { AdaptiveColor::TRANSPARENT })
                    .on_press(move || {
                        state.update(|s| s.active = i);
                        take(state);
                    })
                    .child(ui_text(c.label.clone(), (size, line)).text_color(color::fg()))
                    .children(sub)
                    .into_element()
            })
            .collect::<Vec<_>>()
    };
    dialog::frame("finder").child(input).child(scroll().flex_shrink(1.0).child(dynamic(move || view().children(rows()))))
}

/// The documents and bookmarks to jump to (`goAnywhere`): a document opens in the zoom it was left in, a
/// bookmark as it was made.
pub fn go_anywhere() {
    let docs: Vec<(String, String)> = store::peek(|s| s.files.iter().map(|d| (d.clone(), s.title(d))).collect());
    let mut all: Vec<Choice> = docs
        .into_iter()
        .map(|(id, title)| {
            // the folders it is in (`path`), and whether it is archived
            let path = crate::workspace::path_of(&id);
            let archived = crate::workspace::get(&id).is_some_and(|e| e.archived);
            let sub = if archived { format!("{} (archived)", if path.is_empty() { "Document" } else { &path }) } else { path };
            Choice::new(title, sub, move || crate::sidebar::open_doc(&id))
        })
        .collect();
    for b in crate::workspace::bookmarks_now() {
        let title = format!("★ {}", b.title);
        all.push(Choice::new(title, "Bookmark", move || crate::sidebar::open_bookmark(&b)));
    }
    open("Search any document or bookmark to jump to…", all);
}

/// "Move item to…" (`moveToDialog`): the item edited or the selected items, moved under a document or an item
/// chosen in the finder (not under themselves), at the end of its children (or the start: `move_item_sent_to_position`);
/// into another document they are copied there and deleted here, as on the web.
pub fn move_to() {
    let picked = store::peek(|s| {
        let ids = match (&s.block, &s.editing) {
            (Some(b), _) => b.ids.clone(),
            (None, Some(c)) if c.id != s.view.zoom => vec![c.id.clone()],
            _ => Vec::new(),
        };
        (!ids.is_empty()).then(|| (s.doc.clone(), ids))
    });
    let Some((src, ids)) = picked else { return };
    move_items_to(ids, src);
}

/// "Move to…" of items `ids` of document `src` (the item menu's, the keymap's).
pub fn move_items_to(ids: Vec<NodeId>, src: String) {
    if ids.is_empty() || store::peek(|s| crate::docview::is_locked(&s.doc)) {
        return;
    }
    let bottom = crate::settings::get().pick("move_item_sent_to_position") != "top";
    let ids = Rc::new(ids);
    let mut all = Vec::new();
    store::peek(|s| {
        for d in &s.files {
            let Some(t) = s.tree_of(d) else { continue };
            let title = s.title(d);
            let to = |id: &str, label: String| {
                let (src, ids, doc, id) = (src.clone(), ids.clone(), d.clone(), id.to_owned());
                move || move_into(&src, &ids, &doc, &id, &label, bottom)
            };
            all.push(Choice::new(title.clone(), "Document", to(ROOT, title.clone())));
            let mut stack: Vec<NodeId> = t.get(ROOT).map(|n| n.children.iter().rev().cloned().collect()).unwrap_or_default();
            while let Some(id) = stack.pop() {
                let Some(n) = t.get(&id) else { continue };
                stack.extend(n.children.iter().rev().cloned());
                let c = item::content(Some(n));
                if c.is_empty() || (*d == src && ids.iter().any(|x| *x == id || t.is_ancestor(x, &id))) {
                    continue;
                }
                let sub = format!("{title} › {}", crate::find::breadcrumb(t, &id, ROOT));
                all.push(Choice::new(c, sub, to(&id, c.to_owned())));
            }
        }
    });
    open("Move to…", all);
}

/// Moves `ids` of document `src` under `target` of document `doc`, last among its children (first unless
/// `bottom`).
fn move_into(src: &str, ids: &[NodeId], doc: &str, target: &str, label: &str, bottom: bool) {
    store::edit(|s| {
        let before = match (&s.block, &s.editing) {
            (Some(b), _) => Some(Mark::Block(b.clone())),
            (None, Some(c)) => Some(Mark::Caret(c.clone())),
            _ => None,
        };
        let view = s.view.clone();
        if src == doc {
            let Some(session) = s.session_mut(doc) else { return };
            let mut after = if bottom { session.live.get(target).and_then(|n| n.children.last().cloned()) } else { None };
            let (ops, inverse) = {
                let mut tx = Tx::new(&mut session.live, store::now());
                for id in ids {
                    tx.move_to(id, Some(target), after.as_deref());
                    after = Some(id.clone());
                }
                (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
            };
            session.commit(ops, inverse, before, None, true);
        } else {
            let Some(items) = s.tree_of(src).map(|t| ids.iter().map(|id| commands::to_portable(t, id, None)).collect::<Vec<_>>())
            else {
                return;
            };
            if let Some(dest) = s.session_mut(doc) {
                let after = if bottom { dest.live.get(target).and_then(|n| n.children.last().cloned()) } else { None };
                let (ops, inverse) = {
                    let mut tx = Tx::new(&mut dest.live, store::now());
                    commands::insert_portable(&mut tx, target, after.as_deref(), &items);
                    (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
                };
                dest.commit(ops, inverse, None, None, true);
            }
            if let Some(from) = s.session_mut(src) {
                let (ops, inverse) = {
                    let mut tx = Tx::new(&mut from.live, store::now());
                    commands::delete_items(&mut tx, &view, ids, commands::Dir::Left);
                    (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
                };
                from.commit(ops, inverse, before, None, true);
            }
        }
        s.editing = None;
        s.block = None;
        s.caret_rev += 1;
        s.reflow();
    });
    crate::workspace::changed(doc);
    crate::toast::show(format!("Moved to {label}"));
}

/// Items `ids` of the open document dragged onto the Files pane (`dropItems`): onto document `doc`, they go last
/// among its top items (first: `move_item_sent_to_position`), copied there and deleted here unless `copy`; onto
/// the pane itself (`None`), each becomes a new document in the open one's folder, named after it, holding its
/// children.
pub fn drop_items(doc: Option<&str>, ids: &[NodeId], copy: bool) {
    let src = store::peek(|s| s.doc.clone());
    if ids.is_empty() || crate::docview::is_locked(&src) {
        return;
    }
    let bottom = crate::settings::get().pick("move_item_sent_to_position") != "top";
    let Some(items) = store::peek(|s| {
        let t = s.tree();
        ids.iter().all(|id| t.get(id).is_some()).then(|| ids.iter().map(|id| commands::to_portable(t, id, None)).collect::<Vec<_>>())
    }) else {
        return;
    };
    let message = match doc {
        Some(d) if d == src && !copy => {
            // within the document: to its top level
            store::edit(|s| {
                let Some(session) = s.session_mut(&src) else { return };
                let mut after = if bottom {
                    session.live.get(ROOT).and_then(|n| n.children.iter().rfind(|c| !ids.contains(c)).cloned())
                } else {
                    None
                };
                let (ops, inverse) = {
                    let mut tx = Tx::new(&mut session.live, store::now());
                    for id in ids {
                        tx.move_to(id, Some(ROOT), after.as_deref());
                        after = Some(id.clone());
                    }
                    (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
                };
                session.commit(ops, inverse, None, None, true);
                s.editing = None;
                s.block = None;
                s.caret_rev += 1;
                s.reflow();
            });
            crate::workspace::changed(&src);
            return;
        }
        Some(d) if d == src => {
            // a copy within the document, at its top level
            insert_into(d, &items, bottom);
            return;
        }
        Some(d) => {
            insert_into(d, &items, bottom);
            let what = if copy { "Copied" } else { "Moved" };
            Some(format!("{what} to {}", store::peek(|s| s.title(d))))
        }
        None => {
            let folder = crate::workspace::parent_of(&src);
            for it in &items {
                let plain = outliner_core::inline::inline_to_plain(it.props.get("c").and_then(|v| v.as_str()).unwrap_or(""), None);
                let title = if plain.is_empty() { "Untitled".to_owned() } else { plain };
                let id = crate::workspace::create(crate::workspace::Kind::Doc, folder.as_deref(), &title);
                insert_into(&id, &it.children, true);
            }
            Some(if items.len() == 1 { "Created a document from the item".to_owned() } else { format!("Created {} documents", items.len()) })
        }
    };
    if !copy {
        store::edit(|s| {
            let view = s.view.clone();
            let Some(from) = s.session_mut(&src) else { return };
            let (ops, inverse) = {
                let mut tx = Tx::new(&mut from.live, store::now());
                commands::delete_items(&mut tx, &view, ids, commands::Dir::Left);
                (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
            };
            from.commit(ops, inverse, None, None, true);
            s.editing = None;
            s.block = None;
            s.caret_rev += 1;
            s.reflow();
        });
        crate::workspace::changed(&src);
    }
    if let Some(m) = message {
        crate::toast::show(m);
    }
}

/// Copies `items` into document `doc`'s top level (`copyInto`): last, or first unless `bottom`.
fn insert_into(doc: &str, items: &[commands::Portable], bottom: bool) {
    store::edit(|s| {
        let Some(dest) = s.session_mut(doc) else { return };
        let after = if bottom { dest.live.get(ROOT).and_then(|n| n.children.last().cloned()) } else { None };
        let (ops, inverse) = {
            let mut tx = Tx::new(&mut dest.live, store::now());
            commands::insert_portable(&mut tx, ROOT, after.as_deref(), items);
            (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
        };
        dest.commit(ops, inverse, None, None, true);
        s.reflow();
    });
    crate::workspace::changed(doc);
}

/// An item of a document with some text, for the item finders: its document, id, text and the line under it.
#[derive(Clone)]
struct ItemAt {
    doc: String,
    id: NodeId,
    text: String,
    sub: String,
}

/// The items with some text in the documents (`allItems`), each document's in document order.
fn all_items() -> Vec<ItemAt> {
    store::peek(|s| {
        let mut out = Vec::new();
        for d in &s.files {
            let Some(t) = s.tree_of(d) else { continue };
            let title = s.title(d);
            let mut stack: Vec<NodeId> = t.get(ROOT).map(|n| n.children.iter().rev().cloned().collect()).unwrap_or_default();
            while let Some(id) = stack.pop() {
                let Some(n) = t.get(&id) else { continue };
                let text = item::content(Some(n));
                if !text.is_empty() {
                    let sub = format!("{title} › {}", crate::find::breadcrumb(t, &id, ROOT));
                    out.push(ItemAt { doc: d.clone(), id: id.clone(), text: text.to_owned(), sub });
                }
                stack.extend(n.children.iter().rev().cloned());
            }
        }
        out
    })
}

/// The finder's choices for items `all`, each doing `then` with the item.
fn item_choices(all: Vec<ItemAt>, then: impl Fn(&ItemAt) + 'static) -> Vec<Choice> {
    let then = Rc::new(then);
    all.into_iter()
        .map(|x| {
            let then = then.clone();
            let (label, sub) = (x.text.clone(), x.sub.clone());
            Choice::new(label, sub, move || then(&x))
        })
        .collect()
}

/// The item finder (`itemFinder`): an item of any document, shown where it is.
pub fn goto_item() {
    open("Jump to any item…", item_choices(all_items(), |x| crate::find::reveal(&x.doc, &x.id)));
}

/// "Insert template…" (`templateDialog`): a copy of the chosen item (with what is under it) goes after the item
/// acted on (in its place when it is empty; under the title, last), edited at its start.
pub fn insert_template() {
    let Some(target) = crate::commands::exec_targets().into_iter().next() else { return };
    open(
        "Insert template: pick an item to copy here…",
        item_choices(all_items(), move |x| {
            let item = store::peek(|s| s.tree_of(&x.doc).filter(|t| t.contains(&x.id)).map(|t| commands::to_portable(t, &x.id, None)));
            if let Some(item) = item {
                crate::commands::insert_template(&target, item);
            }
        }),
    );
}
