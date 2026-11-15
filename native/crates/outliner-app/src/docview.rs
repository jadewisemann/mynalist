//! A document's view options, as the web keeps them on the document's entry in the workspace (`docPref`,
//! `v_<key>`): read mode (`locked`: the document can be read, navigated and folded, not edited), the layout
//! (`layout`: the list, the article, or the mind map of `mindmap.rs`), whether checked items show (`checked`) and
//! how notes do (`notes`), the last two "global" (the settings) unless chosen. The web's "View options" menu sets them (`viewOptions`
//! in main.ts), opened from the doc actions beside the search box on desktops (`.doc-actions`: bookmark this
//! view, view options); phones lock a document with the top bar's lock button.

use nsh_core::prelude::*;
use serde_json::Value;

use crate::icons::icon;
use crate::menu;
use crate::store;
use crate::style::{self, color, num};
use crate::workspace;

const LOCKED: &str = "locked";
const LAYOUT: &str = "layout";
const CHECKED: &str = "checked";
const NOTES: &str = "notes";
/// A choice that follows the setting (`global`).
const GLOBAL: &str = "global";
/// The mind map layout's value (`Mind map (read-only)`).
pub const MINDMAP: &str = "mindmap";
/// The article layout's value (`.o-article`: no bullets but under the pointer or on folded items, no guides or
/// thread, more room above and below the rows).
pub const ARTICLE: &str = "article";

/// Whether document `doc` is locked, untracked (the commands' check).
pub fn is_locked(doc: &str) -> bool {
    workspace::doc_pref_now(doc, LOCKED) == Some(Value::Bool(true))
}

/// Whether the open document is locked (tracked).
pub fn locked() -> bool {
    let doc = store::read(|s| s.doc.clone());
    workspace::doc_pref(&doc, LOCKED) == Some(Value::Bool(true))
}

/// Locks the open document, or unlocks it (`toggleLock`); locking it ends its editing (`setReadOnly`).
pub fn toggle_lock() {
    let doc = store::peek(|s| s.doc.clone());
    let on = !is_locked(&doc);
    workspace::set_doc_pref(&doc, LOCKED, on.then_some(Value::Bool(true)));
    if on {
        store::edit(|s| {
            s.editing = None;
            s.caret_rev += 1;
        });
    }
}

/// The open document's layout (tracked): "list", [`ARTICLE`] or [`MINDMAP`].
pub fn layout() -> String {
    let doc = store::read(|s| s.doc.clone());
    match workspace::doc_pref(&doc, LAYOUT) {
        Some(Value::String(l)) => l,
        _ => "list".to_owned(),
    }
}

/// Sets the open document's layout; the default (`list`) is not kept.
pub fn set_layout(value: &str) {
    let doc = store::peek(|s| s.doc.clone());
    workspace::set_doc_pref(&doc, LAYOUT, (value != "list").then(|| Value::String(value.to_owned())));
}

/// Document `doc`'s choice for view option `key` (a string), or `fallback` when it has none.
fn choice(doc: &str, key: &str, fallback: &str, tracked: bool) -> String {
    let v = if tracked { workspace::doc_pref(doc, key) } else { workspace::doc_pref_now(doc, key) };
    match v {
        Some(Value::String(v)) => v,
        _ => fallback.to_owned(),
    }
}

/// Whether document `doc` shows its checked items (`docShowChecked`): its own choice, else the "Show checked
/// items" setting. Untracked.
pub fn shows_checked(doc: &str) -> bool {
    match choice(doc, CHECKED, GLOBAL, false).as_str() {
        GLOBAL => crate::settings::get().flag("show_checked"),
        v => v == "show",
    }
}

/// How the open document shows notes (`docNotes`), tracked: "show", "teaser" (the first line) or "hide"; its own
/// choice, else the "Notes" setting.
pub fn notes() -> String {
    let doc = store::read(|s| s.doc.clone());
    match choice(&doc, NOTES, GLOBAL, true).as_str() {
        GLOBAL => crate::settings::prefs().pick("show_notes").to_owned(),
        v => v.to_owned(),
    }
}

/// Applies the open document's "Checked items" to its view (an effect of the app's root: it runs again when the
/// document, its choice or the setting changes).
pub fn apply() {
    let doc = store::read(|s| s.doc.clone());
    let global = crate::settings::prefs().flag("show_checked");
    let shown = match choice(&doc, CHECKED, GLOBAL, true).as_str() {
        GLOBAL => global,
        v => v == "show",
    };
    if store::peek(|s| s.show_checked != shown) {
        store::edit(|s| {
            s.show_checked = shown;
            s.reflow();
        });
    }
}

/// The "View options" menu (`viewOptions`): checked items, notes and layout as submenus whose chosen entry is
/// ticked (the first entry is the default, not kept), and the lock. The web's "Text direction" is left out: the
/// native app has no right-to-left layout.
pub fn options() -> Vec<menu::Item> {
    let doc = store::peek(|s| s.doc.clone());
    let opt = |key: &'static str, choices: &[(&'static str, &'static str)], global: Option<String>| -> Vec<menu::Item> {
        let first = choices[0].0;
        let cur = choice(&doc, key, first, false);
        choices
            .iter()
            .map(|&(v, l)| {
                let label = match &global {
                    Some(g) if v == GLOBAL => format!("{l} ({g})"),
                    _ => l.to_owned(),
                };
                let label = if cur == v { format!("✓ {label}") } else { label };
                let doc = doc.clone();
                menu::item(label, move || workspace::set_doc_pref(&doc, key, (v != first).then(|| Value::String(v.to_owned()))))
            })
            .collect()
    };
    let p = crate::settings::get();
    let g_checked = if p.flag("show_checked") { "show" } else { "hide" };
    let g_notes = p.pick("show_notes").to_owned();
    let locked = is_locked(&doc);
    vec![
        menu::sub("Checked items", opt(CHECKED, &[(GLOBAL, "Global"), ("show", "Show"), ("hide", "Hide")], Some(g_checked.to_owned()))),
        menu::sub("Notes", opt(NOTES, &[(GLOBAL, "Global"), ("show", "Show"), ("teaser", "1st line"), ("hide", "Hide")], Some(g_notes))),
        menu::sub("Layout", opt(LAYOUT, &[("list", "List"), (ARTICLE, "Article"), (MINDMAP, "Mind map (read-only)")], None)),
        menu::sep(),
        menu::item(if locked { "✓ Lock document (read-only)" } else { "Lock document (read-only)" }, toggle_lock),
    ]
}

/// The doc actions at the document's top right on desktops (`.doc-actions`): bookmark this view, view options.
pub fn actions() -> impl IntoElement {
    // views, not buttons: the menu goes where the press was (`id:bookmark-view`, `id:view-options` in tests)
    let button = |name: &'static str, id: &'static str, run: fn(&PressEvent)| {
        let hover = color::hover();
        view()
            .test_id(id)
            .h(px(num("space.topbar_tool")))
            .min_w(px(num("space.topbar_tool_min")))
            .px(px(num("space.topbar_tool_pad")))
            .rounded(px(num("radius.topbar_tool")))
            .items_center()
            .justify_center()
            .text_color(color::topbar_fg())
            .hover(move |s| s.bg(hover))
            .on_press(run)
            .child(icon(name, num("space.topbar_icon")))
    };
    when(
        || !style::phone(),
        move || {
            view()
                .test_id("doc-actions")
                .absolute()
                .top(px(num("space.step5")))
                .right(px(num("space.step8")))
                .flex_row()
                .gap(px(num("space.step2")))
                .child(button("star", "bookmark-view", |_| crate::sidebar::bookmark_current()))
                .child(button("eye", "view-options", |p: &PressEvent| {
                    // under the button, its right edge near the pointer (`r.right - 220`)
                    let x = p.page_position.x.0 - num("space.menu_min");
                    menu::show(x.max(0.0), p.page_position.y.0 + num("space.step8"), options());
                }))
        },
    )
}
