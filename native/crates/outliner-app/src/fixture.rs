//! Fixed documents and states for the look checks: the structural snapshot (`tests/snapshot.rs`) and the
//! offscreen render (`outliner-mac`, `src/bin/shot.rs`) open the app on one of these instead of the demo.

use nsh_core::prelude::*;
use nsh_core::element::Root;

use crate::{commands, store, DEMO_ITEMS};

/// The documents a fixture opens.
pub const DOCS: [&str; 3] = ["demo", "inline", "looks"];

/// The item each document edits in its editing state.
pub fn edited_item(doc: &str) -> &'static str {
    match doc {
        "inline" => "s0",
        "looks" => "l17",
        _ => "d1",
    }
}

/// The props of [`Fixture`]: the document (one of [`DOCS`]) and the item being edited ("" for none).
#[derive(Clone, PartialEq, Props)]
pub struct FixtureProps {
    pub doc: Prop<String>,
    #[prop(default)]
    pub editing: Prop<String>,
}

/// The app opened on a fixed document, with one item being edited if asked (its caret at the end).
#[component]
pub fn Fixture(props: FixtureProps) -> impl IntoElement {
    let doc = props.doc.get();
    store::open(match doc.as_str() {
        "inline" => store::showcase(),
        "looks" => store::looks(),
        _ => store::demo(DEMO_ITEMS),
    });
    let editing = props.editing.get();
    if !editing.is_empty() {
        commands::start_editing(&editing, None);
    }
    crate::screen()
}

/// The root of the app on `doc`, editing `editing` when given.
pub fn root(doc: &str, editing: Option<&str>) -> Root {
    let doc = doc.to_owned();
    let editing = editing.unwrap_or_default().to_owned();
    Root::new::<Fixture>(move || FixtureProps { doc: Prop::new(doc.clone()), editing: Prop::new(editing.clone()) })
}
