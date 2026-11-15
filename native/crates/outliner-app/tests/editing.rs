//! Editing beyond the item's text, headless on the fixtures' small demo document: notes (Shift+Return, the edges,
//! the phone toolbar's button), selected items (Shift+Up/Down, Mod+Shift+A, Escape, commands on them, the phone's
//! select mode) and the date picker (Mod+D, the toolbar's button, a typed `!`).

use std::time::{Duration, SystemTime};

use nsh_core::platform::{Idiom, Os};
use nsh_core::time::Clock;
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, Screen};

/// How long a step's outcome may take to show (a busy machine delays batches): the tests wait for what they check.
const SHOWS: Duration = Duration::from_secs(10);
/// The clock the date picker's tests run on: 2024-06-15 12:00 UTC, a Saturday.
const NOW_MS: u64 = 1_718_452_800_000;

fn start(os: Os) -> Headless {
    std::env::set_var("TZ", "UTC");
    let mut config = EngineConfig::new(outliner_app::fixture::root("demo", None));
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_millis(NOW_MS));
    config.environment.os = os;
    if os == Os::MacOs {
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: nsh_core::Points(1100.0), height: nsh_core::Points(800.0) };
    }
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:Plain item 1").unwrap();
    app
}

/// The text of the input with test id `id` that shows (a spare cell out of view may hold another, sized 0).
fn input_on(screen: &Screen, id: &str) -> Option<String> {
    let shown = screen.layout_nodes().into_iter().any(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0);
    shown.then(|| screen.input_text(&format!("id:{id}")).ok().flatten().unwrap_or_default().to_owned())
}

/// Waits until the input `id` shows holding `text`.
fn expect_input(app: &mut Headless, id: &str, text: &str) {
    let waited = app.wait_until(&format!("{id} holding `{text}`"), SHOWS, |s| input_on(s, id).as_deref() == Some(text));
    assert!(waited.is_ok(), "{id} holds {:?}, not `{text}`", input_on(app.screen(), id));
}

fn expect_editing(app: &mut Headless, text: &str) {
    expect_input(app, "row-input", text);
}

/// Waits until neither the row's input nor the note's shows.
fn expect_nothing_edited(app: &mut Headless) {
    let gone = app.wait_until("nothing edited", SHOWS, |s| input_on(s, "row-input").is_none() && input_on(s, "note-input").is_none());
    assert!(gone.is_ok(), "still editing {:?} / note {:?}", input_on(app.screen(), "row-input"), input_on(app.screen(), "note-input"));
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
    let waited = app.wait_until(&format!("{n} {id}"), SHOWS, |s| count(s, id) == n);
    assert!(waited.is_ok(), "{} {id}, not {n}", count(app.screen(), id));
}

fn left_of(s: &Screen, id: &str) -> Option<f32> {
    s.layout_nodes()
        .into_iter()
        .find(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0)
        .map(|n| n.bounds.origin.x.0)
}

// ---- notes ----------------------------------------------------------------------------------------------

#[test]
fn shift_return_moves_between_the_content_and_the_note() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    let notes = count(app.screen(), "note");
    // to the item's note, empty and edited under its content
    app.press_key("shift+enter").unwrap();
    expect_input(&mut app, "note-input", "");
    // Return breaks the line in a note
    app.type_text("id:note-input", "a note").unwrap();
    app.press_key("enter").unwrap();
    app.type_text("id:note-input", "two lines").unwrap();
    expect_input(&mut app, "note-input", "a note\ntwo lines");
    // back to the end of the content; the note shows under it
    app.press_key("shift+enter").unwrap();
    expect_editing(&mut app, "Plain item 1");
    app.wait_for("text:a note\ntwo lines").unwrap();
    assert_eq!(count(app.screen(), "note"), notes + 1);
    // a press on the note edits it; Up from its first line goes to the content
    app.press("text:a note\ntwo lines").unwrap();
    expect_input(&mut app, "note-input", "a note\ntwo lines");
    app.press_key("up").unwrap();
    app.press_key("up").unwrap();
    expect_editing(&mut app, "Plain item 1");
    // Down from the content skips the note, to the row below
    app.press_key("down").unwrap();
    expect_editing(&mut app, "**Bold** and __italic__ and `code` 2");
    // Down from a note's last line goes to the row below
    app.press("text:a note\ntwo lines").unwrap();
    expect_input(&mut app, "note-input", "a note\ntwo lines");
    app.press_key("down").unwrap();
    expect_editing(&mut app, "**Bold** and __italic__ and `code` 2");
    // undo takes the note away again
    for _ in 0..3 {
        app.press_key("mod+z").unwrap();
    }
    let gone = app.wait_until("the note gone", SHOWS, |s| !s.shows("a note\ntwo lines") && !s.shows("a note"));
    assert!(gone.is_ok());
}

#[test]
fn an_empty_note_takes_no_line_and_backspace_leaves_it() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Bold and italic and code 2", "**Bold** and __italic__ and `code` 2");
    let notes = count(app.screen(), "note");
    app.press_key("shift+enter").unwrap();
    expect_input(&mut app, "note-input", "");
    // Backspace in the empty note: back to the end of the content, no note shown
    app.press_key("backspace").unwrap();
    expect_editing(&mut app, "**Bold** and __italic__ and `code` 2");
    expect_count(&mut app, "note-input", 0);
    assert_eq!(count(app.screen(), "note"), notes);
    // Tab in a note types a tab; the item stays where it is
    let left = left_of(app.screen(), "row-input").unwrap();
    app.press_key("shift+enter").unwrap();
    expect_input(&mut app, "note-input", "");
    app.press_key("tab").unwrap();
    expect_input(&mut app, "note-input", "\t");
    app.press_key("escape").unwrap();
    expect_nothing_edited(&mut app);
    app.press("text:Bold and italic and code 2").unwrap();
    let same = app.wait_until("not indented", SHOWS, |s| left_of(s, "row-input") == Some(left));
    assert!(same.is_ok(), "{:?} for {left}", left_of(app.screen(), "row-input"));
}

#[test]
fn the_phone_toolbar_edits_the_note() {
    let mut app = start(Os::Ios);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.wait_for("id:toolbar").unwrap();
    app.press("label:Edit note").unwrap();
    expect_input(&mut app, "note-input", "");
    // the toolbar is for the item's text: it goes while the note is edited (`MobileBar.update`)
    app.wait_for_gone("id:toolbar", SHOWS).unwrap();
    app.type_text("id:note-input", "from the phone").unwrap();
    app.press("text:Plain item 1").unwrap();
    expect_editing(&mut app, "Plain item 1");
    app.wait_for("text:from the phone").unwrap();
    app.wait_for("id:toolbar").unwrap();
}

// ---- selected items -------------------------------------------------------------------------------------

#[test]
fn shift_down_selects_items_and_escape_lets_go() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    // from a one-line row: the item and the rows under it show selected, nothing edited
    app.press_key("shift+down").unwrap();
    expect_nothing_edited(&mut app);
    expect_count(&mut app, "selected", 7);
    // its next sibling joins, with its children
    app.press_key("shift+down").unwrap();
    expect_count(&mut app, "selected", 14);
    // and leaves again
    app.press_key("shift+up").unwrap();
    expect_count(&mut app, "selected", 7);
    // Escape: nothing selected; again: the caret at the start of the first of them
    app.press_key("escape").unwrap();
    expect_count(&mut app, "selected", 0);
    expect_nothing_edited(&mut app);
    app.press_key("escape").unwrap();
    expect_editing(&mut app, "Plain item 1");
    // Mod+Shift+A: every item at the top
    app.press_key("shift+mod+a").unwrap();
    expect_nothing_edited(&mut app);
    let all = app.wait_until("every row selected", SHOWS, |s| count(s, "selected") > 0 && count(s, "selected") == count(s, "row"));
    assert!(all.is_ok(), "{} of {} rows", count(app.screen(), "selected"), count(app.screen(), "row"));
    // Up above the first: nothing there, they stay selected; Left edits the first at its start
    app.press_key("up").unwrap();
    app.press_key("left").unwrap();
    expect_editing(&mut app, "Plain item 1");
    expect_count(&mut app, "selected", 0);
}

#[test]
fn commands_act_on_the_selected_items() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "A link and a #tag 8", "A [link](https://example.com) and a #tag 8");
    let top = left_of(app.screen(), "row-input").unwrap();
    app.press_key("shift+up").unwrap();
    expect_count(&mut app, "selected", 7);
    // Tab: under the item above, still selected
    app.press_key("tab").unwrap();
    let moved = app.wait_until("indented", SHOWS, |s| {
        s.layout_nodes().into_iter().any(|n| n.test_id.as_deref() == Some("selected") && n.bounds.origin.x.0 > top)
    });
    assert!(moved.is_ok());
    expect_count(&mut app, "selected", 7);
    // Escape twice: the first selected item edited, indented
    app.press_key("escape").unwrap();
    app.press_key("escape").unwrap();
    expect_editing(&mut app, "A [link](https://example.com) and a #tag 8");
    let indented = left_of(app.screen(), "row-input").unwrap();
    assert!(indented > top, "{indented} after Tab, {top} before");
    // Backspace deletes the selected items; Mod+Z brings them back, selected
    app.press_key("shift+down").unwrap();
    expect_count(&mut app, "selected", 7);
    app.press_key("backspace").unwrap();
    let gone = app.wait_until("deleted", SHOWS, |s| !s.shows("A link and a #tag 8"));
    assert!(gone.is_ok());
    app.press_key("mod+z").unwrap();
    app.wait_for("text:A link and a #tag 8").unwrap();
    expect_count(&mut app, "selected", 7);
    // Return replaces them with one empty item, edited
    app.press_key("enter").unwrap();
    expect_editing(&mut app, "");
    assert!(!app.screen().shows("A link and a #tag 8"));
}

#[test]
fn copying_selected_items_puts_their_text_on_the_clipboard() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Mixed 굵게 코드 text 5", "Mixed **굵게** `코드` text 5");
    app.press_key("shift+down").unwrap();
    app.press_key("shift+down").unwrap();
    expect_count(&mut app, "selected", 2);
    app.press_key("mod+c").unwrap();
    let copied = app.wait_until("copied", SHOWS, |s| s.clipboard().is_some());
    assert!(copied.is_ok());
    assert_eq!(app.screen().clipboard(), Some("Mixed **굵게** `코드` text 5\nPlain item 6\n"));
}

#[test]
fn the_phone_selects_several_items_in_select_mode() {
    let mut app = start(Os::Ios);
    app.press("label:Select multiple items").unwrap();
    // a press selects an item instead of editing it
    app.press("text:Plain item 6").unwrap();
    expect_count(&mut app, "selected", 1);
    expect_nothing_edited(&mut app);
    app.wait_for("id:toolbar").unwrap();
    app.wait_for("label:Edit note").unwrap();
    app.press("text:A link and a #tag 8").unwrap();
    expect_count(&mut app, "selected", 8);
    // several items: the buttons for one item go (`SINGLE_ONLY`)
    app.wait_for_gone("label:Edit note", SHOWS).unwrap();
    // pressed again: out of the selection
    app.press("text:Plain item 6").unwrap();
    expect_count(&mut app, "selected", 7);
    app.press("text:Plain item 6").unwrap();
    expect_count(&mut app, "selected", 8);
    app.press("label:Delete item").unwrap();
    let gone = app.wait_until("deleted", SHOWS, |s| !s.shows("Plain item 6") && !s.shows("A link and a #tag 8"));
    assert!(gone.is_ok());
    // select mode off: a press edits again
    app.press("label:Select multiple items").unwrap();
    edit_row(&mut app, "Plain item 1", "Plain item 1");
}

// ---- the date picker --------------------------------------------------------------------------------------

#[test]
fn mod_d_inserts_edits_and_deletes_a_date() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.press_key("mod+d").unwrap();
    app.wait_for("id:datepicker").unwrap();
    // today first, in its month
    app.wait_for("text:June 2024").unwrap();
    app.wait_for("text:Sat, Jun 15").unwrap();
    app.press("label:2024-06-20").unwrap();
    app.wait_for("text:Thu, Jun 20").unwrap();
    app.press("label:Done").unwrap();
    app.wait_for_gone("id:datepicker", SHOWS).unwrap();
    // after the word, a space before it; the caret after it
    expect_editing(&mut app, "Plain item 1 !(2024-06-20)");
    // the caret in the date: that date, a time added
    app.press_key("mod+d").unwrap();
    app.wait_for("text:Thu, Jun 20").unwrap();
    app.wait_for("label:Delete date").unwrap();
    app.type_text("id:dp-time", "3pm").unwrap();
    app.press("label:Done").unwrap();
    expect_editing(&mut app, "Plain item 1 !(2024-06-20 15:00)");
    // a time that is none: its error, the picker stays
    app.press_key("mod+d").unwrap();
    app.clear("id:dp-time").unwrap();
    app.type_text("id:dp-time", "soon").unwrap();
    app.press("label:Done").unwrap();
    app.wait_for("text:Please enter a valid time format like 22:42").unwrap();
    // Delete date: the date and its space gone
    app.press("label:Delete date").unwrap();
    expect_editing(&mut app, "Plain item 1");
}

#[test]
fn the_picker_keys_move_the_day_and_a_repeat_is_written() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.press_key("mod+d").unwrap();
    app.wait_for("text:Sat, Jun 15").unwrap();
    // a day on, a week on, a month on
    app.press_key("right").unwrap();
    app.press_key("down").unwrap();
    app.press_key("pagedown").unwrap();
    app.wait_for("text:Tue, Jul 23").unwrap();
    app.wait_for("text:July 2024").unwrap();
    app.press("label:Weekly on Tuesday").unwrap();
    app.press("label:From completion").unwrap();
    // Return is Done
    app.press_key("enter").unwrap();
    expect_editing(&mut app, "Plain item 1 !(2024-07-23 | ~1w)");
    // a range: start, then end
    app.press_key("mod+d").unwrap();
    app.press("label:Create date range").unwrap();
    app.wait_for("text:Tue, Jul 23 – Tue, Jul 23 · pick the end date").unwrap();
    app.press("label:2024-07-25").unwrap();
    app.press("label:Does not repeat").unwrap();
    app.press("label:Done").unwrap();
    expect_editing(&mut app, "Plain item 1 !(2024-07-23 - 2024-07-25)");
    // Escape cancels: the text as it was
    app.press_key("mod+d").unwrap();
    app.press("label:2024-07-01").unwrap();
    app.press_key("escape").unwrap();
    app.wait_for_gone("id:datepicker", SHOWS).unwrap();
    expect_editing(&mut app, "Plain item 1 !(2024-07-23 - 2024-07-25)");
}

#[test]
fn a_typed_exclamation_mark_opens_the_picker_in_its_place() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    // after a word, `!` is typed text
    app.type_text("id:row-input", "!").unwrap();
    expect_editing(&mut app, "Plain item 1!");
    assert_eq!(count(app.screen(), "datepicker"), 0);
    app.type_text("id:row-input", " ").unwrap();
    app.type_text("id:row-input", "!").unwrap();
    app.wait_for("id:datepicker").unwrap();
    // Done: the date in place of the `!`
    app.press("label:Done").unwrap();
    expect_editing(&mut app, "Plain item 1! !(2024-06-15)");
}

#[test]
fn the_phone_toolbar_adds_a_date() {
    let mut app = start(Os::Ios);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.press("label:Add date").unwrap();
    app.wait_for("id:datepicker").unwrap();
    app.press("label:2024-06-01").unwrap();
    app.press("label:Done").unwrap();
    expect_editing(&mut app, "Plain item 1 !(2024-06-01)");
}
