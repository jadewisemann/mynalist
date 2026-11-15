//! The editing features area 13 added, headless on the fixtures' small demo document: the keymap's formatting
//! and list commands, the item menu (right click, long press, its submenu and swatches), the hover toolbar, date
//! chips opening the date picker, Shift / command presses and drags selecting rows, items dragged by their
//! bullet, the finders ("Move to…", the file finder), the title and its note edited in place, and the top bar
//! menu's OPML import.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use nsh_core::geometry::point;
use nsh_core::platform::{ColorScheme, Idiom, Os};
use nsh_core::time::Clock;
use nsh_core::Points;
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, MemoryStorage, Screen};
use outliner_core::commands::{toggle_wrap, wrap_link};

/// How long a step's outcome may take to show (a busy machine delays batches): the tests wait for what they check.
const SHOWS: Duration = Duration::from_secs(10);
/// 2024-06-15 12:00 UTC, a Saturday.
const NOW_MS: u64 = 1_718_452_800_000;

/// The app on fixture document `doc` with the prefs `kept` (key, value) holds, its first text shown.
fn start_with(doc: &str, os: Os, kept: &[(&str, &str)], first: &str) -> Headless {
    std::env::set_var("TZ", "UTC");
    let root = outliner_app::fixture::root_with(doc, None, outliner_app::fixture::STORED, "");
    let mut config = EngineConfig::new(root);
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_millis(NOW_MS));
    config.environment.os = os;
    config.environment.color_scheme = ColorScheme::Light;
    let storage: Arc<MemoryStorage> = MemoryStorage::new(kept.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())));
    config.storage = Some(storage);
    if os == Os::MacOs {
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: Points(1100.0), height: Points(800.0) };
    }
    let mut app = Headless::start(config).unwrap();
    app.wait_for(&format!("text:{first}")).unwrap();
    app
}

fn start(os: Os) -> Headless {
    start_with("demo", os, &[], "Plain item 1")
}

/// The text of the input with test id `id` that shows (a spare cell out of view may hold another, sized 0).
fn input_on(screen: &Screen, id: &str) -> Option<String> {
    let shown = screen.layout_nodes().into_iter().any(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0);
    shown.then(|| screen.input_text(&format!("id:{id}")).ok().flatten().unwrap_or_default().to_owned())
}

fn expect_input(app: &mut Headless, id: &str, text: &str) {
    let waited = app.wait_until(&format!("{id} holding `{text}`"), SHOWS, |s| input_on(s, id).as_deref() == Some(text));
    assert!(waited.is_ok(), "{id} holds {:?}, not `{text}`", input_on(app.screen(), id));
}

fn expect_editing(app: &mut Headless, text: &str) {
    expect_input(app, "row-input", text);
}

fn edit_row(app: &mut Headless, shown: &str, source: &str) {
    app.press(&format!("text:{shown}")).unwrap();
    expect_editing(app, source);
}

/// How many views with test id `id` show.
fn count(s: &Screen, id: &str) -> usize {
    s.layout_nodes().into_iter().filter(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0).count()
}

fn expect_count(app: &mut Headless, id: &str, n: usize) {
    let waited = app.wait_until(&format!("{n} {id}"), SHOWS, move |s| count(s, id) == n);
    assert!(waited.is_ok(), "{} {id}, not {n}", count(app.screen(), id));
}

fn expect(app: &mut Headless, what: &str, ok: impl Fn(&Screen) -> bool + 'static) {
    let waited = app.wait_until(what, SHOWS, ok);
    assert!(waited.is_ok(), "{what}: the screen shows {:?}", app.screen().texts());
}

/// The bounds of the shown text `t`.
fn text_box(s: &Screen, t: &str) -> Option<nsh_core::Bounds<Points>> {
    s.layout_nodes()
        .into_iter()
        .find(|n| n.bounds.size.height.0 > 0.0 && n.text.as_ref().is_some_and(|x| x.text.text.trim() == t))
        .map(|n| n.bounds)
}

fn left_of_text(s: &Screen, t: &str) -> Option<f32> {
    text_box(s, t).map(|b| b.origin.x.0)
}

fn top_of_text(s: &Screen, t: &str) -> Option<f32> {
    text_box(s, t).map(|b| b.origin.y.0)
}

/// How many shown texts read `t`.
fn texts_reading(s: &Screen, t: &str) -> usize {
    s.texts().iter().filter(|x| x.trim() == t).count()
}

// ---- the keymap's commands --------------------------------------------------------------------------------

#[test]
fn format_keys_wrap_the_text_at_the_caret() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    // the caret at the end: Mod+B wraps the word there, as the web's `toggleWrap`
    let bold = toggle_wrap("Plain item 1", 12, 12, "**");
    app.press_key("mod+b").unwrap();
    expect_editing(&mut app, &bold.text);
    // again: unwrapped
    app.press_key("mod+b").unwrap();
    expect_editing(&mut app, "Plain item 1");
    // inline code, then undo
    app.press_key("mod+`").unwrap();
    expect_editing(&mut app, &toggle_wrap("Plain item 1", 12, 12, "`").text);
    app.press_key("mod+z").unwrap();
    expect_editing(&mut app, "Plain item 1");
    // a link (`wrapLink`), and a line break in the item (Mod+Shift+Return)
    app.press_key("mod+k").unwrap();
    expect_editing(&mut app, &wrap_link("Plain item 1", 12, 12).text);
    app.press_key("mod+z").unwrap();
    expect_editing(&mut app, "Plain item 1");
    app.press_key("mod+shift+enter").unwrap();
    expect_editing(&mut app, "Plain item 1\n");
}

#[test]
fn list_keys_act_on_the_item_and_the_selection() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Bold and italic and code 2", "**Bold** and __italic__ and `code` 2");
    // Mod+Shift+X numbers the item's children: "A link…" has one, now numbered "1."
    let numbers = |s: &Screen| s.texts().iter().filter(|t| t.trim() == "1.").count();
    let before = numbers(app.screen());
    app.press("text:A link and a #tag 3").unwrap();
    app.press_key("mod+shift+x").unwrap();
    expect(&mut app, "its child numbered", move |s| numbers(s) == before + 1);
    // Mod+Shift+H gives the item a heading: its text grows
    let h = text_box(app.screen(), "Plain item 6").unwrap().size.height.0;
    app.press("text:Plain item 6").unwrap();
    app.press_key("mod+shift+h").unwrap();
    expect(&mut app, "a heading", move |s| {
        // while edited it is the input: leave it to see the text
        input_on(s, "row-input").is_some()
    });
    app.press_key("escape").unwrap();
    expect(&mut app, "a taller row text", move |s| text_box(s, "Plain item 6").is_some_and(|b| b.size.height.0 > h));
}

// ---- the item menu ----------------------------------------------------------------------------------------

#[test]
fn the_item_menu_duplicates_sorts_and_sets_a_heading() {
    let mut app = start(Os::MacOs);
    app.right_click("id:bullet-box >> nth=0").unwrap();
    app.wait_for("id:popup").unwrap();
    for entry in ["Collapse", "Zoom in", "Add note", "Delete", "Sort", "Duplicate", "Number children", "Get link", "Export…"] {
        assert!(app.screen().shows(entry), "the menu has {entry}: {:?}", app.screen().texts());
    }
    // the keys of the same commands at the right
    assert!(app.screen().shows("⌘]"), "Zoom in's key: {:?}", app.screen().texts());
    // Sort ▸ Title Z→A: "Plain item 6" goes first among the item's children
    app.press("text:Sort").unwrap();
    app.wait_for("id:submenu").unwrap();
    app.press("text:Title Z→A").unwrap();
    expect(&mut app, "sorted Z→A", |s| {
        matches!((top_of_text(s, "Plain item 6"), top_of_text(s, "Bold and italic and code 2")), (Some(a), Some(b)) if a < b)
    });
    // Duplicate: a second "Plain item 1"
    app.right_click("id:bullet-box >> nth=0").unwrap();
    app.wait_for("id:popup").unwrap();
    app.press("text:Duplicate").unwrap();
    expect(&mut app, "two items", |s| texts_reading(s, "Plain item 1") == 2);
    app.press_key("mod+z").unwrap();
    expect(&mut app, "one item again", |s| texts_reading(s, "Plain item 1") == 1);
    // the heading swatches: H1 makes the text taller; the swatch of the current level is outlined
    let h = text_box(app.screen(), "Plain item 1").unwrap().size.height.0;
    app.right_click("id:bullet-box >> nth=0").unwrap();
    app.wait_for("id:popup").unwrap();
    // the menu is taller than the window: it scrolls to its end
    app.scroll("id:popup >> nth=0", Points(600.0)).unwrap();
    app.wait_for("id:swatches-heading").unwrap();
    app.press("id:swatches-heading >> text:H1").unwrap();
    app.wait_for_gone("id:popup", SHOWS).unwrap();
    expect(&mut app, "a heading", move |s| text_box(s, "Plain item 1").is_some_and(|b| b.size.height.0 > h));
    app.right_click("id:bullet-box >> nth=0").unwrap();
    app.wait_for("id:popup").unwrap();
    app.scroll("id:popup >> nth=0", Points(600.0)).unwrap();
    app.wait_for("id:swatches-heading").unwrap();
    assert!(app.screen().matches("id:swatches-heading >> id:swatch-current >> text:H1").unwrap());
    // Escape closes the menu
    app.press_key("escape").unwrap();
    app.wait_for_gone("id:popup", SHOWS).unwrap();
}

#[test]
fn the_item_menu_on_selected_items_acts_on_all_of_them() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Bold and italic and code 2", "**Bold** and __italic__ and `code` 2");
    app.press_key("shift+down").unwrap();
    app.press_key("shift+down").unwrap();
    expect_count(&mut app, "selected", 3);
    // the bullet of "Bold…2" (row 1) is one of them: Check off checks both
    app.right_click("id:bullet-box >> nth=1").unwrap();
    app.wait_for("id:popup").unwrap();
    assert!(!app.screen().shows("Zoom in"), "no Zoom in for several items");
    app.press("text:Check off").unwrap();
    app.wait_for_gone("id:popup", SHOWS).unwrap();
    // both are checked now: the menu on the other offers to uncheck them
    app.right_click("id:bullet-box >> nth=2").unwrap();
    app.wait_for("text:Uncheck").unwrap();
    app.press_key("escape").unwrap();
    app.wait_for_gone("id:popup", SHOWS).unwrap();
    // the expand row's buttons: level 1 folds the children of the top item
    app.right_click("id:bullet-box >> nth=0").unwrap();
    app.wait_for("id:popup").unwrap();
    app.press("id:expand-level >> nth=0").unwrap();
    expect(&mut app, "folded to level 1", |s| text_box(s, "한글 항목: 입력 테스트 4").is_none() && text_box(s, "Bold and italic and code 2").is_some());
}

#[test]
fn a_long_press_opens_the_item_menu_on_phones() {
    let mut app = start(Os::Ios);
    app.long_press("id:bullet-box >> nth=0").unwrap();
    app.wait_for("id:popup").unwrap();
    app.press("text:Add note").unwrap();
    expect_input(&mut app, "note-input", "");
}

#[test]
fn the_hover_toolbar_shows_over_the_row_with_its_setting() {
    let mut app = start_with("demo", Os::MacOs, &[("item_toolbar_hover", "true")], "Plain item 1");
    app.hover("text:Plain item 6").unwrap();
    app.wait_for("id:hover-bar").unwrap();
    // Duplicate from the bar
    app.press("id:hover-duplicate").unwrap();
    expect(&mut app, "two items", |s| texts_reading(s, "Plain item 6") == 2);
    // the note button edits the item's note
    app.hover("text:Plain item 1").unwrap();
    app.wait_for("id:hover-note").unwrap();
    app.press("id:hover-note").unwrap();
    expect_input(&mut app, "note-input", "");
    // off by default
    let mut plain = start(Os::MacOs);
    plain.hover("text:Plain item 6").unwrap();
    plain.wait(Duration::from_millis(50)).unwrap();
    assert_eq!(count(plain.screen(), "hover-bar"), 0);
}

// ---- date chips --------------------------------------------------------------------------------------------

#[test]
fn a_date_chip_opens_the_date_picker_on_its_date() {
    let mut app = start_with("inline", Os::MacOs, &[], "Bold, italic, strike and highlight");
    let chips = count(app.screen(), "date-chip");
    assert!(chips > 0);
    app.press("id:date-chip >> nth=0").unwrap();
    app.wait_for("id:datepicker").unwrap();
    // an existing date: the picker offers to delete it; Delete date takes it away
    app.press("text:Delete date").unwrap();
    app.wait_for_gone("id:datepicker", SHOWS).unwrap();
    // the caret is in the item after it: leave it to see its chips
    app.press_key("escape").unwrap();
    expect_count(&mut app, "date-chip", chips - 1);
}

// ---- selecting with the mouse --------------------------------------------------------------------------------

#[test]
fn shift_and_command_presses_select_rows() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Bold and italic and code 2", "**Bold** and __italic__ and `code` 2");
    // Shift+press on "Plain item 6": the items from "Bold…2" to it (5 rows)
    app.hold_keys("shift", true).unwrap();
    app.press("text:Plain item 6").unwrap();
    app.hold_keys("shift", false).unwrap();
    expect_count(&mut app, "selected", 5);
    // a command press adds an item (and its 6 children), another takes it out
    app.hold_keys("mod", true).unwrap();
    app.press("text:A link and a #tag 8").unwrap();
    expect_count(&mut app, "selected", 12);
    app.press("text:A link and a #tag 8").unwrap();
    app.hold_keys("mod", false).unwrap();
    expect_count(&mut app, "selected", 5);
}

#[test]
fn dragging_across_rows_selects_them() {
    let mut app = start(Os::MacOs);
    // from "Bold…2" down two rows: it, "A link…3" and its child selected, nothing edited
    app.drag("text:Bold and italic and code 2", point(Points(0.0), Points(60.0))).unwrap();
    expect_count(&mut app, "selected", 3);
    assert!(input_on(app.screen(), "row-input").is_none());
}

// ---- dragging items ----------------------------------------------------------------------------------------

#[test]
fn a_bullet_drags_its_item_to_another_place() {
    let mut app = start(Os::MacOs);
    let x = left_of_text(app.screen(), "Plain item 6").unwrap();
    // "Plain item 6" (row 5) dragged up to the upper half of "Bold…2" (row 1): a drop mark before it, the label
    app.drag_and_hold("id:bullet-box >> nth=5", point(Points(0.0), Points(-130.0))).unwrap();
    app.wait_for("id:drop-before").unwrap();
    app.wait_for("text:Move 1 item").unwrap();
    app.drop_held().unwrap();
    expect(&mut app, "moved up", |s| {
        matches!((top_of_text(s, "Plain item 6"), top_of_text(s, "Bold and italic and code 2")), (Some(a), Some(b)) if a < b)
    });
    expect_count(&mut app, "drop-before", 0);
    // the moved item is selected; undo puts it back
    expect_count(&mut app, "selected", 1);
    app.press_key("mod+z").unwrap();
    expect(&mut app, "back", move |s| {
        matches!((top_of_text(s, "Plain item 6"), top_of_text(s, "Bold and italic and code 2")), (Some(a), Some(b)) if a > b)
            && left_of_text(s, "Plain item 6") == Some(x)
    });
}

// ---- finders -----------------------------------------------------------------------------------------------

#[test]
fn move_to_puts_the_item_under_the_one_chosen() {
    let mut app = start(Os::MacOs);
    let x = left_of_text(app.screen(), "A link and a #tag 8").unwrap();
    edit_row(&mut app, "A link and a #tag 8", "A [link](https://example.com) and a #tag 8");
    app.press_key("mod+shift+m").unwrap();
    app.wait_for("id:finder").unwrap();
    app.type_text("id:finder-input", "Plain item 1").unwrap();
    app.wait_for("id:finder >> label:Plain item 1").unwrap();
    app.submit("id:finder-input").unwrap();
    app.wait_for_gone("id:finder", SHOWS).unwrap();
    app.wait_for("text:Moved to Plain item 1").unwrap();
    expect(&mut app, "one level in", move |s| left_of_text(s, "A link and a #tag 8").is_some_and(|l| l > x));
}

#[test]
fn the_file_finder_opens_a_document() {
    let mut app = start(Os::MacOs);
    app.press_key("mod+o").unwrap();
    app.wait_for("id:finder").unwrap();
    app.wait_for("id:finder >> label:Demo").unwrap();
    app.type_text("id:finder-input", "Untitled").unwrap();
    app.wait_for_gone("id:finder >> label:Demo", SHOWS).unwrap();
    app.press("id:finder >> label:Untitled").unwrap();
    app.wait_for_gone("id:finder", SHOWS).unwrap();
    expect(&mut app, "the other document", |s| !s.shows("Plain item 1"));
}

// ---- the title -----------------------------------------------------------------------------------------------

#[test]
fn the_title_and_its_note_are_edited_in_place() {
    let mut app = start(Os::MacOs);
    app.press("id:title-text").unwrap();
    expect_input(&mut app, "title-input", "Demo");
    // Shift+Return: its note, which keeps what is typed
    app.press_key("shift+enter").unwrap();
    expect_input(&mut app, "note-input", "");
    app.type_text("id:note-input", "about the demo").unwrap();
    app.press_key("escape").unwrap();
    app.wait_for("text:about the demo").unwrap();
    // a press on the note edits it again
    app.press("text:about the demo").unwrap();
    expect_input(&mut app, "note-input", "about the demo");
    // Shift+Return back to the title; typing renames the document
    app.press_key("shift+enter").unwrap();
    expect_input(&mut app, "title-input", "Demo");
    app.type_text("id:title-input", " two").unwrap();
    expect_input(&mut app, "title-input", "Demo two");
    // Down from the title goes to the first item
    app.press_key("down").unwrap();
    expect_editing(&mut app, "Plain item 1");
}

// ---- the top bar menu --------------------------------------------------------------------------------------

#[test]
fn the_top_bar_menu_offers_the_opml_import() {
    let mut app = start(Os::MacOs);
    app.press("label:Menu").unwrap();
    app.wait_for("text:Import OPML / text…").unwrap();
}
