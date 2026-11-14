//! The settings, headless: the theme follows the system's appearance until one is chosen, the gear (desktops)
//! and the menu (phones) open the settings, a choice applies at once and is kept on the device (a new start with
//! the same storage shows it), and the Look tab edits the user's appearance TOML: as text, checked with the error
//! at its line, and with a control per entry; exported and imported as a file.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use nsh_core::platform::{ColorScheme, Idiom, Os};
use nsh_core::prelude::rgba;
use nsh_core::Color;
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, MemoryStorage, Screen};
use outliner_app::appearance::{Looks, Modes, User};

/// How long a step's outcome may take to show (see `flows.rs`).
const SHOWS: Duration = Duration::from_secs(10);

/// The app on the small demo with the prefs `storage` holds, the system's appearance `scheme`.
fn start(os: Os, storage: &Arc<MemoryStorage>, scheme: ColorScheme) -> Headless {
    let root = outliner_app::fixture::root_with("demo", None, outliner_app::fixture::STORED, "");
    let mut config = EngineConfig::new(root);
    config.environment.os = os;
    config.environment.color_scheme = scheme;
    config.storage = Some(storage.clone());
    if os == Os::MacOs {
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: nsh_core::Points(1100.0), height: nsh_core::Points(800.0) };
    }
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:Plain item 1").unwrap();
    app
}

fn storage(entries: &[(&str, &str)]) -> Arc<MemoryStorage> {
    MemoryStorage::new(entries.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())))
}

/// The look of a desktop in `modes` (`group=mode` words) with a user's appearance.
fn look(modes: &str, user: &str) -> outliner_app::appearance::Look {
    let user = if user.is_empty() { User::default() } else { outliner_app::appearance::parse(user).unwrap() };
    Looks::new(Modes::parse(modes).unwrap(), user).desktop
}

fn hex(c: u32) -> Color {
    Color::from(rgba(c))
}

/// The text color and size of the first row.
fn first_row(s: &Screen) -> Option<(Color, f32)> {
    s.span_style("text:Plain item 1 >> nth=0").ok().map(|st| (st.color, st.font_size.0))
}

/// Waits until the first row's text has `color` (and `size`, when given).
fn expect_row(app: &mut Headless, what: &str, color: u32, size: Option<f32>) {
    let want = hex(color);
    let ok = |s: &Screen| first_row(s).is_some_and(|(c, z)| c == want && size.is_none_or(|w| (z - w).abs() < 0.01));
    let waited = app.wait_until(what, SHOWS, ok);
    if waited.is_err() {
        eprintln!("span: {:?}", app.screen().span_style("text:Plain item 1 >> nth=0"));
    }
    assert!(waited.is_ok(), "{what}: the first row is {:?}, not {want:?} {size:?}", first_row(app.screen()));
}

fn entries(storage: &MemoryStorage) -> BTreeMap<String, String> {
    storage.entries()
}

/// Presses the choice `label` and waits until the device keeps `key` as `value` (`None`: not kept, the default)
/// and the settings show it chosen (`selected`; `on` for a checkbox setting). A choice makes the screen again: the
/// next press waits for the new views instead of landing on the ones going.
fn choose(app: &mut Headless, kept: &Arc<MemoryStorage>, label: &str, key: &str, value: Option<&str>) {
    choose_on(app, kept, label, key, value, true);
}

fn choose_on(app: &mut Headless, kept: &Arc<MemoryStorage>, label: &str, key: &str, value: Option<&str>, on: bool) {
    let selector = format!("label:{label}");
    app.press(&selector).unwrap();
    let kept = kept.clone();
    let want = value.map(str::to_owned);
    let waited = app.wait_until(label, SHOWS, move |s| {
        kept.entries().get(key) == want.as_ref() && s.selected(&selector).is_ok_and(|x| x == on)
    });
    assert!(waited.is_ok(), "{label}: {key} is not kept as {value:?}, or not shown chosen");
}

#[test]
fn the_theme_follows_the_system_until_one_is_chosen() {
    let kept = storage(&[]);
    let mut app = start(Os::MacOs, &kept, ColorScheme::Dark);
    expect_row(&mut app, "dark, as the system", look("theme=dark", "").color("color.fg"), None);
    app.set_environment("light", |e| e.color_scheme = ColorScheme::Light).unwrap();
    expect_row(&mut app, "light, as the system", look("", "").color("color.fg"), None);
    // true black is the dark theme's: in the light appearance nothing changes, in the dark one it is black
    app.press("label:Settings").unwrap();
    choose(&mut app, &kept, "Use true black (dark theme)", "use_true_black", Some("true"));
    app.set_environment("dark", |e| e.color_scheme = ColorScheme::Dark).unwrap();
    expect_row(&mut app, "true black", look("theme=black", "").color("color.fg"), None);
    assert_eq!(entries(&kept).get("use_true_black").map(String::as_str), Some("true"));
    // a theme chosen stays whatever the system's appearance
    choose(&mut app, &kept, "Theme: Sepia", "theme", Some("sepia"));
    expect_row(&mut app, "sepia", look("theme=sepia", "").color("color.fg"), None);
    app.set_environment("light", |e| e.color_scheme = ColorScheme::Light).unwrap();
    expect_row(&mut app, "still sepia", look("theme=sepia", "").color("color.fg"), None);
}

#[test]
fn choices_apply_at_once_and_are_kept_for_the_next_start() {
    let kept = storage(&[]);
    let mut app = start(Os::MacOs, &kept, ColorScheme::Light);
    app.press("label:Settings").unwrap();
    app.wait_for("text:Preferences").unwrap();
    choose(&mut app, &kept, "Theme: Dark", "theme", Some("dark"));
    choose(&mut app, &kept, "Font size: Large", "font_size", Some("large"));
    choose(&mut app, &kept, "List density: Compact", "density", Some("compact"));
    choose(&mut app, &kept, "Font: Serif (Georgia)", "font", Some("serif"));
    choose(&mut app, &kept, "Eastern-Asian font: Batang", "eastern_asian_font", Some("batang"));
    let want = look("theme=dark size=large density=compact font=serif eafont=batang", "");
    expect_row(&mut app, "dark, large", want.color("color.fg"), Some(want.number("type.size")));
    let style = app.screen().span_style("text:Plain item 1 >> nth=0").unwrap();
    let family = style.font_family.map(|f| f.name().to_string()).unwrap_or_default();
    assert_eq!(family, want.text("font.editor"));
    let kept_now = entries(&kept);
    for (k, v) in [
        ("theme", "dark"),
        ("font_size", "large"),
        ("density", "compact"),
        ("font", "serif"),
        ("eastern_asian_font", "batang"),
    ] {
        assert_eq!(kept_now.get(k).map(String::as_str), Some(v), "{k}");
    }
    // Done closes the settings; a new start with the same storage has the same look
    app.press("label:Done").unwrap();
    app.wait_for_gone("id:settings", SHOWS).unwrap();
    drop(app);
    let mut again = start(Os::MacOs, &kept, ColorScheme::Light);
    expect_row(&mut again, "kept", want.color("color.fg"), Some(want.number("type.size")));
    // compact rows: the line and the density's padding
    let row = again
        .screen()
        .layout_nodes()
        .into_iter()
        .find(|n| n.test_id.as_deref() == Some("row") && n.bounds.size.height.0 > 0.0)
        .unwrap();
    let pitch = want.number("type.size") * want.number("type.line") + 2.0 * want.number("space.pad");
    assert!((row.bounds.size.height.0 - pitch).abs() < 1.0, "a row is {} tall, not {pitch}", row.bounds.size.height.0);
    // back to the default: nothing kept
    again.press("label:Settings").unwrap();
    choose(&mut again, &kept, "Theme: Auto-detect", "theme", None);
    // the large type makes the settings taller than the dialog: its body scrolls to the font options
    again.scroll("id:settings-body", nsh_core::prelude::px(400.0)).unwrap();
    choose(&mut again, &kept, "Font size: Medium", "font_size", None);
    expect_row(
        &mut again,
        "auto, medium",
        look("density=compact font=serif eafont=batang", "").color("color.fg"),
        Some(18.0),
    );
    assert!(!entries(&kept).contains_key("theme"));
    assert!(!entries(&kept).contains_key("font_size"));
}

#[test]
fn phones_open_the_settings_from_the_menu() {
    let kept = storage(&[("theme", "sakura")]);
    let mut app = start(Os::Ios, &kept, ColorScheme::Light);
    expect_row(&mut app, "sakura, as kept", look("theme=sakura", "").color("color.fg"), None);
    app.press("label:Menu").unwrap();
    app.wait_for("id:menu").unwrap();
    app.press("label:Settings").unwrap();
    app.wait_for("id:settings").unwrap();
    assert!(!app.screen().shows("id:menu"), "the menu closes");
    choose(&mut app, &kept, "Theme: Christmas", "theme", Some("christmas"));
    expect_row(&mut app, "christmas", look("theme=christmas", "").color("color.fg"), None);
    // a stored value that is no choice counts as none
    drop(app);
    let odd = storage(&[("theme", "nope"), ("font_size", "huge"), ("appearance", "[color]\nbg = 1\n")]);
    let mut app = start(Os::Ios, &odd, ColorScheme::Light);
    expect_row(&mut app, "the default", look("", "").color("color.fg"), Some(18.0));
}

#[test]
fn the_look_tab_applies_valid_toml_and_shows_an_error_at_its_line() {
    let kept = storage(&[]);
    let mut app = start(Os::MacOs, &kept, ColorScheme::Light);
    let light = look("", "").color("color.fg");
    app.press("label:Settings").unwrap();
    app.press("label:Look").unwrap();
    app.wait_for("id:toml").unwrap();
    // an error at its line, checked as it is typed; Apply keeps the look as it was
    app.type_text("id:toml", "[color]\nfg = \"#102030\"\ntag = \"teal\"\n").unwrap();
    app.wait_for("text:Line 3: color.tag: not a color: teal").unwrap();
    app.press("label:Apply").unwrap();
    expect_row(&mut app, "unchanged", light, None);
    assert!(!entries(&kept).contains_key("appearance"));
    // fixed: applied and kept
    app.clear("id:toml").unwrap();
    let toml = "[color]\nfg = \"#102030\"\n\n[type]\nsize = 17\n";
    app.type_text("id:toml", toml).unwrap();
    let gone = app.wait_until("no error", SHOWS, |s| !s.shows("Line 3: color.tag: not a color: teal"));
    assert!(gone.is_ok());
    app.press("label:Apply").unwrap();
    expect_row(&mut app, "the user's", 0x102030ff, Some(17.0));
    assert_eq!(entries(&kept).get("appearance").map(String::as_str), Some(toml));
    drop(app);
    let mut again = start(Os::MacOs, &kept, ColorScheme::Light);
    expect_row(&mut again, "kept", 0x102030ff, Some(17.0));
}

#[test]
fn an_entry_control_sets_one_value_and_resets_it() {
    let kept = storage(&[]);
    let mut app = start(Os::MacOs, &kept, ColorScheme::Light);
    app.press("label:Settings").unwrap();
    app.press("label:Look").unwrap();
    app.press("label:[type]").unwrap();
    app.wait_for("id:entry-type.size").unwrap();
    app.type_text("id:entry-type.size", "17").unwrap();
    app.submit("id:entry-type.size").unwrap();
    expect_row(&mut app, "17 from its control", look("", "").color("color.fg"), Some(17.0));
    let shown =
        app.wait_until("the TOML", SHOWS, |s| s.input_text("id:toml").ok().flatten() == Some("[type]\nsize = 17\n"));
    assert!(shown.is_ok(), "the TOML is {:?}", app.screen().input_text("id:toml"));
    // a value out of range is refused with its entry's name, the look as it was
    app.clear("id:entry-type.size").unwrap();
    app.type_text("id:entry-type.size", "100").unwrap();
    app.submit("id:entry-type.size").unwrap();
    app.wait_for("text:type.size: more than 40").unwrap();
    expect_row(&mut app, "still 17", look("", "").color("color.fg"), Some(17.0));
    // × goes back to the theme's value
    app.press("label:Reset type.size").unwrap();
    expect_row(&mut app, "18 again", look("", "").color("color.fg"), Some(18.0));
    assert!(!entries(&kept).contains_key("appearance"));
}

#[test]
fn the_appearance_exports_and_imports_as_a_toml_file() {
    let toml = "[color]\nfg = \"#102030\"\n";
    let kept = storage(&[("appearance", toml)]);
    let mut app = start(Os::MacOs, &kept, ColorScheme::Light);
    expect_row(&mut app, "kept", 0x102030ff, None);
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("settings-export");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("appearance.toml");
    let _ = std::fs::remove_file(&path);
    app.on_pick_save_path(Some(path.clone())).unwrap();
    app.press("label:Settings").unwrap();
    app.press("label:Look").unwrap();
    app.press("label:Export .toml").unwrap();
    app.wait_for(&format!("text:Exported {}.", path.display())).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), toml);
    // an imported file is applied and kept; an invalid one shows its error and leaves the look
    let file = |name: &str, text: &str| {
        let data = nsh_core::image::ImageData::new(text.as_bytes().to_vec());
        nsh_core::picker::PickedFile::new(name, "application/toml", data)
    };
    let import = "[color]\nfg = \"#303030\"\n";
    app.on_pick(move |_| vec![file("mine.toml", import)]);
    app.press("label:Import .toml").unwrap();
    app.wait_for("text:Imported mine.toml.").unwrap();
    expect_row(&mut app, "imported", 0x303030ff, None);
    assert_eq!(entries(&kept).get("appearance").map(String::as_str), Some(import));
    app.on_pick(move |_| vec![file("bad.toml", "[type]\n\nsize = 100\n")]);
    app.press("label:Import .toml").unwrap();
    app.wait_for("text:bad.toml is not a valid appearance.").unwrap();
    assert!(app.screen().shows("Line 3: type.size: more than 40"));
    expect_row(&mut app, "as it was", 0x303030ff, None);
}
