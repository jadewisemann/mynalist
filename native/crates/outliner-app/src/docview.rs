//! A document's view options, as the web keeps them on the document's entry in the workspace (`docPref`,
//! `v_<key>`): read mode (`locked`: the document can be read, navigated and folded, not edited) and the layout
//! (`layout`: the list, or the mind map of `mindmap.rs`). The web's "View options" menu sets them (`viewOptions`
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
/// The mind map layout's value (`Mind map (read-only)`).
pub const MINDMAP: &str = "mindmap";

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

/// The open document's layout (tracked): "list" or [`MINDMAP`].
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

/// The "View options" menu's entries the native app has: the layout and the lock, the chosen ones ticked (the
/// web shows them as submenus with checks).
pub fn options() -> Vec<menu::Item> {
    let doc = store::peek(|s| s.doc.clone());
    let current = match workspace::doc_pref_now(&doc, LAYOUT) {
        Some(Value::String(l)) => l,
        _ => "list".to_owned(),
    };
    let tick = |on: bool, label: &str| if on { format!("✓ {label}") } else { label.to_owned() };
    vec![
        menu::item(tick(current == "list", "Layout: List"), || set_layout("list")),
        menu::item(tick(current == MINDMAP, "Layout: Mind map (read-only)"), || set_layout(MINDMAP)),
        menu::sep(),
        menu::item(tick(is_locked(&doc), "Lock document (read-only)"), toggle_lock),
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
