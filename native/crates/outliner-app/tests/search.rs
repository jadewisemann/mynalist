//! Search, links, paste and export, headless on the fixtures' small demo document: the search box filters the
//! rows and marks the matched words, its tips and the results screens (flat, every document), search and
//! replace; `[[` and `#` suggestions, a press on a link to an item, backlinks; a paste of several lines, of HTML,
//! of one line; the export dialog.

use std::time::Duration;

use nsh_core::platform::{Idiom, Os};
use nsh_core::PasteEvent;
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, MemoryStorage, Screen};

/// How long a step's outcome may take to show (a busy machine delays batches): the tests wait for what they check.
const SHOWS: Duration = Duration::from_secs(10);

fn start_with(os: Os, prefs: &[(&str, &str)]) -> Headless {
    // the prefs kept on the device, as the app starts with them
    let mut config = EngineConfig::new(outliner_app::fixture::root_with("demo", None, outliner_app::fixture::STORED, ""));
    config.environment.os = os;
    config.storage = Some(MemoryStorage::new(prefs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))));
    if os == Os::MacOs {
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: nsh_core::Points(1100.0), height: nsh_core::Points(800.0) };
    }
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:Plain item 1").unwrap();
    app
}

fn start(os: Os) -> Headless {
    start_with(os, &[])
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

fn expect(app: &mut Headless, what: &str, f: impl Fn(&Screen) -> bool) {
    let waited = app.wait_until(what, SHOWS, f);
    assert!(waited.is_ok(), "not {what}: {:?}", app.screen().texts());
}

/// How many views with test id `id` show.
fn count(s: &Screen, id: &str) -> usize {
    s.layout_nodes().into_iter().filter(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0).count()
}

fn edit_row(app: &mut Headless, shown: &str, source: &str) {
    app.press(&format!("text:{shown}")).unwrap();
    expect_input(app, "row-input", source);
}

fn has_match_bg(app: &Headless, selector: &str) -> bool {
    app.screen().span_style(selector).is_ok_and(|s| s.background.is_some())
}

// ---- search ---------------------------------------------------------------------------------------------

#[test]
fn the_search_box_filters_the_rows_and_marks_the_words() {
    let mut app = start(Os::MacOs);
    app.press("label:Search").unwrap();
    expect_input(&mut app, "search-input", "");
    app.type_text("id:search-input", "plain").unwrap();
    // only the matches (and their ancestors) show, the matched word marked
    expect(&mut app, "the filter", |s| s.shows("Plain item 1") && !s.shows("Bold and italic and code 2"));
    assert!(has_match_bg(&app, "text:/^Plain item 1$/ >> text:Plain"));
    assert!(!has_match_bg(&app, "text:/^Plain item 1$/ >> text:item"));
    // the tips show while the query is typed
    expect(&mut app, "the tips", |s| s.shows("Flat search") && s.shows("Search in all documents"));
    // Escape ends the search: every row again, nothing marked
    app.press_key("escape").unwrap();
    expect(&mut app, "every row", |s| s.shows("Bold and italic and code 2") && count(s, "search-box") == 0);
    assert!(!has_match_bg(&app, "text:/^Plain item 1$/ >> text:Plain"));
}

#[test]
fn closing_the_search_puts_the_caret_back() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 6", "Plain item 6");
    app.press_key("mod+f").unwrap();
    expect_input(&mut app, "search-input", "");
    app.type_text("id:search-input", "item 6").unwrap();
    expect(&mut app, "the filter", |s| s.shows("Plain item 6") && !s.shows("Bold and italic and code 2"));
    app.press("label:Close search").unwrap();
    expect_input(&mut app, "row-input", "Plain item 6");
    assert!(app.screen().shows("Bold and italic and code 2"));
}

#[test]
fn a_tag_press_searches_for_it() {
    let mut app = start(Os::MacOs);
    app.press("text:/^A link and a #tag 3$/ >> text:#tag").unwrap();
    expect_input(&mut app, "search-input", "#tag");
    // the matches and their ancestors
    expect(&mut app, "the tag's items", |s| s.shows("A link and a #tag 3") && !s.shows("Bold and italic and code 2"));
}

#[test]
fn the_flat_and_global_results_open_their_item() {
    let mut app = start(Os::MacOs);
    app.press("label:Search").unwrap();
    expect_input(&mut app, "search-input", "");
    app.type_text("id:search-input", "item 1").unwrap();
    expect(&mut app, "the tips", |s| s.shows("Flat search"));
    app.press("label:Flat search").unwrap();
    expect(&mut app, "the flat results", |s| s.shows("Search results for \"item 1\" in \"Demo\""));
    // "Plain item 1", "Plain item 11", ... and the ones under them
    let flat = count(app.screen(), "result");
    assert!(flat >= 2, "{flat} results");
    app.press("label:Go back [Esc]").unwrap();
    expect_input(&mut app, "search-input", "item 1");
    // every document: Shift is the flat search, Mod the global one (from the box, which "Go back" leaves without
    // the focus, as the web's route does)
    app.press("label:Search").unwrap();
    app.press_key("mod+enter").unwrap();
    expect(&mut app, "the global results", |s| s.shows("Search everywhere") && count(s, "results-card") == 1);
    app.press("label:Plain item 11").unwrap();
    expect_input(&mut app, "row-input", "Plain item 11");
    assert!(app.screen().shows("Bold and italic and code 2"), "the search ended");
}

#[test]
fn phones_search_from_the_top_bar() {
    let mut app = start(Os::Ios);
    app.press("label:Search").unwrap();
    expect_input(&mut app, "search-input", "");
    app.type_text("id:search-input", "한글").unwrap();
    expect(&mut app, "the filter", |s| s.shows("한글 항목: 입력 테스트 4") && !s.shows("Bold and italic and code 2"));
    app.press("label:Close search").unwrap();
    expect(&mut app, "every row", |s| s.shows("Plain item 1"));
}

#[test]
fn search_and_replace_replaces_in_one_undo_step() {
    let mut app = start(Os::MacOs);
    app.press("label:Menu").unwrap();
    app.press("label:Search and replace").unwrap();
    app.type_text("id:replace-find", "Plain").unwrap();
    expect(&mut app, "the count", |s| s.texts().iter().any(|t| t.ends_with("matches in this document")));
    app.type_text("id:replace-with", "Flat").unwrap();
    app.press("label:Replace all").unwrap();
    expect(&mut app, "replaced", |s| s.shows("Flat item 1") && !s.shows("Plain item 1"));
    expect(&mut app, "the toast", |s| s.texts().iter().any(|t| t.starts_with("Replaced ")));
    edit_row(&mut app, "Flat item 1", "Flat item 1");
    app.press_key("mod+z").unwrap();
    expect(&mut app, "undone", |s| s.shows("Plain item 6") && !s.shows("Flat item 6"));
}

// ---- links ----------------------------------------------------------------------------------------------

#[test]
fn double_brackets_suggest_items_and_write_a_link() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.type_text("id:row-input", " [[").unwrap();
    expect(&mut app, "suggestions", |s| count(s, "suggestion") == 10);
    app.type_text("id:row-input", "bold italic code 2").unwrap();
    // the items' path is the one they had when the list opened (the edited item's text then)
    expect(&mut app, "the item first", |s| s.shows("Demo › Plain item 1 [[") && count(s, "suggestion") >= 1);
    // Down and Up move in the list, Return picks
    app.press_key("down").unwrap();
    app.press_key("up").unwrap();
    app.press_key("enter").unwrap();
    expect_input(&mut app, "row-input", "Plain item 1 [Bold and italic and code 2](#d=demo&z=d1)");
    expect(&mut app, "the list closed", |s| count(s, "suggestion") == 0);
    // a press on the link opens the item
    app.press_key("escape").unwrap();
    app.press("text:/^Plain item 1 Bold/ >> text:Bold and italic and code 2").unwrap();
    expect(&mut app, "zoomed in on the item", |s| !s.shows("Plain item 1 Bold and italic and code 2") && s.shows("Bold and italic and code 2"));
}

#[test]
fn hash_suggests_the_tags_in_use() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    app.type_text("id:row-input", " #t").unwrap();
    expect(&mut app, "the tag", |s| count(s, "suggestion") == 1 && s.shows("#tag"));
    app.press("label:#tag").unwrap();
    expect_input(&mut app, "row-input", "Plain item 1 #tag ");
    // Escape with the list showing closes it only
    app.type_text("id:row-input", "#").unwrap();
    expect(&mut app, "the list", |s| count(s, "suggestion") == 1);
    app.press_key("escape").unwrap();
    expect(&mut app, "no list", |s| count(s, "suggestion") == 0);
    expect_input(&mut app, "row-input", "Plain item 1 #tag #");
}

#[test]
fn backlinks_show_under_the_zoomed_item_and_as_counts() {
    let mut app = start_with(Os::MacOs, &[("show_backlinks", "true")]);
    edit_row(&mut app, "Plain item 6", "Plain item 6");
    app.type_text("id:row-input", " [[bold italic code 2").unwrap();
    expect(&mut app, "suggestions", |s| count(s, "suggestion") >= 1);
    app.press_key("enter").unwrap();
    app.press_key("escape").unwrap();
    // the item linked to shows a count; a press lists the item linking
    expect(&mut app, "the count", |s| count(s, "backlink-count") == 1);
    app.press("label:1 backlinks").unwrap();
    expect(&mut app, "the references", |s| s.shows("References") && count(s, "result") == 1);
    app.press("label:Close").unwrap();
    // zoomed in on it: the items linking to it under the document
    app.press("text:/^Plain item 6 Bold/ >> text:Bold and italic and code 2").unwrap();
    expect(&mut app, "the backlinks", |s| s.shows("This item is referenced in:") && count(s, "result") == 1);
    app.press("label:Plain item 6 Bold and italic and code 2").unwrap();
    expect_input(&mut app, "row-input", "Plain item 6 [Bold and italic and code 2](#d=demo&z=d1)");
}

// ---- paste ----------------------------------------------------------------------------------------------

fn paste(app: &mut Headless, text: &str, html: &str) {
    let mut ev = PasteEvent::default();
    ev.text = text.to_owned();
    ev.html = html.to_owned();
    app.paste("id:row-input", &ev).unwrap();
}

#[test]
fn several_lines_paste_as_items_and_one_line_as_text() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 1", "Plain item 1");
    // one line stays text (its indentation trimmed, as the web's)
    paste(&mut app, "  pasted", "");
    expect_input(&mut app, "row-input", "Plain item 1pasted");
    paste(&mut app, "\nfirst\n    under it\nsecond", "");
    expect_input(&mut app, "row-input", "second");
    expect(&mut app, "the items", |s| s.shows("first") && s.shows("under it") && s.shows("Plain item 1pasted"));
    // one undo step takes them away
    app.press_key("mod+z").unwrap();
    expect(&mut app, "undone", |s| !s.shows("under it"));
}

#[test]
fn html_pastes_as_items() {
    let mut app = start(Os::MacOs);
    edit_row(&mut app, "Plain item 6", "Plain item 6");
    paste(&mut app, "One Two", "<ul><li>One</li><li>Two</li></ul>");
    // the caret at the end of the last
    expect_input(&mut app, "row-input", "Two");
    expect(&mut app, "the items", |s| s.shows("Plain item 6One"));
}

// ---- export ---------------------------------------------------------------------------------------------

#[test]
fn the_export_dialog_shows_the_document_in_three_formats() {
    let mut app = start(Os::MacOs);
    app.press("label:Menu").unwrap();
    app.press("label:Export…").unwrap();
    expect(&mut app, "the dialog", |s| s.shows("Export \"Demo\""));
    let text = input_on(app.screen(), "export-output").unwrap_or_default();
    assert!(text.starts_with("Plain item 1\n    **Bold** and __italic__ and `code` 2\n"), "{text}");
    app.press("label:Dashes").unwrap();
    expect(&mut app, "dashes", |s| input_on(s, "export-output").is_some_and(|t| t.starts_with("- Plain item 1\n    - **Bold**")));
    app.press("label:OPML").unwrap();
    expect(&mut app, "OPML", |s| input_on(s, "export-output").is_some_and(|t| t.contains("<opml") && t.contains("text=\"Plain item 1\"")));
    app.press("label:Formatted (HTML)").unwrap();
    expect(&mut app, "HTML", |s| input_on(s, "export-output").is_some_and(|t| t.contains("<li>Plain item 1")));
    app.press("label:Copy").unwrap();
    expect(&mut app, "copied", |s| s.clipboard().is_some_and(|c| c.contains("<li>Plain item 1")) && s.shows("Copied!"));
    assert!(app.screen().shows("Download"));
}

#[test]
fn phones_export_the_selected_items_by_copying() {
    let mut app = start(Os::Ios);
    app.press("label:Select multiple items").unwrap();
    app.press("text:Plain item 6").unwrap();
    app.press("label:Menu").unwrap();
    app.press("label:Export…").unwrap();
    expect(&mut app, "the dialog", |s| s.shows("Export \"Plain item 6\""));
    assert_eq!(input_on(app.screen(), "export-output").as_deref(), Some("Plain item 6"));
    assert!(!app.screen().shows("Download"));
}
