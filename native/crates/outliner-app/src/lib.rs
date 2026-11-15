//! The outliner's UI for Android and macOS, on Native Shell (NSH): `nsh.json` `"root"` names [`App`].
//!
//! This is the spike: a large demo document in a virtualized list, rows with inline formatting, editing in a
//! native input with the outliner's keys, and a toolbar for touch screens. The document model, undo and sync are
//! `outliner-core`, the Rust port of `packages/core`.

pub mod appearance;
pub mod backup;
pub mod commands;
mod datepicker;
/// Generated from `design/*.toml` by `design/build.mjs`.
#[allow(clippy::excessive_precision, clippy::approx_constant, clippy::unreadable_literal)]
pub mod design;
mod dialog;
mod export;
pub mod find;
pub mod fixture;
mod icons;
pub mod inline;
pub mod keymap;
mod links;
mod outline;
pub mod settings;
mod sidebar;
pub mod store;
pub mod style;
mod toast;
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
    if store::is_blank() {
        // `OUTLINER_DEMO=inline`: the inline showcase instead of the large demo; an empty document beside it
        let inline = std::env::var("OUTLINER_DEMO").is_ok_and(|d| d == "inline");
        let first = if inline { store::showcase() } else { store::demo(DEMO_ITEMS) };
        store::open_all(vec![("doc1".into(), first), ("doc2".into(), store::named("Untitled"))]);
        settings::document_opened();
    }
    root()
}

/// The app's screen in the look the settings and the system's appearance choose: made again when the look
/// changes (its views read the look as plain values; what they show is kept in the store and the globals).
fn root() -> impl IntoElement {
    effect(settings::apply);
    view().flex_1().child(dynamic(|| {
        appearance::current();
        untracked(screen)
    }))
}

/// `part`, made again when the settings that are not the look's change (the look's make the whole screen again):
/// the outline and the keys read them as plain values.
fn behaving<E: IntoElement>(part: impl Fn() -> E + 'static) -> impl IntoElement {
    let prefs = computed(settings::behavior);
    dynamic(move || {
        prefs.get();
        untracked(&part).into_element()
    })
}

/// The keymap's app commands (`keymap::Scope::App`), whatever has the focus.
fn app_keys() -> impl IntoElement {
    let opts = KeyOptions { when: nsh_core::keymap::When::Always, ..Default::default() };
    for (keys, run) in keymap::bindings(keymap::Scope::App, &[]) {
        on_key_with(&keys, opts, move |_| run());
    }
    view()
}

/// The app's screen over the open document: the top bar, the sidebar's panel (desktops) beside the outline, the
/// phone toolbar, and the sidebar's drawer over all of it (phones).
fn screen() -> View {
    view()
        .flex_1()
        .pt_safe_area()
        // the software keyboard's height: the document shrinks and the toolbar sits right on the keyboard
        .pb_keyboard()
        .bg(style::color::bg())
        .child(toolbar::topbar())
        .child(view().flex_1().flex_row().child(sidebar::panel()).child(behaving(find::doc_area)))
        .child(toolbar::mobile_bar())
        // the keys of selected items, while there are some (no input has the focus then)
        .child(behaving(|| when(|| store::read(|s| s.block.is_some()), outline::block_keys)))
        // the app's keys (the keymap's: the settings, the sidebar, ...) and the word count
        .child(behaving(app_keys))
        .child(behaving(settings::status_bar))
        .child(datepicker::picker())
        // the home indicator's room, only while the keyboard does not cover it
        .child(when(|| keyboard_height() <= px(0.0), || view().pb_safe_area()))
        .child(links::popup())
        .child(sidebar::drawer())
        .child(settings::menu())
        .child(settings::screen())
        .child(dialog::overlay())
        .child(toast::view_of())
}
