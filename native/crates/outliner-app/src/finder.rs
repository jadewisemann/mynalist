//! The web's fuzzy finder (`finder` in `apps/web/src/ui/overlay.ts`, `.modal.finder`): an input over a list of
//! choices ranked by the fuzzy search of the core (`fuzzy_filter`), the first 50; Up and Down move the active one,
//! Return or a press takes it, Escape or a press outside closes it. The file finder (Mod+O, the Files pane's
//! search button), "Move item to…" and the inbox picker of the settings use it.

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
            let sub = crate::workspace::parent_of(&id).and_then(|f| crate::workspace::get(&f)).map(|f| f.title).unwrap_or_default();
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
/// chosen in the finder (not under themselves), at the end of its children (`move_item_sent_to_position`'s
/// default); into another document they are copied there and deleted here, as on the web.
pub fn move_to() {
    let picked = store::peek(|s| {
        if crate::docview::is_locked(&s.doc) {
            return None;
        }
        let ids = match (&s.block, &s.editing) {
            (Some(b), _) => b.ids.clone(),
            (None, Some(c)) if c.id != s.view.zoom => vec![c.id.clone()],
            _ => Vec::new(),
        };
        (!ids.is_empty()).then(|| (s.doc.clone(), ids))
    });
    let Some((src, ids)) = picked else { return };
    let ids = Rc::new(ids);
    let mut all = Vec::new();
    store::peek(|s| {
        for d in &s.files {
            let Some(t) = s.tree_of(d) else { continue };
            let title = s.title(d);
            let to = |id: &str, label: String| {
                let (src, ids, doc, id) = (src.clone(), ids.clone(), d.clone(), id.to_owned());
                move || move_into(&src, &ids, &doc, &id, &label)
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

/// Moves `ids` of document `src` under `target` of document `doc`, last among its children.
fn move_into(src: &str, ids: &[NodeId], doc: &str, target: &str, label: &str) {
    store::edit(|s| {
        let before = match (&s.block, &s.editing) {
            (Some(b), _) => Some(Mark::Block(b.clone())),
            (None, Some(c)) => Some(Mark::Caret(c.clone())),
            _ => None,
        };
        let view = s.view.clone();
        if src == doc {
            let Some(session) = s.session_mut(doc) else { return };
            let mut after = session.live.get(target).and_then(|n| n.children.last().cloned());
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
                let after = dest.live.get(target).and_then(|n| n.children.last().cloned());
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
