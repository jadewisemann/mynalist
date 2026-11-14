//! The sidebar, headless: the local documents listed, switched between, made and renamed; the panel toggled and
//! dragged wider on desktops; the drawer and its scrim on phones; the tags of the open document.

use std::time::Duration;

use nsh_core::geometry::{point, Points};
use nsh_core::platform::{Idiom, Os};
use nsh_engine::{EngineConfig, Root};
use nsh_testing::{Headless, Screen};

/// How long a step's outcome may take to show (see `flows.rs`).
const SHOWS: Duration = Duration::from_secs(10);

fn start(os: Os) -> Headless {
    let mut config = EngineConfig::new(Root::of::<outliner_app::App>());
    config.environment.os = os;
    if os == Os::MacOs {
        // a desktop window, as the snapshot's: the sidebar's panel shows beside the document
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: nsh_core::Points(1100.0), height: nsh_core::Points(800.0) };
    }
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:Plain item 1").unwrap();
    app
}

/// The bounds of the view with test id `id` that shows (spare cells and hidden views are sized 0).
fn bounds_on(screen: &Screen, id: &str) -> Option<(f32, f32, f32, f32)> {
    screen
        .layout_nodes()
        .into_iter()
        .find(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.width.0 > 0.0 && n.bounds.size.height.0 > 0.0)
        .map(|n| (n.bounds.origin.x.0, n.bounds.origin.y.0, n.bounds.size.width.0, n.bounds.size.height.0))
}

fn shows_id(screen: &Screen, id: &str) -> bool {
    bounds_on(screen, id).is_some()
}

/// Waits until `ok` holds of the screen; fails with `what`.
fn expect(app: &mut Headless, what: &str, ok: impl Fn(&Screen) -> bool) {
    let waited = app.wait_until(what, SHOWS, ok);
    assert!(waited.is_ok(), "{what}: {waited:?}");
}

#[test]
fn desktop_lists_the_documents_and_switches_between_them() {
    let mut app = start(Os::MacOs);
    // the panel shows from the start, the open document's row first
    assert!(app.screen().shows("Demo"));
    app.wait_for("id:sidebar >> text:Untitled").unwrap();
    // an edit in the demo stays when it is left and opened again
    app.press("text:Plain item 1").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.type_text("id:row-input", "!").unwrap();
    app.press("id:sidebar >> text:Untitled").unwrap();
    expect(&mut app, "the empty document open", |s| !s.shows("Bold and italic and code 2"));
    // its title is its name; it has no rows
    assert!(app.screen().shows("Untitled"));
    assert!(!shows_id(app.screen(), "row"));
    app.press("id:sidebar >> text:Demo").unwrap();
    app.wait_for("text:Plain item 1!").unwrap();
}

#[test]
fn an_empty_document_gets_its_first_item_from_a_press_below_the_title() {
    let mut app = start(Os::MacOs);
    app.press("id:sidebar >> text:Untitled").unwrap();
    expect(&mut app, "the empty document open", |s| !s.shows("Plain item 1"));
    app.press("id:doc-end").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.type_text("id:row-input", "first").unwrap();
    app.press("id:sidebar >> text:Demo").unwrap();
    app.wait_for("text:Plain item 1").unwrap();
    app.press("id:sidebar >> text:Untitled").unwrap();
    app.wait_for("text:first").unwrap();
}

#[test]
fn a_new_document_opens_with_its_name_being_edited() {
    let mut app = start(Os::MacOs);
    app.press("id:plus").unwrap();
    app.wait_for("id:rename").unwrap();
    assert_eq!(app.screen().input_text("id:rename").unwrap(), Some("Untitled"));
    // the new document is open: the demo's rows are gone
    expect(&mut app, "the new document open", |s| !s.shows("Plain item 1"));
    app.clear("id:rename").unwrap();
    app.type_text("id:rename", "Plans").unwrap();
    app.submit("id:rename").unwrap();
    app.wait_for_gone("id:rename", SHOWS).unwrap();
    // the row and the title both show the name
    app.wait_for("id:sidebar >> text:Plans").unwrap();
    app.wait_for("id:title >> text:Plans").unwrap();
    assert!(app.screen().shows("Untitled"), "the empty document is still listed");
}

#[test]
fn a_long_press_renames_a_document_and_a_blank_name_is_not_kept() {
    let mut app = start(Os::Ios);
    app.press("label:Toggle sidebar").unwrap();
    app.wait_for("id:sidebar").unwrap();
    app.long_press("id:sidebar >> text:Demo").unwrap();
    app.wait_for("id:rename").unwrap();
    app.clear("id:rename").unwrap();
    app.submit("id:rename").unwrap();
    app.wait_for_gone("id:rename", SHOWS).unwrap();
    app.wait_for("id:sidebar >> text:Demo").unwrap();
    app.long_press("id:sidebar >> text:Demo").unwrap();
    app.wait_for("id:rename").unwrap();
    app.clear("id:rename").unwrap();
    app.type_text("id:rename", "Work").unwrap();
    app.submit("id:rename").unwrap();
    app.wait_for("id:sidebar >> text:Work").unwrap();
    app.wait_for("id:title >> text:Work").unwrap();
}

#[test]
fn the_desktop_panel_toggles_and_drags_wider() {
    let mut app = start(Os::MacOs);
    let (_, _, w, _) = bounds_on(app.screen(), "sidebar").expect("the panel shows");
    assert_eq!(w, base("space.sidebar"));
    let text_x = |s: &Screen| bounds_on(s, "row-text").map(|b| b.0).unwrap_or_default();
    let with_panel = text_x(app.screen());
    app.drag("id:splitter", point(Points(100.0), Points(0.0))).unwrap();
    expect(&mut app, "the panel 100 wider", |s| bounds_on(s, "sidebar").is_some_and(|b| (b.2 - (w + 100.0)).abs() < 1.0));
    expect(&mut app, "the document moved right", |s| text_x(s) > with_panel + 99.0);
    // never narrower than its least width
    app.drag("id:splitter", point(Points(-400.0), Points(0.0))).unwrap();
    let least = base("space.sidebar_min");
    expect(&mut app, "the panel at its least", |s| bounds_on(s, "sidebar").is_some_and(|b| (b.2 - least).abs() < 1.0));
    app.press("label:Toggle sidebar").unwrap();
    expect(&mut app, "the panel hidden", |s| !shows_id(s, "sidebar"));
    expect(&mut app, "the document at the left", |s| text_x(s) < with_panel - least + 1.0);
    app.press("label:Toggle sidebar").unwrap();
    expect(&mut app, "the panel back", |s| shows_id(s, "sidebar"));
}

#[test]
fn the_phone_drawer_opens_over_a_scrim_and_closes() {
    let mut app = start(Os::Ios);
    assert!(!shows_id(app.screen(), "sidebar"), "closed at first on phones");
    app.press("label:Toggle sidebar").unwrap();
    app.wait_for("id:scrim").unwrap();
    let (x, _, w, _) = bounds_on(app.screen(), "sidebar").unwrap();
    let screen_w = bounds_on(app.screen(), "scrim").unwrap().2;
    assert_eq!(x, 0.0);
    let want = (screen_w * base("space.drawer"))
        .min(base("space.sidebar_max"));
    assert!((w - want).abs() < 0.5, "the drawer is {w} wide, not {want}");
    // the scrim right of the drawer
    app.press_at("id:scrim", point(Points(screen_w - 10.0), Points(400.0))).unwrap();
    app.wait_for_gone("id:scrim", SHOWS).unwrap();
    assert!(!shows_id(app.screen(), "sidebar"));
    // opening another document closes it
    app.press("label:Toggle sidebar").unwrap();
    app.press("id:sidebar >> text:Untitled").unwrap();
    app.wait_for_gone("id:scrim", SHOWS).unwrap();
    expect(&mut app, "the empty document open", |s| !s.shows("Plain item 1"));
}

#[test]
fn the_tags_tab_counts_the_open_documents_tags() {
    let mut app = start(Os::MacOs);
    app.press("id:sidebar >> text:Tags").unwrap();
    // the demo writes `#tag` in every fifth of its 10 000 items; the empty document has none (section heads are
    // in capitals, as the web's)
    app.wait_for("text:▾ DOCUMENT TAGS (1)").unwrap();
    assert!(app.screen().shows("▾ ALL TAGS (1)"));
    assert_eq!(app.screen().count("text:(2000)").unwrap(), 2);
    app.press("text:▾ DOCUMENT TAGS (1)").unwrap();
    app.wait_for("text:▸ DOCUMENT TAGS (1)").unwrap();
    assert_eq!(app.screen().count("text:(2000)").unwrap(), 1);
    // the empty document: none of its own, the demo's among all
    app.press("id:sidebar >> text:Files").unwrap();
    app.press("id:sidebar >> text:Untitled").unwrap();
    app.press("id:sidebar >> text:Tags").unwrap();
    app.wait_for("text:▸ DOCUMENT TAGS (0)").unwrap();
    assert!(app.screen().shows("▾ ALL TAGS (1)"));
}

/// A value of the base look on a desktop (the tests run in the default modes).
fn base(id: &str) -> f32 {
    outliner_app::appearance::Looks::default().desktop.number(id)
}
