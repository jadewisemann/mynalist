//! The outliner's UI for Android and macOS, on Native Shell (NSH): `nsh.json` `"root"` names [`App`].
//!
//! This is the spike: a large demo document in a virtualized list, rows with inline formatting, editing in a
//! native input with the outliner's keys, and a toolbar for touch screens. The document model, undo and sync are
//! `outliner-core`, the Rust port of `packages/core`.

pub mod commands;
pub mod inline;
mod outline;
pub mod store;
mod toolbar;

use nsh_base::theme;
use nsh_core::prelude::*;

/// Items in the demo document.
pub const DEMO_ITEMS: usize = 10_000;

#[native_module("outliner/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

#[component]
pub fn App() -> impl IntoElement {
    if store::peek(|s| s.rows.is_empty()) {
        store::open(store::demo(DEMO_ITEMS));
    }
    view()
        .flex_1()
        .pt_safe_area()
        .bg(move || theme().background)
        .child(outline::outline())
        .child(toolbar::toolbar())
}
