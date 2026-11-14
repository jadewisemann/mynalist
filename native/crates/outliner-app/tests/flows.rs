//! The spike's flows, headless: the demo document shows, rows edit in an input, and the outliner's keys work
//! while it has the focus. Each test is a fresh surface with the 10 000-item demo document.

use std::time::{Duration, Instant};

use nsh_core::platform::{Idiom, Os};
use nsh_engine::{EngineConfig, Root};
use nsh_testing::Headless;

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

/// The left edge of the row input (its row's indentation).
fn input_left(app: &Headless) -> f32 {
    let node = app.screen().layout_nodes().into_iter().find(|n| n.test_id.as_deref() == Some("row-input"));
    node.expect("the row input is laid out").bounds.origin.x.0
}

fn editing(app: &Headless) -> String {
    app.screen().input_text("id:row-input").unwrap().unwrap_or_default().to_owned()
}

/// Press a row's text (as shown) to edit it; the input holds its source, markup included.
fn edit_row(app: &mut Headless, shown: &str, source: &str) {
    app.press(&format!("text:{shown}")).unwrap();
    app.wait_for("id:row-input").unwrap();
    assert_eq!(editing(app), source);
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
    assert_eq!(editing(&app), "Plain item 1! 한글");
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
    assert_eq!(editing(&app), "");
    app.type_text("id:row-input", "new row").unwrap();
    app.press("text:Bold and italic and code 2").unwrap();
    app.wait_for("text:new row").unwrap();
    let texts = app.screen().texts();
    let at = |t: &str| texts.iter().position(|x| *x == t).unwrap();
    assert_eq!(at("new row"), at("Plain item 1") + 1);
}

#[test]
fn desktop_keys_indent_outdent_move_and_undo() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Bold and italic and code 2", "**Bold** and *italic* and `code` 2");
    let nested = input_left(&app);
    app.press_key("shift+tab").unwrap();
    let outdented = input_left(&app);
    assert!(outdented < nested, "{outdented} < {nested}");
    app.press_key("tab").unwrap();
    assert_eq!(input_left(&app), nested);
    app.press_key("down").unwrap();
    assert_eq!(editing(&app), "A [link](https://example.com) and a #tag 3");
    app.press_key("up").unwrap();
    app.press_key("up").unwrap();
    assert_eq!(editing(&app), "Plain item 1");
    // undo the indent, then the outdent
    app.press_key("mod+z").unwrap();
    app.press_key("mod+z").unwrap();
    // undo puts the caret back where the change was made
    assert_eq!(editing(&app), "**Bold** and *italic* and `code` 2");
    assert_eq!(input_left(&app), nested);
}

#[test]
fn scrolling_far_down_stays_fast() {
    let mut app = start(Os::Ios);
    let t = Instant::now();
    app.scroll("id:outline", px(20_000.0)).unwrap();
    assert!(t.elapsed() < Duration::from_secs(20), "scrolled in {:?}", t.elapsed());
    assert!(!app.screen().shows("Plain item 1"));
}

fn px(v: f32) -> nsh_core::geometry::Points {
    nsh_core::prelude::px(v)
}
