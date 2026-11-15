//! The web's other screens, headless (area 14): read mode (the lock), Back and Forward through the places the app
//! was in, the help panel with the keymap's keys, the tutorial and "What's new", the mind map, the inbox (its
//! destination picked in the settings, capture, go to inbox), the file finder, and the doc actions (bookmark this
//! view, view options).

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use nsh_core::platform::{Idiom, Os};
use nsh_core::time::Clock;
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, MemoryStorage, Screen};

const SHOWS: Duration = Duration::from_secs(10);

fn start_on(os: Os, root: nsh_core::element::Root, kept: &Arc<MemoryStorage>) -> Headless {
    let mut config = EngineConfig::new(root);
    config.environment.os = os;
    if os == Os::MacOs {
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: nsh_core::Points(1100.0), height: nsh_core::Points(800.0) };
    }
    config.storage = Some(kept.clone());
    // a manual clock: time moves only a frame at a time, as the steps ask for it
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_secs(1_718_452_800));
    Headless::start(config).unwrap()
}

/// The app on the small demo with the prefs `kept` holds.
fn start(os: Os, kept: &Arc<MemoryStorage>) -> Headless {
    let root = outliner_app::fixture::root_with("demo", None, outliner_app::fixture::STORED, "");
    let mut app = start_on(os, root, kept);
    app.wait_for("text:Plain item 1").unwrap();
    app
}

fn fresh() -> Arc<MemoryStorage> {
    MemoryStorage::new(std::iter::empty())
}

fn expect(app: &mut Headless, what: &str, f: impl Fn(&Screen) -> bool) {
    let waited = app.wait_until(what, SHOWS, f);
    assert!(waited.is_ok(), "not {what}: {:?}", app.screen().texts());
}

/// How many views with test id `id` show.
fn count(s: &Screen, id: &str) -> usize {
    s.layout_nodes().into_iter().filter(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0).count()
}

/// Lets a few frames go by (on the manual clock), for what should not happen to have had its chance.
fn frames(app: &mut Headless) {
    app.run_for(Duration::from_millis(200)).unwrap();
}

/// The title's text (the zoom root's, or the document's name): the texts inside the title's box.
fn title(s: &Screen) -> String {
    let nodes = s.layout_nodes();
    let Some(t) = nodes.iter().find(|n| n.test_id.as_deref() == Some("title") && n.bounds.size.height.0 > 0.0) else {
        return String::new();
    };
    let (x0, y0) = (t.bounds.origin.x.0, t.bounds.origin.y.0);
    let (x1, y1) = (x0 + t.bounds.size.width.0, y0 + t.bounds.size.height.0);
    nodes
        .iter()
        .filter(|n| {
            let (x, y) = (n.bounds.origin.x.0, n.bounds.origin.y.0);
            n.bounds.size.height.0 > 0.0 && x >= x0 && y >= y0 && x < x1 && y < y1
        })
        .filter_map(|n| n.text.as_ref().map(|t| t.text.text.clone()))
        .collect::<Vec<_>>()
        .join("")
}

// ---- read mode -------------------------------------------------------------------------------------------

#[test]
fn read_mode_keeps_the_document_from_being_edited() {
    let mut app = start(Os::Ios, &fresh());
    // editing: the toolbar shows; locking ends the editing and the toolbar goes
    app.press("text:Plain item 1").unwrap();
    expect(&mut app, "the row edited", |s| count(s, "row-input") == 1 && count(s, "toolbar") == 1);
    app.press("label:Read mode (lock document)").unwrap();
    expect(&mut app, "nothing edited", |s| count(s, "row-input") == 0 && count(s, "toolbar") == 0);
    // a press on a row edits nothing; undo changes nothing
    app.press("text:Plain item 1").unwrap();
    frames(&mut app);
    assert_eq!(count(app.screen(), "row-input"), 0);
    // the bullet still folds and unfolds
    assert!(app.screen().shows("Bold and italic and code 2"));
    app.press("id:bullet >> nth=0").unwrap();
    expect(&mut app, "folded", |s| !s.shows("Bold and italic and code 2"));
    app.press("id:bullet >> nth=0").unwrap();
    expect(&mut app, "unfolded", |s| s.shows("Bold and italic and code 2"));
    // unlocked: a press edits again
    app.press("label:Read mode (lock document)").unwrap();
    app.press("text:Plain item 1").unwrap();
    expect(&mut app, "the row edited again", |s| count(s, "row-input") == 1);
}

#[test]
fn the_view_options_lock_the_document_on_desktops() {
    let mut app = start(Os::MacOs, &fresh());
    app.press("id:view-options").unwrap();
    app.press("text:Lock document (read-only)").unwrap();
    app.press("text:Plain item 1").unwrap();
    frames(&mut app);
    assert_eq!(count(app.screen(), "row-input"), 0);
    // ticked in the menu while on
    app.press("id:view-options").unwrap();
    app.press("text:✓ Lock document (read-only)").unwrap();
    app.press("text:Plain item 1").unwrap();
    expect(&mut app, "edited once unlocked", |s| count(s, "row-input") == 1);
}

// ---- back and forward -------------------------------------------------------------------------------------

#[test]
fn back_and_forward_go_through_the_zooms_and_documents() {
    let mut app = start(Os::MacOs, &fresh());
    let outside = "A link and a #tag 8";
    expect(&mut app, "the document's rows", |s| s.shows(outside));
    // zoomed in on the first item
    app.press("text:Plain item 1").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.press_key("mod+]").unwrap();
    expect(&mut app, "zoomed", |s| title(s) == "Plain item 1" && !s.shows(outside));
    // another document
    app.press("id:sidebar >> text:Untitled").unwrap();
    expect(&mut app, "the other document", |s| title(s) == "Untitled");
    app.press("label:Back").unwrap();
    expect(&mut app, "back in the zoom", |s| title(s) == "Plain item 1");
    // nothing edited after going back
    assert_eq!(count(app.screen(), "row-input"), 0);
    app.press("label:Back").unwrap();
    expect(&mut app, "back at the top", |s| title(s) == "Demo" && s.shows(outside));
    // nowhere further back
    app.press("label:Back").unwrap();
    frames(&mut app);
    assert_eq!(title(app.screen()), "Demo");
    app.press("label:Forward").unwrap();
    expect(&mut app, "forward to the zoom", |s| title(s) == "Plain item 1");
    // the keys (Alt+Right: forward again)
    app.press_key("alt+right").unwrap();
    expect(&mut app, "forward to the other document", |s| title(s) == "Untitled");
    app.press_key("alt+left").unwrap();
    expect(&mut app, "back with the key", |s| title(s) == "Plain item 1");
    // going somewhere new leaves nothing to go forward to
    app.press("label:Back").unwrap();
    expect(&mut app, "back at the top", |s| title(s) == "Demo");
    app.press("id:sidebar >> text:Untitled").unwrap();
    expect(&mut app, "the other document", |s| title(s) == "Untitled");
    app.press("label:Forward").unwrap();
    frames(&mut app);
    assert_eq!(title(app.screen()), "Untitled");
    app.press("label:Back").unwrap();
    expect(&mut app, "back at the top", |s| title(s) == "Demo");
}

// ---- help, tutorial, what's new ---------------------------------------------------------------------------

#[test]
fn the_help_panel_lists_the_keys_in_use() {
    let kept = MemoryStorage::new([("shortcut_override".to_owned(), r#"{"zoom-in":["Mod+Shift+K"]}"#.to_owned())]);
    let mut app = start(Os::MacOs, &kept);
    app.press_key("mod+/").unwrap();
    app.wait_for("id:help >> text:Shortcuts & formatting").unwrap();
    for t in ["Triggers", "Tag autocomplete", "Formatting", "**bold**", "Basics", "Navigate", "Change shortcuts in Settings → Keymap."] {
        assert!(app.screen().shows(t), "{t}");
    }
    // the keymap in use: the user's key for "Zoom in", a dash for a command without keys
    assert!(app.screen().shows("⌘⇧K"));
    assert!(app.screen().shows("—"));
    // beside the document: the rows still show
    assert!(app.screen().shows("Plain item 1"));
    app.press("id:help >> label:Close").unwrap();
    app.wait_for_gone("id:help", SHOWS).unwrap();
}

#[test]
fn the_menu_opens_help_the_tutorial_and_whats_new_on_phones() {
    let kept = fresh();
    let mut app = start(Os::Ios, &kept);
    app.press("label:Menu").unwrap();
    app.press("label:Shortcuts & formatting").unwrap();
    app.wait_for("id:help >> text:Tag autocomplete").unwrap();
    app.press("id:help >> label:Close").unwrap();
    app.wait_for_gone("id:help", SHOWS).unwrap();
    // the tutorial: a step at a time, its dots, Done on the last
    app.press("label:Menu").unwrap();
    app.press("label:Tutorial").unwrap();
    app.wait_for("text:1. Items").unwrap();
    app.press("label:Next").unwrap();
    app.wait_for("text:2. Zoom").unwrap();
    app.press("label:Step 8").unwrap();
    app.wait_for("text:8. Your data").unwrap();
    app.press("label:Done").unwrap();
    app.wait_for_gone("id:tour", SHOWS).unwrap();
    assert_eq!(kept.entries().get("finished_onboarding").map(String::as_str), Some("true"));
    app.press("label:Menu").unwrap();
    app.press("label:What's new").unwrap();
    app.wait_for("text:October 2026").unwrap();
    app.press("label:Okay").unwrap();
    app.wait_for_gone("id:news", SHOWS).unwrap();
}

// ---- the mind map -----------------------------------------------------------------------------------------

#[test]
fn the_mind_map_shows_the_zoomed_item_read_only() {
    let root = outliner_app::fixture::root_with("looks", None, outliner_app::fixture::STORED, "");
    let mut app = start_on(Os::MacOs, root, &fresh());
    app.wait_for("text:Heading one").unwrap();
    app.press("id:view-options").unwrap();
    app.press("text:Layout: Mind map (read-only)").unwrap();
    app.wait_for("id:mindmap").unwrap();
    // a box per item, the title's at the root; a folded item starts folded, with its count
    let boxes = |s: &Screen| count(s, "mm-box");
    expect(&mut app, "the boxes", |s| s.shows("Looks") && s.shows("Label 6") && s.shows("Fourth level"));
    assert!(!app.screen().shows("Hidden under the fold"));
    let all = boxes(app.screen());
    // a fold here unfolds it in the map only
    app.press("label:Expand").unwrap();
    expect(&mut app, "unfolded", |s| s.shows("Hidden under the fold") && boxes(s) == all + 1);
    // a double press zooms in on a box; the path zooms out
    app.press("id:mindmap >> text:Levels").unwrap();
    app.press("id:mindmap >> text:Levels").unwrap();
    expect(&mut app, "zoomed", |s| !s.shows("Heading one") && s.shows("Second level"));
    app.press("label:Looks").unwrap();
    expect(&mut app, "at the top", |s| s.shows("Heading one"));
    // back to the list
    app.press("label:← Back to document").unwrap();
    app.wait_for_gone("id:mindmap", SHOWS).unwrap();
    app.wait_for("id:row").unwrap();
}

// ---- the inbox and the finder ------------------------------------------------------------------------------

#[test]
fn the_inbox_is_chosen_captured_into_and_opened() {
    let kept = fresh();
    let mut app = start(Os::MacOs, &kept);
    // not set: capture says so
    app.press_key("mod+shift+i").unwrap();
    app.wait_for("text:/Set an inbox first/").unwrap();
    // chosen in the settings with the finder
    app.press("label:Settings").unwrap();
    app.wait_for("label:Change…").unwrap();
    assert!(app.screen().shows("not set"));
    app.press("label:Change…").unwrap();
    app.wait_for("id:finder-input").unwrap();
    app.type_text("id:finder-input", "plain item 8").unwrap();
    expect(&mut app, "the choices", |s| s.shows("Plain item 1") || s.shows("Plain item 6"));
    app.press_key("enter").unwrap();
    app.wait_for_gone("id:finder", SHOWS).unwrap();
    expect(&mut app, "the inbox named", |s| s.texts().iter().any(|t| t.starts_with("Demo › Plain item")));
    let loc = kept.entries().get("inbox_location").cloned().unwrap_or_default();
    assert!(loc.starts_with("demo/d"), "{loc}");
    let item = loc.trim_start_matches("demo/").to_owned();
    // at the start of the inbox
    app.press("label:Move to position: Start of inbox").unwrap();
    // the toast over the settings' foot goes first
    app.run_for(Duration::from_secs(3)).unwrap();
    app.wait_for_gone("text:Inbox set", SHOWS).unwrap();
    app.press("label:Done").unwrap();
    app.wait_for_gone("label:Change…", SHOWS).unwrap();
    // captured: two lines, two items
    app.press("label:Menu").unwrap();
    app.press("label:Capture to inbox").unwrap();
    app.wait_for("id:capture").unwrap();
    app.type_text("id:capture", "First thought\nSecond thought").unwrap();
    app.press("label:Add").unwrap();
    app.wait_for("text:Successfully captured to your inbox.").unwrap();
    // go to inbox: zoomed in on it, the new items first
    app.press("id:sidebar >> id:inbox").unwrap();
    expect(&mut app, "the inbox", |s| s.shows("First thought") && s.shows("Second thought"));
    let rows: Vec<String> = app
        .screen()
        .layout_nodes()
        .into_iter()
        .filter(|n| n.test_id.as_deref() == Some("row-text") && n.bounds.size.height.0 > 0.0)
        .filter_map(|n| n.text.as_ref().map(|t| t.text.text.clone()))
        .collect();
    assert_eq!(rows.first().map(String::as_str), Some("First thought"), "{rows:?} in {item}");
    assert_eq!(rows.get(1).map(String::as_str), Some("Second thought"));
}

#[test]
fn the_file_finder_opens_a_document() {
    let mut app = start(Os::MacOs, &fresh());
    app.press("id:sidebar >> id:search").unwrap();
    app.wait_for("id:finder-input").unwrap();
    app.type_text("id:finder-input", "untit").unwrap();
    expect(&mut app, "one choice", |s| s.shows("Untitled") && !s.shows("Search any document"));
    app.press("id:finder >> text:Untitled").unwrap();
    app.wait_for_gone("id:finder", SHOWS).unwrap();
    expect(&mut app, "the document", |s| title(s) == "Untitled");
    // Mod+O, Escape closes it
    app.press_key("mod+o").unwrap();
    app.wait_for("id:finder-input").unwrap();
    app.press_key("escape").unwrap();
    app.wait_for_gone("id:finder", SHOWS).unwrap();
}

#[test]
fn bookmark_this_view_keeps_the_search() {
    let mut app = start(Os::MacOs, &fresh());
    app.press("label:Search").unwrap();
    app.wait_for("id:search-input").unwrap();
    app.type_text("id:search-input", "plain").unwrap();
    expect(&mut app, "the filter", |s| !s.shows("Bold and italic and code 2"));
    app.press("id:search-box >> label:Bookmark this search").unwrap();
    app.wait_for("id:sidebar >> text:Demo (plain)").unwrap();
    app.press("id:search-box >> label:Close search").unwrap();
    expect(&mut app, "every row", |s| s.shows("Bold and italic and code 2"));
    // the bookmark opens the search again
    app.press("id:sidebar >> text:Demo (plain)").unwrap();
    expect(&mut app, "the search again", |s| !s.shows("Bold and italic and code 2") && count(s, "search-box") == 1);
}

#[test]
fn move_item_to_another_document_on_phones() {
    let mut app = start(Os::Ios, &fresh());
    app.press("text:Plain item 6").unwrap();
    expect(&mut app, "the toolbar", |s| count(s, "toolbar") == 1);
    app.press("label:Move item to").unwrap();
    app.wait_for("id:finder-input").unwrap();
    // not under itself
    app.type_text("id:finder-input", "plain item 6").unwrap();
    expect(&mut app, "no choice of itself", |s| !s.texts().iter().any(|t| t.starts_with("Demo › ") && t.ends_with("Plain item 6")));
    app.clear("id:finder-input").unwrap();
    app.type_text("id:finder-input", "untitled").unwrap();
    app.press("id:finder >> text:Untitled").unwrap();
    app.wait_for("text:Moved to Untitled").unwrap();
    expect(&mut app, "gone from here", |s| !s.shows("Plain item 6") && s.shows("Plain item 1"));
    app.press("label:Toggle sidebar").unwrap();
    app.press("id:sidebar >> text:Untitled").unwrap();
    expect(&mut app, "there", |s| title(s) == "Untitled" && s.shows("Plain item 6"));
}
