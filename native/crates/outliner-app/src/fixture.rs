//! Fixed documents and states for the look checks: the structural snapshot (`tests/snapshot.rs`) and the
//! offscreen render (`outliner-mac`, `src/bin/shot.rs`) open the app on one of these instead of the demo.

use nsh_core::prelude::*;
use nsh_core::element::Root;

use crate::appearance::{self, Modes};
use crate::sidebar::{self, Tab};
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
    /// The modes the look is resolved for, as `group=mode` words ([`Modes::parse`]; the device comes from the
    /// platform).
    #[prop(default)]
    pub modes: Prop<String>,
    /// A user's appearance TOML layered over them ("" for none).
    #[prop(default)]
    pub appearance: Prop<String>,
    /// The sidebar: "" as the device starts it (shown on desktops, hidden on phones), or shown on a tab
    /// ("files", "bookmarks", "tags").
    #[prop(default)]
    pub side: Prop<String>,
}

/// The app opened on a fixed document, with one item being edited if asked (its caret at the end).
#[component]
pub fn Fixture(props: FixtureProps) -> impl IntoElement {
    let doc = props.doc.get();
    let (modes, user) = (props.modes.get(), props.appearance.get());
    let modes = Modes::parse(&modes).unwrap_or_else(|e| panic!("the fixture's modes: {e}"));
    if modes != Modes::default() || !user.is_empty() {
        appearance::set(modes, &user).unwrap_or_else(|e| panic!("the fixture's appearance: {e}"));
    }
    let first = match doc.as_str() {
        "inline" => store::showcase(),
        "looks" => store::looks(),
        _ => store::demo(DEMO_ITEMS),
    };
    // an empty document beside it, as the app has (the sidebar lists both)
    store::open_all(vec![(doc.clone(), first), ("untitled".into(), store::named("Untitled"))]);
    match props.side.get().as_str() {
        "" => {}
        "files" => sidebar::show(Tab::Files),
        "bookmarks" => sidebar::show(Tab::Bookmarks),
        "tags" => sidebar::show(Tab::Tags),
        other => panic!("the fixture's sidebar: {other}"),
    }
    let editing = props.editing.get();
    if !editing.is_empty() {
        commands::start_editing(&editing, None);
    }
    crate::screen()
}

/// The root of the app on `doc`, editing `editing` when given.
pub fn root(doc: &str, editing: Option<&str>) -> Root {
    root_with(doc, editing, "", "")
}

/// [`root`] with the look resolved for `modes` and a user's appearance TOML.
pub fn root_with(doc: &str, editing: Option<&str>, modes: &str, appearance: &str) -> Root {
    root_side(doc, editing, modes, appearance, "")
}

/// [`root_with`] with the sidebar shown on a tab ([`FixtureProps::side`]).
pub fn root_side(doc: &str, editing: Option<&str>, modes: &str, appearance: &str, side: &str) -> Root {
    let doc = doc.to_owned();
    let modes = modes.to_owned();
    let editing = editing.unwrap_or_default().to_owned();
    let appearance = appearance.to_owned();
    let side = side.to_owned();
    Root::new::<Fixture>(move || FixtureProps {
        doc: Prop::new(doc.clone()),
        editing: Prop::new(editing.clone()),
        modes: Prop::new(modes.clone()),
        appearance: Prop::new(appearance.clone()),
        side: Prop::new(side.clone()),
    })
}

/// The items of `doc` (one of [`DOCS`] but the 10 000-item demo) in tree order, as JSON lines the web's model
/// page reads to show the same document: `{"id", "parent", "props", "collapsed"}`.
pub fn items_json(doc: &str) -> String {
    let session = match doc {
        "inline" => store::showcase(),
        "looks" => store::looks(),
        _ => panic!("no items for {doc}"),
    };
    let tree = &session.live;
    let mut out = String::new();
    let mut stack = vec![outliner_core::tree::ROOT.to_owned()];
    // the zoom root (the title) first, then its items depth first
    while let Some(id) = stack.pop() {
        let n = tree.get(&id).expect("a node of the tree");
        let item = serde_json::json!({ "id": id, "parent": n.parent, "props": n.props, "collapsed": n.collapsed });
        out.push_str(&item.to_string());
        out.push('\n');
        stack.extend(n.children.iter().rev().cloned());
    }
    out
}
