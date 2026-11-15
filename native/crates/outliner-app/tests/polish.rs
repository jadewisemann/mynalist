//! Area 15's follow-ups, headless: a document's own view options (checked items, notes, the article layout) kept
//! on its workspace entry, menus whose submenus open under the pointer, the file finder's folder paths, search
//! bookmarks that open their results screen, and items dragged by a bullet: the list scrolling near its edges,
//! and dropped onto the Files pane (into a document, or becoming documents).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use nsh_core::geometry::point;
use nsh_core::platform::{Idiom, Os};
use nsh_core::time::Clock;
use nsh_core::{Bounds, Points};
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, MemoryStorage, Screen};

const SHOWS: Duration = Duration::from_secs(10);

fn start_on(root: nsh_core::element::Root, kept: &Arc<MemoryStorage>) -> Headless {
    let mut config = EngineConfig::new(root);
    config.environment.os = Os::MacOs;
    config.environment.idiom = Idiom::Desktop;
    config.viewport = nsh_core::Size { width: Points(1100.0), height: Points(800.0) };
    config.storage = Some(kept.clone());
    // a manual clock: waits move it a frame at a time (the saves, the drag's scrolling)
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_secs(1_718_452_800));
    Headless::start(config).unwrap()
}

/// The app as it starts (the small demo and an empty document), keeping its documents in `kept`.
fn started(kept: &Arc<MemoryStorage>) -> Headless {
    let mut app = start_on(outliner_app::fixture::started_root(), kept);
    app.wait_for("text:Plain item 1").unwrap();
    app
}

/// Fixture document `doc`, the sidebar as `side` says (`fixture::FixtureProps::side`).
fn fixture(doc: &str, side: &str, first: &str) -> Headless {
    let root = outliner_app::fixture::root_side(doc, None, outliner_app::fixture::STORED, "", side);
    let mut app = start_on(root, &fresh());
    app.wait_for(&format!("text:{first}")).unwrap();
    app
}

fn fresh() -> Arc<MemoryStorage> {
    MemoryStorage::new(std::iter::empty())
}

fn expect(app: &mut Headless, what: &str, f: impl Fn(&Screen) -> bool) {
    let waited = app.wait_until(what, SHOWS, f);
    assert!(waited.is_ok(), "not {what}: {:?}", app.screen().texts());
}

fn expect_kept(app: &mut Headless, kept: &Arc<MemoryStorage>, what: &str, ok: impl Fn(&BTreeMap<String, String>) -> bool) {
    let watched = kept.clone();
    let waited = app.wait_until(what, SHOWS, move |_| ok(&watched.entries()));
    assert!(waited.is_ok(), "{what}: {:?}", kept.entries().get("doc:workspace"));
}

/// How many views with test id `id` show.
fn count(s: &Screen, id: &str) -> usize {
    s.layout_nodes().into_iter().filter(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0).count()
}

/// The bounds of the shown text `t` (the first one, or the one inside the view with test id `within`).
fn text_box(s: &Screen, t: &str) -> Option<Bounds<Points>> {
    s.layout_nodes()
        .into_iter()
        .find(|n| n.bounds.size.height.0 > 0.0 && n.text.as_ref().is_some_and(|x| x.text.text.trim() == t))
        .map(|n| n.bounds)
}

/// The bounds of the `nth` shown view with test id `id`.
fn nth_box(s: &Screen, id: &str, nth: usize) -> Option<Bounds<Points>> {
    s.layout_nodes()
        .into_iter()
        .filter(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0)
        .nth(nth)
        .map(|n| n.bounds)
}

/// The height of the row showing text `t`.
fn row_height(s: &Screen, t: &str) -> Option<f32> {
    let (_, y) = center(text_box(s, t)?);
    s.layout_nodes()
        .into_iter()
        .filter(|n| n.test_id.as_deref() == Some("row"))
        .find(|n| y >= n.bounds.origin.y.0 && y < n.bounds.origin.y.0 + n.bounds.size.height.0)
        .map(|n| n.bounds.size.height.0)
}

fn center(b: Bounds<Points>) -> (f32, f32) {
    (b.origin.x.0 + b.size.width.0 / 2.0, b.origin.y.0 + b.size.height.0 / 2.0)
}

/// The text of the shown sidebar file rows, top down.
fn file_names(s: &Screen) -> Vec<String> {
    s.layout_nodes()
        .into_iter()
        .filter(|n| n.bounds.size.height.0 > 0.0 && n.test_id.as_deref() == Some("file-name"))
        .filter_map(|n| n.text.map(|t| t.text.text.trim().to_owned()))
        .collect()
}

/// Opens the View options menu and the submenu of its entry `entry` under the pointer.
fn view_option(app: &mut Headless, entry: &str, choice: &str) {
    app.press("id:view-options").unwrap();
    app.wait_for("id:popup").unwrap();
    app.hover(&format!("id:popup >> text:{entry}")).unwrap();
    app.wait_for("id:submenu").unwrap();
    app.press(&format!("id:submenu >> text:{choice}")).unwrap();
    app.wait_for_gone("id:popup", SHOWS).unwrap();
}

// ---- a document's own view options -------------------------------------------------------------------------

/// Row 10 of the demo, which is checked.
const CHECKED: &str = "Mixed 굵게 코드 text 10";

#[test]
fn checked_items_and_the_layout_are_the_documents_own_and_kept() {
    let kept = fresh();
    let mut app = started(&kept);
    assert!(app.screen().shows(CHECKED));
    // the setting's value beside "Global", ticked while chosen
    app.press("id:view-options").unwrap();
    app.hover("id:popup >> text:Checked items").unwrap();
    app.wait_for("id:submenu >> text:✓ Global (show)").unwrap();
    app.press("id:submenu >> text:Hide").unwrap();
    expect(&mut app, "the checked item hidden", |s| !s.shows(CHECKED) && s.shows("Plain item 1"));
    expect_kept(&mut app, &kept, "kept on the entry", |e| {
        e.get("doc:workspace").is_some_and(|w| w.contains(r#""v_checked":"\"hide\"""#))
    });
    // the other document does not follow it; this one keeps it when opened again
    app.press("id:sidebar >> text:Untitled").unwrap();
    expect(&mut app, "the other document", |s| !s.shows("Plain item 1"));
    app.press("id:sidebar >> text:Demo").unwrap();
    app.wait_for("text:Plain item 1").unwrap();
    assert!(!app.screen().shows(CHECKED));
    // the article layout: more room above and below the rows (no guides or bullets: the structural snapshot's)
    let row = row_height(app.screen(), "Plain item 1").unwrap();
    view_option(&mut app, "Layout", "Article");
    let waited = app.wait_until("taller rows", SHOWS, move |s| row_height(s, "Plain item 1").is_some_and(|h| h > row));
    assert!(waited.is_ok(), "an article row is {:?} high, a list row {row}", row_height(app.screen(), "Plain item 1"));
    view_option(&mut app, "Layout", "List");
    expect(&mut app, "the rows as they were", move |s| row_height(s, "Plain item 1") == Some(row));
    // "Global" again takes the choice away
    view_option(&mut app, "Checked items", "Global (show)");
    expect(&mut app, "the checked item again", |s| s.shows(CHECKED));
    expect_kept(&mut app, &kept, "taken off the entry", |e| e.get("doc:workspace").is_some_and(|w| w.contains(r#""v_checked":null"#)));
}

#[test]
fn notes_follow_the_documents_choice() {
    let mut app = fixture("looks", "", "A checked item");
    assert!(app.screen().shows("Its note is checked too."));
    view_option(&mut app, "Notes", "Hide");
    expect(&mut app, "the note hidden", |s| !s.shows("Its note is checked too.") && s.shows("A checked item"));
    view_option(&mut app, "Notes", "1st line");
    expect(&mut app, "the note's first line", |s| s.shows("Its note is checked too."));
}

#[test]
fn submenus_open_under_the_pointer() {
    let mut app = fixture("demo", "", "Plain item 1");
    app.press("id:view-options").unwrap();
    app.hover("id:popup >> text:Notes").unwrap();
    app.wait_for("id:submenu >> text:1st line").unwrap();
    // another entry with a submenu replaces it; one without closes it
    app.hover("id:popup >> text:Layout").unwrap();
    app.wait_for("id:submenu >> text:Article").unwrap();
    assert!(!app.screen().shows("1st line"));
    app.hover("id:popup >> text:Lock document (read-only)").unwrap();
    app.wait_for_gone("id:submenu", SHOWS).unwrap();
    app.press_key("escape").unwrap();
    app.wait_for_gone("id:popup", SHOWS).unwrap();
    // the item menu's Sort
    app.right_click("id:bullet-box >> nth=0").unwrap();
    app.hover("id:popup >> text:Sort").unwrap();
    app.wait_for("id:submenu >> text:Title A→Z").unwrap();
}

// ---- the file finder ---------------------------------------------------------------------------------------

#[test]
fn the_file_finder_shows_where_documents_are() {
    let mut app = fixture("demo", "folders", "Plain item 1");
    app.press_key("mod+o").unwrap();
    app.wait_for("id:finder-input").unwrap();
    app.type_text("id:finder-input", "budget").unwrap();
    // in folder "Projects", archived
    app.wait_for("id:finder >> text:Projects (archived)").unwrap();
    app.clear("id:finder-input").unwrap();
    app.type_text("id:finder-input", "notes").unwrap();
    app.wait_for("id:finder >> text:Old").unwrap();
}

// ---- search bookmarks --------------------------------------------------------------------------------------

/// The Bookmarks tab's star: bookmarks what shows.
fn bookmark(app: &mut Headless, name: &str) {
    app.press("id:tabs >> text:Bookmarks").unwrap();
    app.press("id:sidebar >> id:star").unwrap();
    app.wait_for(&format!("id:sidebar >> text:{name}")).unwrap();
}

#[test]
fn search_bookmarks_open_the_results_they_were_made_from() {
    let kept = fresh();
    let mut app = started(&kept);
    app.press("label:Search").unwrap();
    app.type_text("id:search-input", "item 1").unwrap();
    expect(&mut app, "the tips", |s| s.shows("Flat search"));
    app.press("label:Flat search").unwrap();
    expect(&mut app, "the flat results", |s| s.shows("Search results for \"item 1\" in \"Demo\""));
    bookmark(&mut app, "Demo (item 1)");
    // every document's results
    app.press("label:Go back [Esc]").unwrap();
    app.press("label:Search").unwrap();
    app.press_key("mod+enter").unwrap();
    expect(&mut app, "the global results", |s| s.shows("Search everywhere") && count(s, "results-card") == 1);
    bookmark(&mut app, "Search: item 1");
    expect_kept(&mut app, &kept, "both kept as the web keeps them", |e| {
        e.get("doc:workspace").is_some_and(|w| w.contains(r#""flat":true"#) && w.contains(r#""global":true"#))
    });
    app.press_key("escape").unwrap();
    expect(&mut app, "the document", |s| s.shows("Plain item 1") && !s.shows("Search everywhere"));
    // each opens its results screen again
    app.press("id:sidebar >> text:Demo (item 1)").unwrap();
    expect(&mut app, "the flat results again", |s| s.shows("Search results for \"item 1\" in \"Demo\""));
    app.press("id:sidebar >> text:Search: item 1").unwrap();
    expect(&mut app, "the global results again", |s| s.shows("Search everywhere") && count(s, "results-card") == 1);
}

// ---- dragging items ----------------------------------------------------------------------------------------

/// Drags the bullet of row `nth` (held, not dropped) to window point `to`.
fn drag_bullet_to(app: &mut Headless, nth: usize, to: (f32, f32)) {
    let from = center(nth_box(app.screen(), "bullet-box", nth).unwrap());
    app.drag_and_hold(&format!("id:bullet-box >> nth={nth}"), point(Points(to.0 - from.0), Points(to.1 - from.1))).unwrap();
}

#[test]
fn dragging_items_near_the_bottom_scrolls_the_list() {
    let mut app = fixture("demo", "", "Plain item 1");
    let top = text_box(app.screen(), "Plain item 6").unwrap().origin.y.0;
    let x = center(nth_box(app.screen(), "bullet-box", 5).unwrap()).0;
    drag_bullet_to(&mut app, 5, (x + 40.0, 790.0));
    app.wait_for("text:Move 1 item").unwrap();
    app.run_for(Duration::from_millis(1000)).unwrap();
    // the rows went up under the held pointer
    expect(&mut app, "scrolled", move |s| text_box(s, "Plain item 1").is_none_or(|b| b.origin.y.0 < top - 200.0));
    app.drop_held().unwrap();
    app.wait_for_gone("text:Move 1 item", SHOWS).unwrap();
    // let go: the scrolling stops
    let now = text_box(app.screen(), "Plain item 11").map(|b| b.origin.y.0);
    app.run_for(Duration::from_millis(500)).unwrap();
    assert_eq!(text_box(app.screen(), "Plain item 11").map(|b| b.origin.y.0), now);
}

#[test]
fn items_dropped_on_a_document_in_the_sidebar_move_into_it() {
    let mut app = fixture("demo", "", "Plain item 1");
    let to = center(text_box(app.screen(), "Untitled").unwrap());
    drag_bullet_to(&mut app, 5, to);
    app.wait_for("text:Move 1 item").unwrap();
    app.drop_held().unwrap();
    app.wait_for("text:Moved to Untitled").unwrap();
    expect(&mut app, "gone from here", |s| !s.shows("Plain item 6") && s.shows("Plain item 1"));
    app.press("id:sidebar >> text:Untitled").unwrap();
    app.wait_for("text:Plain item 6").unwrap();
}

#[test]
fn items_dropped_on_the_files_pane_become_documents() {
    let mut app = fixture("demo", "", "Plain item 1");
    let below = center(text_box(app.screen(), "Untitled").unwrap());
    drag_bullet_to(&mut app, 5, (below.0, below.1 + 120.0));
    app.wait_for("text:Move 1 item").unwrap();
    app.drop_held().unwrap();
    app.wait_for("text:Created a document from the item").unwrap();
    expect(&mut app, "the new document", |s| file_names(s).iter().any(|n| n == "Plain item 6"));
    assert!(!app.screen().shows("Plain item 6") || file_names(app.screen()).contains(&"Plain item 6".to_owned()));
}

// ---- the menu's last entries -------------------------------------------------------------------------------

#[test]
fn the_menu_has_about_and_the_workflowy_import() {
    let mut app = fixture("demo", "", "Plain item 1");
    app.press("label:Menu").unwrap();
    app.press("label:About").unwrap();
    app.wait_for("id:about >> text:Sync: local only").unwrap();
    app.press("label:Okay").unwrap();
    app.wait_for_gone("id:about", SHOWS).unwrap();
    app.press("label:Menu").unwrap();
    app.press("label:Import from WorkFlowy…").unwrap();
    app.wait_for("text:Import from WorkFlowy").unwrap();
    app.press("label:Cancel").unwrap();
    app.wait_for_gone("id:workflowy-dialog", SHOWS).unwrap();
}
