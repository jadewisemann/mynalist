//! Fixed documents and states for the look checks: the structural snapshot (`tests/snapshot.rs`) and the
//! offscreen render (`outliner-mac`, `src/bin/shot.rs`) open the app on one of these instead of the demo.

use nsh_core::element::Root;
use nsh_core::prelude::*;

use crate::appearance::{self, Modes};
use crate::settings::{self, Prefs};
use crate::sidebar::{self, Tab};
use crate::{commands, store};

/// The documents a fixture opens.
pub const DOCS: [&str; 3] = ["demo", "inline", "looks"];

/// Items in the fixtures' demo document: the demo's first screens, without the 10 000 items of the app's (only
/// the performance test opens that one), so that tests start fast however busy the machine is.
pub const DEMO_SMALL: usize = 120;

/// The item each document edits in its editing state.
pub fn edited_item(doc: &str) -> &'static str {
    match doc {
        "inline" => "s0",
        "looks" => "l17",
        _ => "d1",
    }
}

/// The modes of a fixture that uses the prefs kept on the device instead of modes of its own.
pub const STORED: &str = "stored";

/// The props of [`Fixture`]: the document (one of [`DOCS`]) and the item being edited ("" for none).
#[derive(Clone, PartialEq, Props)]
pub struct FixtureProps {
    pub doc: Prop<String>,
    #[prop(default)]
    pub editing: Prop<String>,
    /// The modes the look is resolved for, as `group=mode` words ([`Modes::parse`]; the device comes from the
    /// platform), or [`STORED`].
    #[prop(default)]
    pub modes: Prop<String>,
    /// A user's appearance TOML layered over them ("" for none).
    #[prop(default)]
    pub appearance: Prop<String>,
    /// The sidebar: "" as the device starts it (shown on desktops, hidden on phones), or shown on a tab
    /// ("files", "bookmarks", "tags"); or the settings shown, on Preferences ("settings") or on Look with the
    /// `[type]` controls ("look").
    #[prop(default)]
    pub side: Prop<String>,
}

/// The app opened on a fixed document, with one item being edited if asked (its caret at the end).
#[component]
pub fn Fixture(props: FixtureProps) -> impl IntoElement {
    let doc = props.doc.get();
    let (modes, user) = (props.modes.get(), props.appearance.get());
    // "stored": the prefs kept on the device, as the app starts with them (the settings' tests)
    if modes != STORED {
        let modes = Modes::parse(&modes).unwrap_or_else(|e| panic!("the fixture's modes: {e}"));
        if let Err(e) = appearance::parse(&user) {
            panic!("the fixture's appearance: {e}");
        }
        // the modes as prefs, not kept on the device (the theme named: none follows the system's appearance)
        settings::use_prefs(Prefs::of(&modes, &user));
    }
    let first = match doc.as_str() {
        "inline" => store::showcase(),
        "looks" => store::looks(),
        "demo" => store::demo(DEMO_SMALL),
        other => panic!("no fixture document {other}"),
    };
    // an empty document beside it, as the app has (the sidebar lists both)
    store::open_all(vec![(doc.clone(), first), ("untitled".into(), store::named("Untitled"))]);
    match props.side.get().as_str() {
        "" => {}
        "files" => sidebar::show(Tab::Files),
        "bookmarks" => sidebar::show(Tab::Bookmarks),
        "tags" => sidebar::show(Tab::Tags),
        "settings" => settings::open(settings::Tab::Prefs),
        "look" => {
            settings::open(settings::Tab::Look);
            settings::show_table("type");
        }
        other => panic!("the fixture's sidebar: {other}"),
    }
    let editing = props.editing.get();
    if !editing.is_empty() {
        commands::start_editing(&editing, None);
    }
    crate::root()
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
