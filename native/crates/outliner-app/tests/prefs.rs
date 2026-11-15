//! The settings beyond the look, headless: the Preferences that change the outline (checked items, notes, dates,
//! the bullet), the Keymap tab and the keys it binds, the Sync & data tab's backup and import, and the date
//! picker's popover under the caret on desktops. Each starts on a fixture with the prefs a memory storage holds.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use nsh_core::platform::{ColorScheme, Idiom, Os};
use nsh_core::time::Clock;
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, MemoryStorage, Screen};

/// How long a step's outcome may take to show (see `flows.rs`).
const SHOWS: Duration = Duration::from_secs(10);
/// 2024-06-15 12:00 UTC, a Saturday (the date tests' clock).
const NOW_MS: u64 = 1_718_452_800_000;

fn storage(entries: &[(&str, &str)]) -> Arc<MemoryStorage> {
    MemoryStorage::new(entries.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())))
}

/// The app on fixture document `doc` with the prefs `kept` holds, as a desktop (or a phone), its first text shown.
fn start(doc: &str, kept: &Arc<MemoryStorage>, os: Os, first: &str) -> Headless {
    std::env::set_var("TZ", "UTC");
    let root = outliner_app::fixture::root_with(doc, None, outliner_app::fixture::STORED, "");
    let mut config = EngineConfig::new(root);
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_millis(NOW_MS));
    config.environment.os = os;
    config.environment.color_scheme = ColorScheme::Light;
    config.storage = Some(kept.clone());
    if os == Os::MacOs {
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: nsh_core::Points(1100.0), height: nsh_core::Points(800.0) };
    }
    let mut app = Headless::start(config).unwrap();
    app.wait_for(&format!("text:{first}")).unwrap();
    app
}

fn entries(kept: &MemoryStorage) -> BTreeMap<String, String> {
    kept.entries()
}

/// Presses `label` and waits until the device keeps `key` as `value` (`None`: not kept).
fn set(app: &mut Headless, kept: &Arc<MemoryStorage>, label: &str, key: &str, value: Option<&str>) {
    app.press(&format!("label:{label}")).unwrap();
    let k = kept.clone();
    let want = value.map(str::to_owned);
    let waited = app.wait_until(label, SHOWS, move |_| k.entries().get(key) == want.as_ref());
    assert!(waited.is_ok(), "{label}: {key} is {:?}, not {value:?}", entries(kept).get(key));
}

fn expect(app: &mut Headless, what: &str, ok: impl Fn(&Screen) -> bool + 'static) {
    let waited = app.wait_until(what, SHOWS, ok);
    assert!(waited.is_ok(), "{what}: the screen shows {:?}", app.screen().texts());
}

/// The bounds of the shown view with test id `id`.
fn bounds(s: &Screen, id: &str) -> Option<nsh_core::Bounds<nsh_core::Points>> {
    s.layout_nodes().into_iter().find(|n| n.test_id.as_deref() == Some(id) && n.bounds.size.height.0 > 0.0).map(|n| n.bounds)
}

#[test]
fn viewing_settings_hide_checked_items_and_notes_and_stay() {
    let kept = storage(&[]);
    let mut app = start("looks", &kept, Os::MacOs, "A checked item");
    app.press("label:Settings").unwrap();
    // notes: hidden, then on their first line, then shown again (the default is not kept)
    set(&mut app, &kept, "Notes: Hide", "show_notes", Some("hide"));
    expect(&mut app, "the note hidden", |s| !s.shows("Its note is checked too."));
    set(&mut app, &kept, "Notes: 1st line", "show_notes", Some("teaser"));
    expect(&mut app, "the note's first line", |s| s.shows("Its note is checked too."));
    set(&mut app, &kept, "Notes: Show", "show_notes", None);
    // checked items hidden, and still hidden at the next start
    set(&mut app, &kept, "Show checked items", "show_checked", Some("false"));
    expect(&mut app, "no checked item", |s| !s.shows("A checked item") && !s.shows("A checkbox, on"));
    assert!(app.screen().shows("A checkbox, off"));
    drop(app);
    let app = start("looks", &kept, Os::MacOs, "A checkbox, off");
    assert!(!app.screen().shows("A checked item"));
    // a stored value this app does not read is left alone; a bad one counts as the default
    let kept = storage(&[("show_checked", "maybe"), ("tag_bg", "true")]);
    let mut app = start("looks", &kept, Os::MacOs, "A checked item");
    app.press("label:Settings").unwrap();
    set(&mut app, &kept, "Notes: Hide", "show_notes", Some("hide"));
    assert_eq!(entries(&kept).get("show_checked").map(String::as_str), Some("maybe"));
    assert_eq!(entries(&kept).get("tag_bg").map(String::as_str), Some("true"));
}

#[test]
fn date_settings_change_labels_and_the_picker() {
    // 24-hour times and a pattern for days without a time
    let kept = storage(&[("time_format", "24h"), ("custom_date_display_format_no_time", "YYYY-MM-DD")]);
    let mut app = start("inline", &kept, Os::MacOs, "Sun, March 10th 2024, 15:30 - 17:00");
    assert!(app.screen().shows("2024-01-05"), "{:?}", app.screen().texts());
    // the picker: the week from Monday once that is chosen, the time as 24-hour
    app.press("label:Settings").unwrap();
    set(&mut app, &kept, "First day of week: Monday", "start_day", Some("mon"));
    app.press("label:Done").unwrap();
    app.press("text:Item with a note").unwrap();
    app.wait_for("id:row-input").unwrap();
    // no space before the `!`: typing it does not open the picker
    app.type_text("id:row-input", " x!(2024-06-20 15:00)").unwrap();
    app.press_key("mod+d").unwrap();
    app.wait_for("id:datepicker").unwrap();
    let texts = app.screen().texts();
    let at = |t: &str| texts.iter().position(|x| *x == t).unwrap_or_else(|| panic!("no {t} in {texts:?}"));
    assert!(at("Mo") < at("Su"), "{texts:?}");
    assert_eq!(app.screen().input_text("id:dp-time").unwrap(), Some("15:00"));
}

#[test]
fn the_picker_is_a_popover_under_the_caret_on_desktops() {
    let kept = storage(&[]);
    let mut app = start("demo", &kept, Os::MacOs, "Plain item 1");
    app.press("text:Plain item 1").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.type_text("id:row-input", " x").unwrap();
    app.press_key("mod+d").unwrap();
    app.wait_for("id:datepicker").unwrap();
    expect(&mut app, "laid out", |s| bounds(s, "datepicker").is_some());
    let s = app.screen();
    let (picker, scrim) = (bounds(s, "datepicker").unwrap(), bounds(s, "dp-scrim").unwrap());
    // the first row is near the top: the popover goes under it, at the caret's left (after the text), not in the
    // middle of the screen
    let row = s.layout_nodes().into_iter().find(|n| n.test_id.as_deref() == Some("row")).unwrap().bounds;
    assert!(picker.origin.y.0 >= row.origin.y.0 + 10.0, "{picker:?} under {row:?}");
    assert!(picker.origin.x.0 > row.origin.x.0 + 40.0, "{picker:?} at the caret of {row:?}");
    assert!(picker.origin.y.0 < scrim.size.height.0 / 3.0, "{picker:?} not centered");
    // a phone keeps the sheet at the bottom
    let mut phone = start("demo", &kept, Os::Ios, "Plain item 1");
    phone.press("text:Plain item 1").unwrap();
    phone.wait_for("id:row-input").unwrap();
    phone.press("label:Add date").unwrap();
    phone.wait_for("id:datepicker").unwrap();
    expect(&mut phone, "laid out", |s| bounds(s, "datepicker").is_some());
    let s = phone.screen();
    let (picker, scrim) = (bounds(s, "datepicker").unwrap(), bounds(s, "dp-scrim").unwrap());
    let bottom = picker.origin.y.0 + picker.size.height.0;
    assert!((bottom - scrim.size.height.0).abs() < 1.0, "{picker:?} at the bottom of {scrim:?}");
}

#[test]
fn control_settings_change_the_bullet_and_unindent() {
    let kept = storage(&[("use_bullet_to_zoom", "true"), ("unindent_in_place", "true")]);
    let mut app = start("looks", &kept, Os::MacOs, "Color labels");
    // unindent in place: the siblings after it become its children, so it stays above them (else it would go
    // after its old parent's last child)
    app.press("text:Label 3").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.press_key("shift+tab").unwrap();
    app.press_key("escape").unwrap();
    expect(&mut app, "unindented in place", |s| {
        let t = s.texts();
        let at = |x: &str| t.iter().position(|y| *y == x);
        matches!((at("Label 2"), at("Label 3"), at("Label 4")), (Some(a), Some(b), Some(c)) if a < b && b < c)
    });
    // the bullet zooms in instead of folding: the item becomes the title, its children the rows (the first one
    // edited); "Color labels" is the seventh row
    app.press("id:bullet >> nth=6").unwrap();
    expect(&mut app, "zoomed in", |s| s.shows("Label 2") && !s.shows("Heading one"));
}

#[test]
fn the_keymap_tab_records_keys_that_the_outline_uses() {
    let kept = storage(&[]);
    let mut app = start("demo", &kept, Os::MacOs, "Plain item 1");
    app.press("label:Settings").unwrap();
    app.press("label:Keymap").unwrap();
    app.wait_for("id:km-search").unwrap();
    app.type_text("id:km-search", "completed").unwrap();
    expect(&mut app, "found", |s| s.shows("Toggle completed") && !s.shows("Indent"));
    // + records the next key with a modifier
    app.press("label:Add a key to Toggle completed").unwrap();
    app.wait_for("text:Press keys…").unwrap();
    app.press_key("mod+shift+k").unwrap();
    let k = kept.clone();
    let waited = app.wait_until("kept", SHOWS, move |_| k.entries().contains_key("shortcut_override"));
    assert!(waited.is_ok());
    assert_eq!(entries(&kept)["shortcut_override"], r#"{"toggle-checked":["Mod+Enter","Mod+Shift+K"]}"#);
    // × takes the old key away
    app.press("label:Remove Mod+Enter from Toggle completed").unwrap();
    let k = kept.clone();
    let waited = app.wait_until("removed", SHOWS, move |_| {
        k.entries().get("shortcut_override").map(String::as_str) == Some(r#"{"toggle-checked":["Mod+Shift+K"]}"#)
    });
    assert!(waited.is_ok(), "{:?}", entries(&kept));
    app.press("label:Done").unwrap();
    // the new key checks the item being edited; the old one no longer does
    app.press("text:Plain item 1").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.press_key("mod+enter").unwrap();
    app.press_key("mod+shift+k").unwrap();
    app.press_key("escape").unwrap();
    let check = outliner_app::appearance::Looks::new(outliner_app::appearance::Modes::default(), Default::default())
        .desktop
        .color("color.check");
    let want = nsh_core::Color::from(nsh_core::prelude::rgba(check));
    expect(&mut app, "checked", move |s| s.span_style("text:Plain item 1 >> nth=0").is_ok_and(|st| st.color == want));
    // Reset all shortcuts: nothing kept
    app.press("label:Settings").unwrap();
    app.press("label:Keymap").unwrap();
    set(&mut app, &kept, "Reset all shortcuts", "shortcut_override", None);
}

#[test]
fn workflowy_keys_fold_and_move() {
    // Apple keyboards: Ctrl+Up folds, Alt+Shift+Up moves up (the web's WorkFlowy keys; its Alt+Left / Alt+Right
    // for zooming lose to Back and Forward, which come later in the keymap, on the web as here)
    let kept = storage(&[("workflowy_shortcuts", "true")]);
    let mut app = start("looks", &kept, Os::MacOs, "Color labels");
    app.press("text:Color labels").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.press_key("ctrl+up").unwrap();
    expect(&mut app, "folded", |s| !s.shows("Label 1"));
    app.press_key("alt+shift+up").unwrap();
    expect(&mut app, "moved up", |s| {
        let t = s.texts();
        let at = |x: &str| t.iter().position(|y| *y == x);
        at("A checkbox, on").is_some_and(|p| at("A checkbox, off").is_some_and(|q| q < p))
            && !t.contains(&"Color labels")
    });
    // Mod+Up is no longer the move key
    let kept = storage(&[]);
    let mut app = start("looks", &kept, Os::MacOs, "Color labels");
    app.press("text:Color labels").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.press_key("ctrl+up").unwrap();
    app.press_key("mod+.").unwrap();
    expect(&mut app, "folded by the default key", |s| !s.shows("Label 1"));
}

#[test]
fn backup_saves_a_zip_and_import_makes_documents() {
    let kept = storage(&[]);
    let mut app = start("looks", &kept, Os::MacOs, "Heading one");
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("prefs-backup");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("backup.zip");
    let _ = std::fs::remove_file(&path);
    app.on_pick_save_path(Some(path.clone())).unwrap();
    app.press("label:Settings").unwrap();
    app.press("label:Sync & data").unwrap();
    app.press("label:Download backup").unwrap();
    app.wait_for(&format!("text:Saved {}.", path.display())).unwrap();
    let zip = std::fs::read(&path).unwrap();
    assert_eq!(&zip[..4], b"PK\x03\x04");
    let names = String::from_utf8_lossy(&zip).into_owned();
    for name in ["Looks.txt", "Looks.opml", "Untitled.txt", "Untitled.opml"] {
        assert!(names.contains(name), "{name}");
    }
    assert!(names.contains("Looks\nHeading one\n"));
    // the sync server's form, with no server
    app.press("label:Connect").unwrap();
    app.wait_for("text:Sync is not in this app yet: documents stay on this device.").unwrap();
    // an OPML file and a text file: a document each, the last one opened
    let file = |name: &str, text: &str| {
        let data = nsh_core::image::ImageData::new(text.as_bytes().to_vec());
        nsh_core::picker::PickedFile::new(name, "text/plain", data)
    };
    let opml = r#"<?xml version="1.0"?><opml version="2.0"><head><title>Plan</title></head><body><outline text="Step one"/></body></opml>"#;
    app.on_pick(move |_| vec![file("plan.opml", opml), file("notes.txt", "First\n    Under it")]);
    app.press("label:Import OPML / text files…").unwrap();
    app.wait_for("text:Imported: plan.opml, notes.txt").unwrap();
    app.press("label:Done").unwrap();
    expect(&mut app, "the text file's document", |s| s.shows("Under it") && s.shows("Plan"));
}

#[test]
fn word_count_and_auto_focus() {
    let kept = storage(&[("word_count", "true"), ("auto_focus_first_item", "true")]);
    let mut app = start("looks", &kept, Os::MacOs, "Heading two");
    // the first item is edited from the start
    app.wait_for("id:row-input").unwrap();
    assert_eq!(app.screen().input_text("id:row-input").unwrap(), Some("Heading one"));
    // the words of the items and notes, and the items, under the zoom root: "Its note is checked too." is 5
    let shows = |want: &'static str| move |s: &Screen| s.texts().contains(&want);
    expect(&mut app, "the word count", shows("57 words · 52 in items · 5 in notes · 21 items"));
    app.type_text("id:row-input", " and more").unwrap();
    expect(&mut app, "two more words", shows("59 words · 54 in items · 5 in notes · 21 items"));
}
