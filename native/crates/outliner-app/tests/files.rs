//! Files, folders and bookmarks, headless (area 11): the documents and the workspace kept on the device and
//! opened again; folders made, renamed, folded and filled; the file menu (copy, archive, delete with its
//! confirmation, "Move to…" on phones); rows dragged to another place or into a folder; bookmarks of a document
//! and of a zoomed item, opened, renamed, removed; the sidebar's state kept across starts.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use nsh_core::geometry::{point, Points};
use nsh_core::platform::{Idiom, Os};
use nsh_core::time::Clock;
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, MemoryStorage, Screen};
use outliner_core::DocSnapshot;

const SHOWS: Duration = Duration::from_secs(10);

/// The app as it starts on the documents `kept` holds (a first start: the small demo and an empty document).
fn start(os: Os, kept: &Arc<MemoryStorage>) -> Headless {
    let mut config = EngineConfig::new(outliner_app::fixture::started_root());
    config.environment.os = os;
    if os == Os::MacOs {
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: Points(1100.0), height: Points(800.0) };
    }
    config.storage = Some(kept.clone());
    // a manual clock: waits move it a frame at a time, so the saves (300 ms after a change) come in step
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_secs(1_718_452_800));
    Headless::start(config).unwrap()
}

fn fresh() -> Arc<MemoryStorage> {
    MemoryStorage::new(std::iter::empty())
}

fn entries(kept: &MemoryStorage) -> BTreeMap<String, String> {
    kept.entries()
}

/// The texts of the sidebar's file rows (files or bookmarks), top to bottom, with their depth.
fn rows(screen: &Screen) -> Vec<(String, f32)> {
    let nodes = screen.layout_nodes();
    let mut out: Vec<(f32, String, f32)> = Vec::new();
    for n in nodes.iter().filter(|n| n.test_id.as_deref() == Some("file-name") && n.bounds.size.height.0 > 0.0) {
        let text = n.text.as_ref().map(|t| t.text.text.clone()).unwrap_or_default();
        out.push((n.bounds.origin.y.0, text, n.bounds.origin.x.0));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, t, x)| (t, x)).collect()
}

fn names(screen: &Screen) -> Vec<String> {
    rows(screen).into_iter().map(|(t, _)| t).collect()
}

fn expect(app: &mut Headless, what: &str, ok: impl Fn(&Screen) -> bool) {
    let waited = app.wait_until(what, SHOWS, ok);
    assert!(waited.is_ok(), "{what}: {waited:?}");
}

fn expect_names(app: &mut Headless, want: &[&str]) {
    let want: Vec<String> = want.iter().map(|s| (*s).to_owned()).collect();
    let waited = app.wait_until("the file rows", SHOWS, |s| names(s) == want);
    assert!(waited.is_ok(), "rows {:?}, want {want:?}", names(app.screen()));
}

/// Waits until `kept` holds what `ok` says (the documents are saved 300 ms after a change).
fn expect_kept(app: &mut Headless, kept: &Arc<MemoryStorage>, what: &str, ok: impl Fn(&BTreeMap<String, String>) -> bool) {
    let watched = kept.clone();
    let waited = app.wait_until(what, SHOWS, move |_| ok(&entries(&watched)));
    assert!(waited.is_ok(), "{what}: {waited:?}; kept: {:?}", entries(kept).keys().collect::<Vec<_>>());
}

/// Lets the saves due run (300 ms after the last change).
fn saved(app: &mut Headless) {
    app.run_for(Duration::from_millis(400)).unwrap();
}

fn rename_to(app: &mut Headless, name: &str) {
    app.wait_for("id:rename").unwrap();
    app.clear("id:rename").unwrap();
    app.type_text("id:rename", name).unwrap();
    app.submit("id:rename").unwrap();
    app.wait_for_gone("id:rename", SHOWS).unwrap();
}

/// A new folder at the top level, named `name`.
fn new_folder(app: &mut Headless, name: &str) {
    app.press("id:plus").unwrap();
    app.press("text:New folder").unwrap();
    rename_to(app, name);
}

#[test]
fn the_documents_are_kept_and_open_again_as_they_were_left() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    // a first start keeps the documents and the workspace in the web's format
    expect_kept(&mut app, &kept, "the first documents kept", |e| {
        ["doc:workspace", "doc:demo", "doc:untitled"].iter().all(|k| e.contains_key(*k))
    });
    let snap: DocSnapshot = serde_json::from_str(&entries(&kept)["doc:demo"]).unwrap();
    assert_eq!(snap.version, 0);
    assert!(!snap.pending.is_empty(), "the demo's items are ops waiting for a server");
    // an edit, a new document, the empty one opened and zoomed nowhere, the demo zoomed into an item
    app.press("text:Plain item 1").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.type_text("id:row-input", " kept").unwrap();
    app.press("id:plus").unwrap();
    app.press("text:New document").unwrap();
    rename_to(&mut app, "Second");
    app.press("id:sidebar >> text:Demo").unwrap();
    app.wait_for("text:Plain item 1 kept").unwrap();
    expect_kept(&mut app, &kept, "the edit kept", |e| {
        e.get("doc:demo").is_some_and(|d| d.contains("Plain item 1 kept"))
            && e.get("outliner:lastDoc").map(String::as_str) == Some("demo")
            && e.values().any(|v| v.contains("\"Second\""))
    });
    drop(app);
    let mut again = start(Os::MacOs, &kept);
    again.wait_for("text:Plain item 1 kept").unwrap();
    expect_names(&mut again, &["Demo", "Untitled", "Second"]);
    again.press("id:sidebar >> text:Second").unwrap();
    again.wait_for("id:title >> text:Second").unwrap();
    expect_kept(&mut again, &kept, "the open document kept", |e| e.get("outliner:lastDoc").is_some_and(|d| d != "demo"));
    drop(again);
    // the document open last opens
    let mut third = start(Os::MacOs, &kept);
    third.wait_for("id:title >> text:Second").unwrap();
}

#[test]
fn a_zoomed_document_opens_zoomed_again() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    // zoom into the first item (Mod+. on the edited item)
    app.press("text:Plain item 1").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.press_key("mod+]").unwrap();
    app.wait_for("id:title >> text:Plain item 1").unwrap();
    expect_kept(&mut app, &kept, "the zoom kept", |e| e.get("outliner:zooms").is_some_and(|z| z.contains("\"demo\":\"d0\"")));
    drop(app);
    let mut again = start(Os::MacOs, &kept);
    again.wait_for("id:title >> text:Plain item 1").unwrap();
}

#[test]
fn folders_are_made_named_filled_and_folded() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    new_folder(&mut app, "Projects");
    expect_names(&mut app, &["Demo", "Untitled", "Projects"]);
    // a document made in the folder (its menu's "New document"), one level in
    app.right_click("id:sidebar >> text:Projects").unwrap();
    app.press("text:New document").unwrap();
    rename_to(&mut app, "Plan");
    expect_names(&mut app, &["Demo", "Untitled", "Projects", "Plan"]);
    let r = rows(app.screen());
    let indent = outliner_app::appearance::Looks::default().desktop.number("space.file_indent");
    assert!((r[3].1 - r[2].1 - indent).abs() < 0.5, "the document is one level in: {r:?}");
    // a press folds the folder and opens it again
    app.press("id:sidebar >> text:Projects").unwrap();
    expect_names(&mut app, &["Demo", "Untitled", "Projects"]);
    assert!(app.screen().shows("▸"));
    // not a double press (which edits the name)
    app.run_for(Duration::from_millis(600)).unwrap();
    app.press("id:sidebar >> text:Projects").unwrap();
    expect_names(&mut app, &["Demo", "Untitled", "Projects", "Plan"]);
    // "Collapse all folders"
    app.press("id:fold").unwrap();
    expect_names(&mut app, &["Demo", "Untitled", "Projects"]);
    // the tree is kept: a new start shows it folded, the document in it
    expect_kept(&mut app, &kept, "the folder kept", |e| e.get("doc:workspace").is_some_and(|w| w.contains("\"Projects\"") && w.contains("\"folder\"")));
    saved(&mut app);
    drop(app);
    // the document open last ("Plan") opens
    let mut again = start(Os::MacOs, &kept);
    again.wait_for("id:title >> text:Plan").unwrap();
    expect_names(&mut again, &["Demo", "Untitled", "Projects"]);
    again.press("id:sidebar >> text:Projects").unwrap();
    expect_names(&mut again, &["Demo", "Untitled", "Projects", "Plan"]);
}

#[test]
fn a_double_press_renames_and_the_menu_copies_archives_and_deletes() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    // a double press edits the name (the web's double click)
    app.press("id:sidebar >> text:Untitled").unwrap();
    app.press("id:sidebar >> text:Untitled").unwrap();
    rename_to(&mut app, "Notes");
    app.wait_for("id:title >> text:Notes").unwrap();
    // a copy of the demo, right after it, opened, with its items
    app.right_click("id:sidebar >> text:Demo").unwrap();
    for item in ["Make a copy", "Add to bookmarks", "Archive", "Rename", "Delete"] {
        assert!(app.screen().shows(item), "the menu offers {item}");
    }
    app.press("text:Make a copy").unwrap();
    expect_names(&mut app, &["Demo", "Demo (copy)", "Notes"]);
    app.wait_for("id:title >> text:Demo (copy)").unwrap();
    app.wait_for("text:Plain item 1").unwrap();
    // archived: the menu offers to unarchive it
    app.right_click("id:sidebar >> text:Notes").unwrap();
    app.press("text:Archive").unwrap();
    app.right_click("id:sidebar >> text:Notes").unwrap();
    app.wait_for("text:Unarchive").unwrap();
    app.press("id:menu-scrim").unwrap();
    app.wait_for_gone("id:popup", SHOWS).unwrap();
    // Delete asks first; Cancel keeps it
    app.right_click("id:sidebar >> text:Demo (copy)").unwrap();
    app.press("text:Delete").unwrap();
    app.wait_for("text:Are you sure you want to delete the document \"Demo (copy)\"?").unwrap();
    app.press("label:Cancel").unwrap();
    app.wait_for_gone("id:dialog", SHOWS).unwrap();
    expect_names(&mut app, &["Demo", "Demo (copy)", "Notes"]);
    // deleting the open document opens the next one
    app.right_click("id:sidebar >> text:Demo (copy)").unwrap();
    app.press("text:Delete").unwrap();
    app.press("id:dialog >> label:Delete").unwrap();
    expect_names(&mut app, &["Demo", "Notes"]);
    app.wait_for("id:title >> text:Notes").unwrap();
    // its data stays on the device, as the web leaves it
    expect_kept(&mut app, &kept, "the workspace kept", |e| {
        e.get("doc:workspace").is_some_and(|w| w.contains("\"archived\":true"))
    });
}

#[test]
fn deleting_a_folder_takes_its_documents() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    new_folder(&mut app, "Box");
    app.right_click("id:sidebar >> text:Box").unwrap();
    app.press("text:New document").unwrap();
    rename_to(&mut app, "Inside");
    app.wait_for("id:title >> text:Inside").unwrap();
    app.right_click("id:sidebar >> text:Box").unwrap();
    app.press("text:Delete").unwrap();
    app.wait_for("text:Are you sure you want to delete the folder \"Box\"?").unwrap();
    app.press("id:dialog >> label:Delete").unwrap();
    expect_names(&mut app, &["Demo", "Untitled"]);
    // the open document was in it: the one before it opens
    app.wait_for("id:title >> text:Untitled").unwrap();
}

#[test]
fn desktop_rows_drag_to_another_place_and_into_a_folder() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    new_folder(&mut app, "Projects");
    expect_names(&mut app, &["Demo", "Untitled", "Projects"]);
    let h = outliner_app::appearance::Looks::default().desktop.number("space.file_row");
    // "Untitled" dragged up a row and a bit: before "Demo"
    app.drag("id:sidebar >> text:Untitled", point(Points(0.0), Points(-h * 0.9))).unwrap();
    expect_names(&mut app, &["Untitled", "Demo", "Projects"]);
    // "Demo" dragged down a row: into the folder's middle
    app.drag("id:sidebar >> text:Demo", point(Points(0.0), Points(h))).unwrap();
    expect_names(&mut app, &["Untitled", "Projects", "Demo"]);
    let r = rows(app.screen());
    assert!(r[2].1 > r[1].1, "Demo is in the folder: {r:?}");
    // the press did not open anything on the way
    app.wait_for("text:Plain item 1").unwrap();
    // back out: dragged above the folder
    app.drag("id:sidebar >> text:Demo", point(Points(0.0), Points(-h * 1.2))).unwrap();
    expect_names(&mut app, &["Untitled", "Demo", "Projects"]);
    let r = rows(app.screen());
    assert!((r[1].1 - r[0].1).abs() < 0.5, "Demo is at the top level again: {r:?}");
}

#[test]
fn phones_move_a_document_with_its_menu() {
    let kept = fresh();
    let mut app = start(Os::Ios, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    app.press("label:Toggle sidebar").unwrap();
    app.wait_for("id:sidebar").unwrap();
    app.press("id:plus").unwrap();
    app.press("text:New folder").unwrap();
    rename_to(&mut app, "Projects");
    app.long_press("id:sidebar >> text:Untitled").unwrap();
    app.press("text:Move to…").unwrap();
    app.wait_for("text:Top level").unwrap();
    app.press("id:popup >> text:Projects").unwrap();
    expect_names(&mut app, &["Demo", "Projects", "Untitled"]);
    let r = rows(app.screen());
    assert!(r[2].1 > r[1].1, "Untitled is in the folder: {r:?}");
    // a folder does not offer itself
    app.long_press("id:sidebar >> text:Projects").unwrap();
    app.press("text:Move to…").unwrap();
    app.wait_for("text:Top level").unwrap();
    assert_eq!(app.screen().count("id:popup >> text:Projects").unwrap(), 0);
}

#[test]
fn bookmarks_open_a_document_or_a_zoomed_item() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    app.press("id:sidebar >> text:Bookmarks").unwrap();
    app.wait_for("text:Bookmark a document, a zoomed item or a search with the ☆ button.").unwrap();
    // the document, then an item zoomed into
    app.press("id:star").unwrap();
    expect_names(&mut app, &["Demo"]);
    app.press("text:Plain item 1").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.press_key("mod+]").unwrap();
    app.wait_for("id:title >> text:Plain item 1").unwrap();
    app.press("id:star").unwrap();
    expect_names(&mut app, &["Demo", "Plain item 1"]);
    assert!(!app.screen().shows("Bookmark a document, a zoomed item or a search with the ☆ button."));
    // from another document: the zoomed one opens zoomed, the document's at its top
    app.press("id:sidebar >> text:Files").unwrap();
    app.press("id:sidebar >> text:Untitled").unwrap();
    app.wait_for("id:title >> text:Untitled").unwrap();
    app.press("id:sidebar >> text:Bookmarks").unwrap();
    app.press("id:sidebar >> text:Plain item 1").unwrap();
    app.wait_for("id:title >> text:Plain item 1").unwrap();
    app.press("id:sidebar >> text:Demo").unwrap();
    app.wait_for("id:title >> text:Demo").unwrap();
    // renamed in a dialog (a double press, or its menu), removed after a question
    app.right_click("id:sidebar >> text:Plain item 1").unwrap();
    app.press("text:Rename").unwrap();
    app.wait_for("id:dialog-input").unwrap();
    app.clear("id:dialog-input").unwrap();
    app.type_text("id:dialog-input", "First item").unwrap();
    app.press("label:Save").unwrap();
    expect_names(&mut app, &["Demo", "First item"]);
    // dragged above the other
    let h = outliner_app::appearance::Looks::default().desktop.number("space.file_row");
    app.drag("id:sidebar >> text:First item", point(Points(0.0), Points(-h * 0.9))).unwrap();
    expect_names(&mut app, &["First item", "Demo"]);
    app.right_click("id:sidebar >> text:Demo").unwrap();
    app.press("text:Remove").unwrap();
    app.wait_for("text:Remove the bookmark \"Demo\"?").unwrap();
    app.press("id:dialog >> label:Remove").unwrap();
    expect_names(&mut app, &["First item"]);
    // kept with the workspace
    expect_kept(&mut app, &kept, "the bookmark kept", |e| {
        e.get("doc:workspace").is_some_and(|w| w.contains("\"First item\"") && w.contains("\"zoom\":\"d0\""))
    });
    saved(&mut app);
    drop(app);
    let mut again = start(Os::MacOs, &kept);
    again.wait_for("text:Plain item 1").unwrap();
    // the Bookmarks tab was the last one shown
    expect_names(&mut again, &["First item"]);
}

#[test]
fn the_sidebars_state_is_kept() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    app.press("id:sidebar >> text:Tags").unwrap();
    app.press("text:By count").unwrap();
    app.wait_for("text:A–Z").unwrap();
    app.drag("id:splitter", point(Points(60.0), Points(0.0))).unwrap();
    let e = entries(&kept);
    assert_eq!(e.get("tag_sort").map(String::as_str), Some("alpha"));
    assert_eq!(e.get("sidebar_tab").map(String::as_str), Some("tags"));
    let w: f32 = e["sidebar_width"].parse().unwrap();
    drop(app);
    let mut again = start(Os::MacOs, &kept);
    again.wait_for("text:A–Z").unwrap();
    let side = again
        .screen()
        .layout_nodes()
        .into_iter()
        .find(|n| n.test_id.as_deref() == Some("sidebar"))
        .map(|n| n.bounds.size.width.0)
        .unwrap();
    assert!((side - w).abs() < 1.0, "the panel is {side} wide, kept {w}");
    again.press("label:Toggle sidebar").unwrap();
    expect(&mut again, "the panel hidden", |s| s.layout_nodes().iter().all(|n| n.test_id.as_deref() != Some("sidebar") || n.bounds.size.width.0 == 0.0));
    drop(again);
    let mut third = start(Os::MacOs, &kept);
    third.wait_for("text:Plain item 1").unwrap();
    assert!(!third.screen().shows("A–Z"), "the panel stays hidden");
}

#[test]
fn a_folders_menu_imports_an_opml_file_into_it() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    app.wait_for("text:Plain item 1").unwrap();
    new_folder(&mut app, "Projects");
    let opml = r#"<?xml version="1.0"?><opml version="2.0"><head><title>Trip</title></head><body><outline text="Pack"><outline text="Socks"/></outline></body></opml>"#;
    app.on_pick(move |_| vec![nsh_core::picker::PickedFile::new("trip.opml", "text/x-opml", nsh_core::image::ImageData::new(opml.as_bytes().to_vec()))]);
    app.right_click("id:sidebar >> text:Projects").unwrap();
    app.press("text:Import OPML…").unwrap();
    // a new document in the folder, named after the file's title, opened
    expect_names(&mut app, &["Demo", "Untitled", "Projects", "Trip"]);
    app.wait_for("id:title >> text:Trip").unwrap();
    app.wait_for("text:Pack").unwrap();
    let r = rows(app.screen());
    assert!(r[3].1 > r[2].1, "the import is in the folder: {r:?}");
}
