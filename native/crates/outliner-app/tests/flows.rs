//! The spike's flows, headless: the demo document shows, rows edit in an input, and the outliner's keys work
//! while it has the focus. Each test is a fresh surface with the 10 000-item demo document.

use std::time::{Duration, Instant};

use nsh_core::platform::{Idiom, Os};
use nsh_engine::{EngineConfig, Root};
use nsh_testing::{Headless, Screen};

/// How long a step's outcome may take to show. A key's batches can come after `press_key` settled when the
/// machine is busy (the whole suite at once), so the tests wait for what they check instead of reading it once.
const SHOWS: Duration = Duration::from_secs(10);

fn start(os: Os) -> Headless {
    let mut config = EngineConfig::new(Root::of::<outliner_app::App>());
    config.environment.os = os;
    if os == Os::MacOs {
        config.environment.idiom = Idiom::Desktop;
    }
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:Plain item 1").unwrap();
    app
}

/// The left edge of the row input (its row's indentation). A list keeps spare cells out of view, sized 0, with
/// the last row they showed: only the input that shows counts.
fn input_left_on(screen: &Screen) -> Option<f32> {
    let node = screen
        .layout_nodes()
        .into_iter()
        .find(|n| n.test_id.as_deref() == Some("row-input") && n.bounds.size.height.0 > 0.0);
    node.map(|n| n.bounds.origin.x.0)
}

fn input_left(app: &Headless) -> f32 {
    input_left_on(app.screen()).expect("the row input is laid out")
}

fn editing_on(screen: &Screen) -> String {
    screen.input_text("id:row-input").ok().flatten().unwrap_or_default().to_owned()
}

fn editing(app: &Headless) -> String {
    editing_on(app.screen())
}

/// Waits until the row input holds `source`; fails with what it holds then.
fn expect_editing(app: &mut Headless, source: &str) {
    let waited = app.wait_until(&format!("editing `{source}`"), SHOWS, |s| editing_on(s) == source);
    assert!(waited.is_ok(), "editing `{}`, not `{source}`", editing(app));
}

/// Waits until the row input's left edge satisfies `ok`; fails with where it is then.
fn expect_left(app: &mut Headless, what: &str, ok: impl Fn(f32) -> bool) {
    let waited = app.wait_until(what, SHOWS, |s| input_left_on(s).is_some_and(&ok));
    assert!(waited.is_ok(), "{what}: the row input's left is {:?}", input_left_on(app.screen()));
}

/// Press a row's text (as shown) to edit it; the input holds its source, markup included.
fn edit_row(app: &mut Headless, shown: &str, source: &str) {
    app.press(&format!("text:{shown}")).unwrap();
    app.wait_for("id:row-input").unwrap();
    expect_editing(app, source);
}

#[test]
fn shows_the_demo_document_formatted() {
    let app = start(Os::Ios);
    // a formatted row is one text of spans: its characters read whole, without the markup
    assert!(app.screen().shows("Bold and italic and code 2"));
    assert!(app.screen().shows("Mixed 굵게 코드 text 5"));
    // only the rows in view are made out of the 10 000
    assert!(app.screen().count("role:button").unwrap() < 200);
}

#[test]
fn typing_edits_the_item() {
    let mut app = start(Os::Ios);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.type_text("id:row-input", "! 한글").unwrap();
    expect_editing(&mut app, "Plain item 1! 한글");
    // leaving the row shows the edited text
    app.press("text:Bold and italic and code 2").unwrap();
    app.wait_for("text:Plain item 1! 한글").unwrap();
}

#[test]
fn return_splits_into_a_new_row_below() {
    let mut app = start(Os::Ios);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.submit("id:row-input").unwrap();
    // the caret was at the end: an empty row below takes it
    expect_editing(&mut app, "");
    app.type_text("id:row-input", "new row").unwrap();
    app.press("text:Bold and italic and code 2").unwrap();
    app.wait_for("text:new row").unwrap();
    let texts = app.screen().texts();
    let at = |t: &str| texts.iter().position(|x| *x == t).unwrap();
    assert_eq!(at("new row"), at("Plain item 1") + 1);
}

/// The row input's height, while it shows.
fn input_height_on(screen: &Screen) -> Option<f32> {
    let node = screen
        .layout_nodes()
        .into_iter()
        .find(|n| n.test_id.as_deref() == Some("row-input") && n.bounds.size.height.0 > 0.0);
    node.map(|n| n.bounds.size.height.0)
}

#[test]
fn a_long_row_wraps_in_its_input_and_return_still_splits_it() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    let one_line = input_height_on(app.screen()).unwrap();
    let long = " and a sentence long enough to need more than one line of the editor's width".repeat(3);
    app.type_text("id:row-input", &long).unwrap();
    let source = format!("Plain item 1{long}");
    expect_editing(&mut app, &source);
    // the input grows with its text instead of scrolling it sideways
    let grew = app.wait_until("the input taller", SHOWS, |s| input_height_on(s).is_some_and(|h| h > 1.5 * one_line));
    assert!(grew.is_ok(), "the input is {:?} tall, one line {one_line}", input_height_on(app.screen()));
    // Shift+Return and Alt+Return break no line in the item's text
    app.press_key("shift+enter").unwrap();
    app.press_key("alt+enter").unwrap();
    expect_editing(&mut app, &source);
    // Return splits the row: the caret was at the end, an empty row below takes it
    app.press_key("enter").unwrap();
    expect_editing(&mut app, "");
}

#[test]
fn the_bullet_thread_follows_the_row_being_edited() {
    let mut app = start(Os::MacOs);
    let count = |s: &Screen, id: &str| {
        s.layout_nodes().into_iter().filter(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0).count()
    };
    // nothing edited: no thread
    assert_eq!(count(app.screen(), "elbow") + count(app.screen(), "thread"), 0);
    // a top-level row: its bullet only, no line
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    assert_eq!(count(app.screen(), "elbow") + count(app.screen(), "thread"), 0);
    // its third child (depth 1, under a sibling with a child): down from the parent's bullet, past the rows
    // above, and a turn into the row's bullet
    edit_row(&mut app, "Mixed 굵게 코드 text 5", "Mixed **굵게** `코드` text 5");
    let drawn = app.wait_until("the thread drawn", SHOWS, |s| count(s, "elbow") == 1 && count(s, "thread") == 4);
    assert!(drawn.is_ok(), "{} turns, {} runs", count(app.screen(), "elbow"), count(app.screen(), "thread"));
}

#[test]
fn desktop_keys_indent_outdent_move_and_undo() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Bold and italic and code 2", "**Bold** and __italic__ and `code` 2");
    let nested = input_left(&app);
    app.press_key("shift+tab").unwrap();
    expect_left(&mut app, "outdented", |x| x < nested);
    app.press_key("tab").unwrap();
    expect_left(&mut app, "indented again", |x| x == nested);
    // Dynalist's rules: outdenting left its siblings below under the old parent, so indenting again made it
    // that parent's last child; the row below is now the parent's next sibling
    app.press_key("down").unwrap();
    expect_editing(&mut app, "A [link](https://example.com) and a #tag 8");
    app.press_key("up").unwrap();
    expect_editing(&mut app, "**Bold** and __italic__ and `code` 2");
    // Mod+Down moves it past the next row; Mod+Up brings it back
    app.press_key("mod+down").unwrap();
    app.press_key("mod+up").unwrap();
    app.press_key("up").unwrap();
    expect_editing(&mut app, "**Bold** and __italic__ and `code` 7");
    app.press_key("down").unwrap();
    expect_editing(&mut app, "**Bold** and __italic__ and `code` 2");
    // undo both moves, the indent and the outdent
    for _ in 0..4 {
        app.press_key("mod+z").unwrap();
    }
    // undo puts the caret back where the change was made, at the row's first place
    expect_editing(&mut app, "**Bold** and __italic__ and `code` 2");
    expect_left(&mut app, "back where it was", |x| x == nested);
    app.press_key("down").unwrap();
    expect_editing(&mut app, "A [link](https://example.com) and a #tag 3");
}

#[test]
fn zoom_in_and_out() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.press_key("mod+]").unwrap();
    // only its children show; the caret is on the first one
    expect_editing(&mut app, "**Bold** and __italic__ and `code` 2");
    let gone = app.wait_until("the parent's next sibling gone", SHOWS, |s| !s.shows("A link and a #tag 8"));
    assert!(gone.is_ok(), "{gone:?}");
    app.press_key("mod+[").unwrap();
    app.wait_for("text:A link and a #tag 8").unwrap();
    expect_editing(&mut app, "Plain item 1");
}

#[test]
fn scrolling_far_down_stays_fast() {
    let mut app = start(Os::Ios);
    let t = Instant::now();
    app.scroll("id:outline", px(20_000.0)).unwrap();
    assert!(t.elapsed() < Duration::from_secs(20), "scrolled in {:?}", t.elapsed());
    let gone = app.wait_until("the first row gone", SHOWS, |s| !s.shows("Plain item 1"));
    assert!(gone.is_ok(), "{gone:?}");
}

fn px(v: f32) -> nsh_core::geometry::Points {
    nsh_core::prelude::px(v)
}
