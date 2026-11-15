//! Fixed documents and states for the look checks: the structural snapshot (`tests/snapshot.rs`) and the
//! offscreen render (`outliner-mac`, `src/bin/shot.rs`) open the app on one of these instead of the demo.

use nsh_core::element::Root;
use nsh_core::prelude::*;

use crate::appearance::{self, Modes};
use crate::settings::{self, Prefs};
use crate::sidebar::{self, Tab};
use crate::{commands, store, workspace};

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

/// The item whose note the "note" state edits, and the item the "select" state selects.
pub fn noted_item(doc: &str) -> &'static str {
    match doc {
        "inline" => "s5",
        "looks" => "l3",
        _ => "d1",
    }
}

/// The query the "search" state searches for.
pub fn search_query(doc: &str) -> &'static str {
    match doc {
        "inline" => "code",
        "looks" => "level",
        _ => "plain",
    }
}

pub fn selected_item(doc: &str) -> &'static str {
    match doc {
        "inline" => "s7",
        "looks" => "l6",
        _ => "d0",
    }
}

/// The settings of the "prefs" state: checked items hidden, notes on their first line, no bullet thread, the
/// current item highlighted, 24-hour times.
pub const PREFS: &[(&str, &str)] = &[
    ("show_checked", "false"),
    ("show_notes", "teaser"),
    ("bullet_threading", "false"),
    ("current_highlight", "true"),
    ("time_format", "24h"),
];

/// The documents in the folders of the file tree states ("folders", "menu"): "Projects" (open) holds the first
/// two, the second archived; "Old" (folded) the last.
pub const FOLDER_DOCS: [(&str, &str); 3] = [("p1", "Plan"), ("p2", "Budget"), ("o1", "Notes")];

/// Where the "menu" state shows the menu of folder "Projects".
pub const MENU_AT: (f32, f32) = (40.0, 160.0);

/// A fixture document's name (its root's text).
pub fn doc_title(doc: &str) -> &'static str {
    match doc {
        "inline" => "Inline",
        "looks" => "Looks",
        _ => "Demo",
    }
}

/// The item the "marks" state's second bookmark zooms into, and its name.
pub fn zoomed_item(doc: &str) -> (&'static str, &'static str) {
    match doc {
        "inline" => ("s7", "Numbered list"),
        "looks" => ("l6", "Color labels"),
        _ => ("d0", "Plain item 1"),
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
    /// ("files", "bookmarks", "tags"), with folders in the file tree ("folders"; "menu": and the menu of a folder
    /// open), or with two bookmarks ("marks"); or the settings shown, on Preferences ("settings"), on Look with the
    /// `[type]` controls ("look"), on Keymap ("keymap") or on Sync & data ("data"); or the outline with the
    /// settings of [`PREFS`] ("prefs"); or a state of the outline: the note of [`noted_item`] edited ("note"),
    /// [`selected_item`] selected ("select"); searching for [`search_query`] ("search"); `[[` typed at the end of
    /// [`edited_item`] ("links"); an item linking to `looks`' "Levels", zoomed in on it with backlinks shown
    /// ("backlinks").
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
    // an empty document beside it, as the app has (the sidebar lists both); the file tree states add folders
    let side = props.side.get();
    let tree = matches!(side.as_str(), "folders" | "menu");
    let mut docs = vec![(doc.clone(), first), ("untitled".to_owned(), store::named("Untitled"))];
    if tree {
        for (id, name) in FOLDER_DOCS {
            docs.push((id.to_owned(), store::named(name)));
        }
    }
    store::open_all(docs);
    workspace::reset();
    let top = [(doc.as_str(), doc_title(&doc)), ("untitled", "Untitled")];
    if tree {
        workspace::list(&top, &[("f1", "Projects", false, &FOLDER_DOCS[..2]), ("f2", "Old", true, &FOLDER_DOCS[2..])]);
        workspace::set_archived("p2", true);
    } else {
        workspace::list(&top, &[]);
    }
    // as the app starts (`App`): the first item edited with "Auto-focus on first item"
    settings::document_opened();
    match side.as_str() {
        "" => {}
        "files" => sidebar::show(Tab::Files),
        "bookmarks" => sidebar::show(Tab::Bookmarks),
        "tags" => sidebar::show(Tab::Tags),
        "folders" => sidebar::show(Tab::Files),
        "menu" => {
            sidebar::show(Tab::Files);
            sidebar::file_menu_at("f1", MENU_AT.0, MENU_AT.1);
        }
        "marks" => {
            let (item, name) = zoomed_item(&doc);
            workspace::add_bookmark(doc_title(&doc), &doc, None);
            workspace::add_bookmark(name, &doc, Some(item));
            sidebar::show(Tab::Bookmarks);
        }
        "settings" => settings::open(settings::Tab::Prefs),
        "keymap" => settings::open(settings::Tab::Keymap),
        "data" => settings::open(settings::Tab::Data),
        "prefs" => {
            // the settings that change how the outline shows (the web's model page sets the same, qa/model.mjs)
            let mut p = settings::get();
            for (key, value) in PREFS {
                if settings::FLAGS.iter().any(|f| f.key == *key) {
                    p.set_flag(key, *value == "true");
                } else {
                    p.set_pick(key, value);
                }
            }
            settings::use_prefs(p);
        }
        "look" => {
            settings::open(settings::Tab::Look);
            settings::show_table("type");
        }
        "note" => commands::start_editing_note(noted_item(&doc)),
        "select" => {
            commands::start_editing(selected_item(&doc), Some(0));
            commands::extend(true);
        }
        "search" => crate::find::open(Some(search_query(&doc))),
        "links" => {
            // `[[` typed at the end of the edited item: the items suggested
            let id = edited_item(&doc);
            let old = store::peek(|s| store::content_of(s, id));
            let new = format!("{old} [[");
            commands::set_text(id, false, &new, commands::utf16_len(&new));
            commands::start_editing(id, None);
            crate::links::typed(id, false, &old, &new);
        }
        "backlinks" => {
            // an item linking to "Levels" and to its child, zoomed in on "Levels", backlinks shown
            crate::find::set_pref("show_backlinks", true);
            let src = format!("A checkbox, off [Levels]({}) [Second]({})", crate::links::item_url(&doc, "l15"), crate::links::item_url(&doc, "l16"));
            commands::set_text("l4", false, &src, 0);
            crate::links::go(&doc, "l15");
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

/// The app as it starts, on the documents kept in the storage the engine is given (`config.storage`); on a first
/// start the small demo (id "demo") and an empty document ("untitled"), as the app's first start has the large
/// demo and an empty one.
#[component]
pub fn Started() -> impl IntoElement {
    workspace::start(|| {
        vec![
            ("demo".to_owned(), "Demo".to_owned(), store::demo(DEMO_SMALL)),
            ("untitled".to_owned(), "Untitled".to_owned(), store::named("Untitled")),
        ]
    });
    crate::root()
}

/// The root of [`Started`].
pub fn started_root() -> Root {
    Root::of::<Started>()
}
