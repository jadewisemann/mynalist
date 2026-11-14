//! The outliner's UI for Android and macOS, on Native Shell (NSH): `nsh.json` `"root"` names [`App`].
//!
//! This is the spike: a large demo document in a virtualized list, rows with inline formatting, editing in a
//! native input with the outliner's keys, and a toolbar for touch screens. The document model, undo and sync are
//! `outliner-core`, the Rust port of `packages/core`.

pub mod commands;
mod icons;
pub mod inline;
mod outline;
pub mod store;
mod style;
mod toolbar;

use nsh_core::prelude::*;

/// The web app's fonts (Fira Sans and Fira Mono, SIL OFL: `assets/fonts/OFL.txt`), for hosts that do not read the
/// crate's assets (the macOS app passes them with its root: `Root::font`).
pub const FONTS: [&[u8]; 6] = [
    include_bytes!("../assets/fonts/fira-sans-latin-300-normal.woff2"),
    include_bytes!("../assets/fonts/fira-sans-latin-400-normal.woff2"),
    include_bytes!("../assets/fonts/fira-sans-latin-400-italic.woff2"),
    include_bytes!("../assets/fonts/fira-sans-latin-700-normal.woff2"),
    include_bytes!("../assets/fonts/fira-sans-latin-700-italic.woff2"),
    include_bytes!("../assets/fonts/fira-mono-latin-400-normal.woff2"),
];

/// Items in the demo document.
pub const DEMO_ITEMS: usize = 10_000;

#[native_module("outliner/native")]
fn native(registry: &mut Registry) {
    // the web app's fonts (Fira Sans and Fira Mono, SIL OFL: assets/fonts/OFL.txt)
    registry.assets(assets!());
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
        // the software keyboard's height: the document shrinks and the toolbar sits right on the keyboard
        .pb_keyboard()
        .bg(style::color::bg())
        .child(toolbar::topbar())
        .child(outline::outline())
        .child(toolbar::mobile_bar())
        // the home indicator's room, only while the keyboard does not cover it
        .child(when(|| keyboard_height() <= px(0.0), || view().pb_safe_area()))
}
