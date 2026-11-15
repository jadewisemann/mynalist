//! The settings, as the web app's (`apps/web/src/ui/settings.ts` and `ui/appearance.ts`): the look (the theme,
//! following the system's light or dark appearance by default, the text size, the list density, the fonts, the
//! user's appearance TOML of `design/schema.md`) and the web's other prefs the native app honors ([`FLAGS`],
//! [`PICKS`], [`TEXTS`], the keymap's changes). The choices are kept on the device (NSH storage, under the web's
//! pref names) and applied at once: the look is resolved again (`appearance::set`) and the screen made again from
//! it; the other prefs make the outline and the keys again (`lib.rs` `behaving`).
//!
//! The settings screen is the web's settings dialog: Preferences (the web's sections, with what the native app
//! honors), Look (the appearance: its TOML text, checked with the error at its line, one control per schema entry,
//! export and import as a .toml file), Keymap (the commands' keys, recorded and reset) and Sync & data (the sync
//! server's form as the web shows it without one, backup, import). Desktops open it with the top bar's gear,
//! phones from the top bar's menu.

use std::collections::BTreeMap;

use nsh_core::file_dialog::{FileFilter, SaveDialog};
use nsh_core::picker::FilePickRequest;
use nsh_core::prelude::*;
use nsh_core::storage::Storage;
use nsh_core::LineHeight;

use crate::appearance::{self, Modes};
use crate::design::{Kind, SCHEMA};
use crate::sidebar::ui_text;
use crate::store;
use crate::style::{self, color, num};

/// What the user chose (the web's prefs of the same names).
#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// `auto` (the system's light or dark) or a theme of `design/modes.toml`.
    pub theme: String,
    /// The dark theme in true black (`use_true_black`: the `black` theme).
    pub true_black: bool,
    pub size: String,
    pub density: String,
    pub font: String,
    pub eafont: String,
    /// The user's appearance TOML; only a valid one is kept.
    pub appearance: String,
    /// The other prefs ([`FLAGS`], [`PICKS`], [`TEXTS`], [`SHORTCUTS`]) that differ from their defaults, as kept
    /// on the device: `true`/`false`, a choice's value, a text, the shortcuts' JSON.
    pub other: BTreeMap<&'static str, String>,
}

impl Default for Prefs {
    fn default() -> Self {
        let m = Modes::default();
        Prefs {
            theme: AUTO.into(),
            true_black: false,
            size: m.size,
            density: m.density,
            font: m.font,
            eafont: m.eafont,
            appearance: String::new(),
            other: BTreeMap::new(),
        }
    }
}

/// A setting that is on or off (the web's checkbox prefs): its key, label, default and hint.
pub struct Flag {
    pub key: &'static str,
    pub label: &'static str,
    pub default: bool,
    pub hint: &'static str,
}

/// A setting with a few choices (the web's `<select>` prefs).
pub struct Pick {
    pub key: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    pub options: &'static [(&'static str, &'static str)],
}

/// A setting typed as text (the date display formats).
pub struct Typed {
    pub key: &'static str,
    pub label: &'static str,
    pub placeholder: &'static str,
    pub hint: &'static str,
}

const fn flag(key: &'static str, label: &'static str, default: bool) -> Flag {
    Flag { key, label, default, hint: "" }
}

/// The on/off settings the native app honors, with the web's names and defaults (`ui/settings.ts`).
pub const FLAGS: &[Flag] = &[
    flag("current_highlight", "Highlight current item", false),
    flag("bullet_threading", "Bullet threading", true),
    flag("center_doc", "Narrow document layout", true),
    flag("display_images_inline", "Display images inline", false),
    flag("show_checked", "Show checked items", true),
    flag("word_count", "Show word count", false),
    flag("workflowy_shortcuts", "WorkFlowy-style shortcuts", false),
    flag("show_timezone", "Encode timezone in new dates", false),
    flag("highlight_overdue", "Highlight overdue dates", false),
    flag("use_bullet_to_zoom", "Use bullet point to zoom in", false),
    Flag {
        key: "unindent_in_place",
        label: "Unindent items in-place",
        default: false,
        hint: "Following siblings become children of the unindented item.",
    },
    flag("auto_focus_first_item", "Auto-focus on first item", false),
    flag("global_tag_autocomplete", "Auto-complete tags from all docs", false),
    flag("show_backlinks", "Show backlinks", false),
    flag("show_search_tips", "Show search tip popup", true),
    flag("export_visible_only", "Export visible items only (default)", false),
    flag("item_toolbar_hover", "Show item toolbar on hover (desktop)", false),
];

/// The settings with choices the native app honors.
pub const PICKS: &[Pick] = &[
    Pick { key: "show_notes", label: "Notes", default: "show", options: &[("show", "Show"), ("teaser", "1st line"), ("hide", "Hide")] },
    Pick { key: "time_format", label: "Time format", default: "12h", options: &[("12h", "12-hour"), ("24h", "24-hour")] },
    Pick {
        key: "start_day",
        label: "First day of week",
        default: "sun",
        options: &[("sat", "Saturday"), ("sun", "Sunday"), ("mon", "Monday")],
    },
    Pick {
        key: "recurrence_check_behavior",
        label: "Recurring date",
        default: "new_instance",
        options: &[("new_instance", "Next date appears in new item"), ("update", "Existing date gets updated")],
    },
    Pick {
        key: "inbox_move_position",
        label: "Move to position",
        default: "bottom",
        options: &[("bottom", "End of inbox"), ("top", "Start of inbox")],
    },
    Pick {
        key: "move_item_sent_to_position",
        label: "Move item to",
        default: "bottom",
        options: &[("bottom", "End of destination"), ("top", "Start of destination")],
    },
    Pick {
        key: "dyslexia_font",
        label: "Font for dyslexia",
        default: "none",
        options: &[("none", "None"), ("opendyslexic", "OpenDyslexic (if installed)")],
    },
];

/// The typed settings the native app honors.
pub const TEXTS: &[Typed] = &[
    Typed {
        key: "custom_date_display_format",
        label: "Date & time display format",
        placeholder: "relative",
        hint: "Empty or \"relative\" for \"Tomorrow, 3 pm\"; or a pattern like \"YYYY-MM-DD HH:mm\".",
    },
    Typed {
        key: "custom_date_display_format_no_time",
        label: "Date display format (no time)",
        placeholder: "relative",
        hint: "e.g. \"ddd, MMM D\"",
    },
    // chosen with the inbox picker ("Your inbox"), not typed: "<doc>" or "<doc>/<item>"
    Typed { key: "inbox_location", label: "Inbox destination", placeholder: "", hint: "" },
];

/// The keymap's changes (the web's `shortcut_override`: a command's id and its keys, as JSON).
pub const SHORTCUTS: &str = "shortcut_override";

impl Prefs {
    /// Whether on/off setting `key` ([`FLAGS`]) is on.
    pub fn flag(&self, key: &str) -> bool {
        let def = FLAGS.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("no setting {key}"));
        self.other.get(key).map_or(def.default, |v| v == "true")
    }

    /// The value chosen for setting `key` ([`PICKS`]).
    pub fn pick(&self, key: &str) -> &str {
        let def = PICKS.iter().find(|p| p.key == key).unwrap_or_else(|| panic!("no setting {key}"));
        self.other.get(key).map_or(def.default, String::as_str)
    }

    /// The text typed for setting `key` ([`TEXTS`]), "" when none.
    pub fn text(&self, key: &str) -> &str {
        self.other.get(key).map_or("", String::as_str)
    }

    /// The keymap's changes: each command's keys where they differ from its defaults.
    pub fn shortcuts(&self) -> BTreeMap<String, Vec<String>> {
        self.other.get(SHORTCUTS).and_then(|j| serde_json::from_str(j).ok()).unwrap_or_default()
    }

    /// Sets the keymap's changes (none: the defaults).
    pub fn set_shortcuts(&mut self, o: &BTreeMap<String, Vec<String>>) {
        if o.is_empty() {
            self.other.remove(SHORTCUTS);
        } else {
            self.other.insert(SHORTCUTS, serde_json::to_string(o).expect("a map of strings"));
        }
    }

    /// Sets an on/off setting.
    pub fn set_flag(&mut self, key: &str, on: bool) {
        let f = FLAGS.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("no setting {key}"));
        if on == f.default {
            self.other.remove(f.key);
        } else {
            self.other.insert(f.key, on.to_string());
        }
    }

    /// Chooses a setting's value.
    pub fn set_pick(&mut self, key: &str, value: &str) {
        let p = PICKS.iter().find(|p| p.key == key).unwrap_or_else(|| panic!("no setting {key}"));
        if value == p.default {
            self.other.remove(p.key);
        } else {
            self.other.insert(p.key, value.to_owned());
        }
    }

    /// Types a setting's text (trimmed; empty: none).
    pub fn set_text(&mut self, key: &str, value: &str) {
        let t = TEXTS.iter().find(|t| t.key == key).unwrap_or_else(|| panic!("no setting {key}"));
        let v = value.trim();
        if v.is_empty() {
            self.other.remove(t.key);
        } else {
            self.other.insert(t.key, v.to_owned());
        }
    }

    /// The date labels' options: the clock, the display formats (`renderOpts` on the web).
    pub fn date_format(&self, now: i64) -> outliner_core::dates::DateFormatOptions {
        let mut o = outliner_core::dates::DateFormatOptions::new(now);
        o.hour12 = self.pick("time_format") == "12h";
        let fmt = |k| Some(self.text(k).to_owned()).filter(|t| !t.is_empty());
        o.format = fmt("custom_date_display_format");
        o.format_no_time = fmt("custom_date_display_format_no_time");
        o
    }

    /// The first day of the week, 0 = Sunday (`start_day`).
    pub fn start_day(&self) -> i64 {
        match self.pick("start_day") {
            "sat" => 6,
            "mon" => 1,
            _ => 0,
        }
    }
}

/// The tutorial was done or skipped (`finished_onboarding`), kept on the device.
pub fn finish_onboarding() {
    if let Err(e) = Storage::set_item(crate::help::FINISHED, "true") {
        eprintln!("settings: {} not kept: {e:?}", crate::help::FINISHED);
    }
}

/// The extra prefs kept on the device: those with a valid value, by their key.
fn stored_other() -> BTreeMap<&'static str, String> {
    let mut out = BTreeMap::new();
    for f in FLAGS {
        if let Some(v) = Storage::get_item(f.key).filter(|v| (v == "true" || v == "false") && *v != f.default.to_string()) {
            out.insert(f.key, v);
        }
    }
    for p in PICKS {
        if let Some(v) = Storage::get_item(p.key).filter(|v| v != p.default && p.options.iter().any(|(o, _)| o == v)) {
            out.insert(p.key, v);
        }
    }
    for t in TEXTS {
        if let Some(v) = Storage::get_item(t.key).filter(|v| !v.trim().is_empty()) {
            out.insert(t.key, v);
        }
    }
    let valid = |j: &String| serde_json::from_str::<BTreeMap<String, Vec<String>>>(j).is_ok_and(|m| !m.is_empty());
    if let Some(v) = Storage::get_item(SHORTCUTS).filter(valid) {
        out.insert(SHORTCUTS, v);
    }
    out
}

const AUTO: &str = "auto";

/// The storage keys: the web's pref names.
const THEME: &str = "theme";
const TRUE_BLACK: &str = "use_true_black";
const SIZE: &str = "font_size";
const DENSITY: &str = "density";
const FONT: &str = "font";
const EAFONT: &str = "eastern_asian_font";
const APPEARANCE: &str = "appearance";

/// The choices of each setting, as the web's settings list them: the value and its label.
const THEMES: &[(&str, &str)] = &[
    (AUTO, "Auto-detect"),
    ("light", "Light"),
    ("dark", "Dark"),
    ("sepia", "Sepia"),
    ("scifi", "Sci-Fi"),
    ("sakura", "Sakura"),
    ("darkgold", "Dark gold"),
    ("christmas", "Christmas"),
];
const SIZES: &[(&str, &str)] = &[("small", "Small"), ("medium", "Medium"), ("large", "Large")];
const DENSITIES: &[(&str, &str)] = &[("comfortable", "Comfortable"), ("cozy", "Cozy"), ("compact", "Compact")];
const FONTS: &[(&str, &str)] = &[
    ("default", "Fira Sans (default)"),
    ("system", "System font"),
    ("inter", "Inter (if installed)"),
    ("arial", "Arial"),
    ("helvetica", "Helvetica"),
    ("roboto", "Roboto (if installed)"),
    ("serif", "Serif (Georgia)"),
    ("mono", "Monospace"),
];
const EA_FONTS: &[(&str, &str)] = &[
    ("default", "Default"),
    ("pretendard", "Pretendard"),
    ("notosanskr", "Noto Sans KR"),
    ("malgun", "Malgun Gothic"),
    ("batang", "Batang"),
    ("yahei", "Microsoft YaHei"),
    ("kaiti", "KaiTi"),
    ("meiryo", "Meiryo"),
    ("hiragino", "Hiragino Kaku Gothic"),
];

impl Prefs {
    /// The prefs kept on the device; a value that is not one of its setting's choices (or an invalid appearance)
    /// counts as not set.
    pub fn stored() -> Prefs {
        let d = Prefs::default();
        let pick = |key: &str, choices: &[(&str, &str)], default: String| {
            Storage::get_item(key).filter(|v| choices.iter().any(|(c, _)| c == v)).unwrap_or(default)
        };
        let appearance = Storage::get_item(APPEARANCE).filter(|t| appearance::parse(t).is_ok()).unwrap_or_default();
        Prefs {
            theme: pick(THEME, THEMES, d.theme),
            true_black: Storage::get_item(TRUE_BLACK).as_deref() == Some("true"),
            size: pick(SIZE, SIZES, d.size),
            density: pick(DENSITY, DENSITIES, d.density),
            font: pick(FONT, FONTS, d.font),
            eafont: pick(EAFONT, EA_FONTS, d.eafont),
            appearance,
            other: stored_other(),
        }
    }

    /// Keeps on the device what changed since `before`: a default is not kept. A setting that did not change is
    /// left as it is there (a value this app does not read, or does not know, stays).
    fn store(&self, before: &Prefs) {
        let d = Prefs::default();
        let keep = |key: &str, v: &str, old: &str, default: &str| {
            if v == old {
                return;
            }
            if v == default {
                Storage::remove_item(key);
            } else if let Err(e) = Storage::set_item(key, v) {
                eprintln!("settings: {key} not kept: {e:?}");
            }
        };
        let tf = |b: bool| if b { "true" } else { "false" };
        keep(THEME, &self.theme, &before.theme, &d.theme);
        keep(TRUE_BLACK, tf(self.true_black), tf(before.true_black), "false");
        keep(SIZE, &self.size, &before.size, &d.size);
        keep(DENSITY, &self.density, &before.density, &d.density);
        keep(FONT, &self.font, &before.font, &d.font);
        keep(EAFONT, &self.eafont, &before.eafont, &d.eafont);
        keep(APPEARANCE, &self.appearance, &before.appearance, "");
        let keys = FLAGS.iter().map(|f| f.key).chain(PICKS.iter().map(|p| p.key)).chain(TEXTS.iter().map(|t| t.key));
        for key in keys.chain([SHORTCUTS]) {
            let (v, old) = (self.other.get(key), before.other.get(key));
            if v == old {
                continue;
            }
            match v {
                None => Storage::remove_item(key),
                Some(v) => {
                    if let Err(e) = Storage::set_item(key, v) {
                        eprintln!("settings: {key} not kept: {e:?}");
                    }
                }
            }
        }
    }

    /// The modes these prefs choose while the system's appearance is dark or not (the web's `applyPrefs`).
    pub fn modes(&self, dark: bool) -> Modes {
        let mut theme = if self.theme == AUTO {
            if dark {
                "dark"
            } else {
                "light"
            }
        } else {
            self.theme.as_str()
        };
        if theme == "dark" && self.true_black {
            theme = "black";
        }
        Modes {
            theme: theme.into(),
            size: self.size.clone(),
            density: self.density.clone(),
            // the font for dyslexia over the font chosen (the web's `data-font`)
            font: if self.pick("dyslexia_font") == "opendyslexic" { "dyslexic".into() } else { self.font.clone() },
            eafont: self.eafont.clone(),
        }
    }

    /// The prefs that choose `modes` (a fixture's), with an appearance.
    pub fn of(modes: &Modes, appearance: &str) -> Prefs {
        Prefs {
            theme: modes.theme.clone(),
            true_black: false,
            size: modes.size.clone(),
            density: modes.density.clone(),
            font: modes.font.clone(),
            eafont: modes.eafont.clone(),
            appearance: appearance.to_owned(),
            other: BTreeMap::new(),
        }
    }
}

/// The prefs in use, once read from the device.
#[derive(Clone, Default, PartialEq)]
struct Current(Option<Prefs>);

fn current() -> Signal<Current> {
    global::<Current>()
}

/// The prefs in use (tracked); read from the device the first time.
pub fn prefs() -> Prefs {
    if current().with_untracked(|c| c.0.is_none()) {
        current().set(Current(Some(Prefs::stored())));
    }
    current().with(|c| c.0.clone().unwrap_or_default())
}

/// Uses `prefs` without keeping them (a look check's fixture).
pub fn use_prefs(prefs: Prefs) {
    current().set(Current(Some(prefs)));
}

/// Changes the prefs, keeps them on the device and applies them.
pub fn change(f: impl FnOnce(&mut Prefs)) {
    let before = untracked(prefs);
    let mut p = before.clone();
    f(&mut p);
    p.store(&before);
    current().set(Current(Some(p)));
}

/// The prefs in use, untracked (handlers, and views made again when they change).
pub fn get() -> Prefs {
    untracked(prefs)
}

/// The prefs that are not the look's (tracked, changing only with them): the outline and its keys are made again
/// when they change.
pub fn behavior() -> BTreeMap<&'static str, String> {
    prefs().other
}

/// Applies the prefs and the system's appearance to the look (an effect of the app's root: it runs again when
/// either changes), and the checked items' visibility to the documents.
pub fn apply() {
    let p = prefs();
    let modes = p.modes(dark());
    if appearance::set(modes.clone(), &p.appearance).is_err() {
        // kept only once valid: this is a stored value from elsewhere, so the look goes without it
        let _ = appearance::set(modes, "");
    }
    let shown = p.flag("show_checked");
    if store::peek(|s| s.show_checked != shown) {
        store::edit(|s| {
            s.show_checked = shown;
            s.reflow();
        });
    }
}

/// A document was opened (the app started, another chosen): with "Auto-focus on first item" on, its first item is
/// edited (the web's `goto-first` on a new route).
pub fn document_opened() {
    if !get().flag("auto_focus_first_item") {
        return;
    }
    if let Some(first) = store::peek(|s| s.rows.first().map(|r| r.id.clone())) {
        crate::commands::start_editing(&first, Some(0));
    }
}

/// The words of a text as the web's status bar counts them: a letter or digit, then letters, digits, `'`, `’` and
/// `-`.
fn words(src: &str) -> usize {
    let plain = outliner_core::inline::inline_to_plain(src, None);
    let mut n = 0;
    let mut inside = false;
    for c in plain.chars() {
        if c.is_alphanumeric() {
            if !inside {
                n += 1;
            }
            inside = true;
        } else if !(inside && matches!(c, '\'' | '’' | '-')) {
            inside = false;
        }
    }
    n
}

/// The status bar's text (`paintStatus`): the words in the items and notes under the zoom root, and the items.
pub fn word_count(s: &store::State) -> String {
    let t = s.tree();
    let (mut items, mut in_items, mut in_notes) = (0, 0, 0);
    let mut stack = vec![s.view.zoom.clone()];
    while let Some(id) = stack.pop() {
        let Some(n) = t.get(&id) else { continue };
        for c in n.children.iter().rev() {
            let k = t.get(c);
            items += 1;
            in_items += words(outliner_core::item::content(k));
            in_notes += words(outliner_core::item::note(k));
            stack.push(c.clone());
        }
    }
    format!("{} words · {in_items} in items · {in_notes} in notes · {items} items", in_items + in_notes)
}

/// The word count at the bottom right (`.status-bar`), while "Show word count" is on.
pub fn status_bar() -> impl IntoElement {
    if !get().flag("word_count") {
        return view().into_element();
    }
    view()
        .absolute()
        .right(px(num("space.step8")))
        .bottom(px(num("space.step6")))
        .child(
            view()
                .test_id("status-bar")
                .bg(color::bg())
                .border(px(num("space.border")))
                .border_color(color::line())
                .rounded(px(num("radius.block")))
                .px(px(num("space.step6")))
                .py(px(num("space.step2")))
                .child(hint(|| store::read(word_count))),
        )
        .into_element()
}

/// Which tab the settings show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Prefs,
    Look,
    Keymap,
    Data,
}

/// The settings screen's state: kept apart from the views, which are made again when the look changes.
#[derive(Clone, Debug, Default, PartialEq)]
struct Panel {
    open: bool,
    tab: Tab,
    /// The phone's top bar menu shows.
    menu: bool,
    /// The appearance TOML being edited, while it differs from the one in use.
    draft: Option<String>,
    /// What is wrong with the draft (or with a control's value).
    error: Option<appearance::Error>,
    /// The tables of the appearance's controls shown (`[color]`, `[type]`, ...).
    tables: Vec<&'static str>,
    /// What the last export or import did.
    status: String,
    /// The Preferences tab shows its advanced settings.
    advanced: bool,
    /// The Keymap tab's search, and the command whose new key is being recorded.
    filter: String,
    recording: Option<&'static str>,
    /// What the last backup or import did.
    data_status: String,
}

fn panel() -> Signal<Panel> {
    global::<Panel>()
}

/// Shows the settings on `tab` (the gear, the phone menu, a fixture).
pub fn open(tab: Tab) {
    // the row being edited lets go: its keys would act while a setting is typed
    crate::store::edit(|s| s.editing = None);
    panel().update(|p| {
        p.open = true;
        p.menu = false;
        p.tab = tab;
    });
}

/// Shows the controls of `table` (a fixture's Look tab).
pub fn show_table(table: &'static str) {
    panel().update(|p| {
        if !p.tables.contains(&table) {
            p.tables.push(table);
        }
    });
}

fn close() {
    panel().update(|p| {
        p.open = false;
        p.recording = None;
        p.data_status.clear();
        p.draft = None;
        p.error = None;
        p.status.clear();
    });
}

/// Opens or closes the top bar's menu.
pub fn toggle_menu() {
    panel().update(|p| p.menu = !p.menu);
}

/// The appearance TOML the Look tab shows: the draft, or the one in use.
fn draft() -> String {
    panel().with(|p| p.draft.clone()).unwrap_or_else(|| prefs().appearance)
}

/// Uses appearance `text` when it is valid (kept and applied); else the error shows and the look stays.
fn apply_toml(text: String) -> bool {
    match appearance::parse(&text) {
        Ok(_) => {
            change(|p| p.appearance = text);
            panel().update(|p| {
                p.draft = None;
                p.error = None;
            });
            true
        }
        Err(e) => {
            panel().update(|p| {
                p.draft = Some(text);
                p.error = Some(e);
            });
            false
        }
    }
}

/// Sets one entry of the appearance from its control (`None`: back to the theme's value).
fn set_entry(id: &'static str, input: Option<&str>) {
    let base = untracked(prefs).appearance;
    let value = input.and_then(|t| appearance::control_value(id, t));
    match appearance::with_entry(&base, id, value) {
        Ok(text) => {
            apply_toml(text);
        }
        Err(e) => panel().update(|p| p.error = Some(e)),
    }
}

fn export() {
    let text = untracked(draft);
    if style::phone() {
        // phones have no save dialog: the file's text goes to the clipboard, to paste where it is kept
        set_clipboard(text);
        panel().update(|p| p.status = "Copied appearance.toml to the clipboard.".into());
        return;
    }
    spawn(async move {
        let dialog = SaveDialog::new().file_name("appearance.toml").filter(FileFilter::new("TOML", ["toml"]));
        let status = match pick_save_path(dialog).await {
            Ok(Some(path)) => match std::fs::write(&path, &text) {
                Ok(()) => format!("Exported {}.", path.display()),
                Err(e) => format!("Could not write {}: {e}", path.display()),
            },
            Ok(None) => return,
            Err(e) => format!("Could not export: {e}"),
        };
        panel().update(|p| p.status = status);
    });
}

fn import() {
    spawn(async move {
        let mut request = FilePickRequest::new(false, 1);
        request.filters = vec![FileFilter::new("TOML", ["toml"])];
        let Some(file) = pick_files(request).await.into_iter().next() else {
            return;
        };
        let status = match String::from_utf8(file.data.bytes().to_vec()) {
            Ok(text) if appearance::parse(&text).is_ok() => {
                apply_toml(text);
                format!("Imported {}.", file.name)
            }
            Ok(text) => {
                // the file's text in the editor, its error at its line
                apply_toml(text);
                format!("{} is not a valid appearance.", file.name)
            }
            Err(_) => format!("{} is not a text file.", file.name),
        };
        panel().update(|p| p.status = status);
    });
}

// ---- views: made again whenever the look changes (the app's root), so they read the look as plain values

fn ui(t: impl IntoReactive<SharedString>) -> Text {
    ui_text(t, style::ui()).text_color(color::fg())
}

fn small(t: impl IntoReactive<SharedString>) -> Text {
    ui_text(t, style::ui_sized("type.ui_small"))
}

/// A hint under a setting (`.hint`).
fn hint(t: impl IntoReactive<SharedString>) -> Text {
    ui_text(t, style::ui_sized("type.ui_tiny")).text_color(color::muted())
}

/// A button (`.btn`; `.btn.primary`).
fn btn(label: &'static str, primary: bool, run: impl Fn() + 'static) -> impl IntoElement {
    let (bg, fg, border) = if primary {
        (color::accent(), rgba(style::look().color("color.on_accent")), color::accent())
    } else {
        (color::bg(), color::fg(), color::chip_border())
    };
    nsh_base::button()
        .accessibility_label(Some(label.into()))
        .px(px(num("space.btn_x")))
        .py(px(num("space.btn_y")))
        .bg(bg)
        .border(px(num("space.border")))
        .border_color(border)
        .rounded(px(num("radius.block")))
        .items_center()
        .on_press(run)
        .child(ui(label).text_color(fg))
}

/// A choice of a setting: one button per value, the chosen one marked (the web's `<select>`, its options in view).
fn choice(
    name: &'static str,
    options: &'static [(&'static str, &'static str)],
    chosen: String,
    pick: impl Fn(&mut Prefs, &str) + Clone + 'static,
) -> impl IntoElement {
    let row = view().flex_row().flex_wrap().gap(px(num("space.set_tabs_gap")));
    row.children(options.iter().map(|(value, label)| {
        let on = chosen == *value;
        let pick = pick.clone();
        nsh_base::button()
            .accessibility_label(Some(format!("{name}: {label}").into()))
            .selected(on)
            .px(px(num("space.input_x")))
            .py(px(num("space.input_y")))
            .bg(if on { color::sel() } else { color::bg() })
            .border(px(num("space.border")))
            .border_color(if on { color::accent() } else { color::chip_border() })
            .rounded(px(num("radius.block")))
            .on_press(move || change(|p| pick(p, value)))
            .child(ui(*label))
            .into_element()
    }))
}

/// A setting: its label over its input (`.field`).
fn field(label: &'static str, input: impl IntoElement) -> impl IntoElement {
    view().flex_col().gap(px(num("space.field_gap"))).mb(px(num("space.field_bottom"))).child(ui(label)).child(input)
}

/// A checkbox setting (`.field.row`).
fn check_field(label: &'static str, on: bool, flip: impl Fn(&mut Prefs) + 'static) -> impl IntoElement {
    let size = num("space.check");
    let mut boxed = view()
        .w(px(size))
        .h(px(size))
        .rounded(px(num("radius.check")))
        .border(px(num("space.check_border")))
        .border_color(if on { color::accent() } else { color::check() })
        .items_center()
        .justify_center();
    if on {
        boxed = boxed.bg(color::accent());
    }
    nsh_base::button()
        .accessibility_label(Some(label.into()))
        .selected(on)
        .flex_row()
        .items_center()
        .gap(px(num("space.field_row_gap")))
        .mb(px(num("space.field_bottom")))
        .on_press(move || change(&flip))
        .child(boxed)
        .child(ui(label))
}

/// A section's head (`.set-sec h3`).
fn section(title: &'static str) -> impl IntoElement {
    let (size, _) = style::ui_sized("type.ui_small");
    small(title)
        .text_color(color::muted())
        .uppercase()
        .letter_spacing(px(size * num("type.head_tracking")))
        .mt(px(num("space.set_head_top")))
        .mb(px(num("space.set_head_bottom")))
}

/// An on/off setting of [`FLAGS`], with its hint.
fn flag_field(key: &'static str) -> impl IntoElement {
    let f = FLAGS.iter().find(|f| f.key == key).expect("a setting of FLAGS");
    let on = get().flag(key);
    let row = check_field(f.label, on, move |p| p.set_flag(key, !on));
    if f.hint.is_empty() {
        return row.into_element();
    }
    // the hint under its checkbox, in the row's bottom margin
    view().child(row).child(hint(f.hint).mt(px(-num("space.field_bottom"))).mb(px(num("space.field_bottom")))).into_element()
}

/// A setting of [`PICKS`]: its choices as buttons.
fn pick_field(key: &'static str) -> impl IntoElement {
    let p = PICKS.iter().find(|p| p.key == key).expect("a setting of PICKS");
    let chosen = get().pick(key).to_owned();
    field(p.label, choice(p.label, p.options, chosen, move |prefs, v| prefs.set_pick(key, v)))
}

/// A setting of [`TEXTS`]: typed, kept on Return or when the input lets go.
fn text_setting(key: &'static str) -> impl IntoElement {
    let t = TEXTS.iter().find(|t| t.key == key).expect("a setting of TEXTS");
    let typed = signal(get().text(key).to_owned());
    let commit = move || {
        let v = typed.get_untracked();
        if v.trim() != get().text(key) {
            change(|p| p.set_text(key, &v));
        }
    };
    let input = text_field(&format!("pref-{key}"), typed.get_untracked(), t.placeholder.into())
        .on_change(move |v: &str| typed.set(v.to_owned()))
        .on_submit(move |_: &str| commit())
        .on_blur(commit);
    view()
        .flex_col()
        .gap(px(num("space.field_gap")))
        .mb(px(num("space.field_bottom")))
        .child(ui(t.label))
        .child(input)
        .child(hint(t.hint))
}

/// The Preferences tab: the web's settings that the native app honors, in the web's sections (the advanced ones
/// behind "Show advanced settings").
fn prefs_pane() -> impl IntoElement {
    let p = get();
    let advanced = panel().with_untracked(|p| p.advanced);
    let mut pane = view()
        .test_id("settings-prefs")
        .child(section("Appearance"))
        .child(field("Theme", choice("Theme", THEMES, p.theme.clone(), |p, v| p.theme = v.into())))
        .child(field("List density", choice("List density", DENSITIES, p.density.clone(), |p, v| p.density = v.into())))
        .child(flag_field("current_highlight"))
        .child(flag_field("bullet_threading"))
        .child(flag_field("center_doc"))
        .child(flag_field("display_images_inline"))
        .child(check_field("Use true black (dark theme)", p.true_black, |p| p.true_black = !p.true_black))
        .child(section("Viewing options"))
        .child(flag_field("show_checked"))
        .child(pick_field("show_notes"))
        .child(section("Font options"))
        .child(field("Font size", choice("Font size", SIZES, p.size.clone(), |p, v| p.size = v.into())))
        .child(field("Font", choice("Font", FONTS, p.font.clone(), |p, v| p.font = v.into())))
        .child(field(
            "Eastern-Asian font",
            choice("Eastern-Asian font", EA_FONTS, p.eafont.clone(), |p, v| p.eafont = v.into()),
        ))
        .child(section("Input"))
        .child(flag_field("word_count"))
        .child(flag_field("global_tag_autocomplete"))
        .child(flag_field("workflowy_shortcuts"))
        .child(section("Date"))
        .child(pick_field("time_format"))
        .child(pick_field("start_day"))
        .child(text_setting("custom_date_display_format"))
        .child(text_setting("custom_date_display_format_no_time"))
        .child(flag_field("show_timezone"))
        .child(flag_field("highlight_overdue"))
        .child(pick_field("recurrence_check_behavior"))
        .child(section("Internal links"))
        .child(flag_field("show_backlinks"))
        .child(section("Your inbox"))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(num("space.step4")))
                .mb(px(num("space.field_bottom")))
                .child(ui("Inbox destination: "))
                .child(
                    ui(|| -> SharedString {
                        // again when the inbox is chosen
                        prefs();
                        crate::inbox::label().into()
                    })
                    .test_id("inbox-label")
                    .flex_shrink(1.0),
                )
                .child(btn("Change…", false, crate::inbox::pick)),
        )
        .child(pick_field("inbox_move_position"))
        .child(section("Control"))
        .child(flag_field("use_bullet_to_zoom"))
        .child(flag_field("unindent_in_place"))
        .child(flag_field("item_toolbar_hover"))
        .child(
            nsh_base::button()
                .accessibility_label(Some(if advanced { "Hide advanced settings" } else { "Show advanced settings" }.into()))
                .self_start()
                .mb(px(num("space.field_bottom")))
                .on_press(|| panel().update(|p| p.advanced = !p.advanced))
                .child(ui(if advanced { "Hide advanced settings" } else { "Show advanced settings" }).text_color(color::accent())),
        );
    if advanced {
        pane = pane
            .child(section("Advanced"))
            .child(flag_field("auto_focus_first_item"))
            .child(flag_field("show_search_tips"))
            .child(pick_field("dyslexia_font"))
            .child(pick_field("move_item_sent_to_position"))
            .child(hint("Also used when dropping onto a collapsed item.").mb(px(num("space.field_bottom"))))
            .child(flag_field("export_visible_only"));
    }
    pane
}

/// An input of the settings (`.field input`): the app's text in a bordered box.
fn text_field(test_id: &str, value: String, placeholder: String) -> nsh_base::Input {
    let (size, line) = style::ui_sized("type.ui_small");
    nsh_base::input()
        .test_id(test_id)
        .value(value)
        .placeholder(placeholder)
        .font_family(style::font())
        .text_size(px(size))
        .line_height(LineHeight::Points(px(line)))
        .text_color(color::fg())
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.block")))
        .px(px(num("space.input_x")))
        .py(px(num("space.input_y")))
}

/// The control of one appearance entry: a value for it typed (applied on Return or when the input lets go), a
/// color's swatch beside it, a choice's values as buttons; × sets it back to the theme's.
fn entry_row(id: &'static str, doc: &'static str, kind: Kind, choices: &'static [&'static str]) -> impl IntoElement {
    let set = untracked(prefs);
    let set = appearance::raw_entries(&set.appearance).unwrap_or_default();
    let current = set.iter().find(|(x, _)| *x == id).map(|(_, v)| match v {
        toml::Value::String(t) => t.clone(),
        other => other.to_string(),
    });
    let key = id.split_once('.').map_or(id, |(_, k)| k);
    let control: AnyElement = match kind {
        Kind::Choice => {
            let chosen = current.clone().unwrap_or_default();
            view()
                .flex_row()
                .gap(px(num("space.set_tabs_gap")))
                .children(choices.iter().map(|c| {
                    let on = chosen == *c;
                    nsh_base::button()
                        .accessibility_label(Some(format!("{id}: {c}").into()))
                        .selected(on)
                        .px(px(num("space.input_x")))
                        .py(px(num("space.input_y")))
                        .bg(if on { color::sel() } else { color::bg() })
                        .border(px(num("space.border")))
                        .border_color(if on { color::accent() } else { color::chip_border() })
                        .rounded(px(num("radius.block")))
                        .on_press(move || set_entry(id, Some(c)))
                        .child(small(*c).text_color(color::fg()))
                        .into_element()
                }))
                .into_element()
        }
        _ => {
            let typed = signal(current.clone().unwrap_or_default());
            let commit = move || {
                let t = typed.get_untracked();
                set_entry(id, Some(&t));
            };
            let input = text_field(&format!("entry-{id}"), typed.get_untracked(), appearance::base_text(id))
                .w(px(num("space.sidebar")))
                .on_change(move |t: &str| typed.set(t.to_owned()))
                .on_submit(move |_: &str| commit())
                .on_blur(move || {
                    // only a value that changed: letting go of an untouched input sets nothing
                    let t = typed.get_untracked();
                    if t != untracked(|| current_of(id)).unwrap_or_default() {
                        set_entry(id, Some(&t));
                    }
                });
            let mut row = view().flex_row().items_center().gap(px(num("space.set_tabs_gap")));
            if kind == Kind::Color {
                let look = style::look();
                let size = num("space.check");
                row = row.child(
                    view()
                        .test_id(&format!("swatch-{id}"))
                        .w(px(size))
                        .h(px(size))
                        .rounded(px(num("radius.code")))
                        .border(px(num("space.border")))
                        .border_color(color::chip_border())
                        .bg(rgba(look.color(id))),
                );
            }
            row.child(input).into_element()
        }
    };
    // phones: the name and its description over the control, which would leave them no room beside it
    let row = view().test_id("entry");
    let row = if style::phone() { row.flex_col().items_start() } else { row.flex_row().items_center() };
    let mut row = row
        .gap(px(num("space.km_gap")))
        .py(px(num("space.km_y")))
        .border_b(px(num("space.border")))
        .border_color(color::line())
        .child(view().flex_grow(1.0).child(small(key).text_color(color::fg())).child(hint(doc)))
        .child(control);
    if current.is_some() {
        row = row.child(
            nsh_base::button()
                .accessibility_label(Some(format!("Reset {id}").into()))
                .px(px(num("space.km_y")))
                .on_press(move || set_entry(id, None))
                .child(small("×").text_color(color::muted())),
        );
    }
    row
}

/// What the appearance in use sets `id` to, as its control shows it.
fn current_of(id: &str) -> Option<String> {
    let p = prefs();
    appearance::raw_entries(&p.appearance).ok()?.into_iter().find(|(x, _)| *x == id).map(|(_, v)| match v {
        toml::Value::String(t) => t,
        other => other.to_string(),
    })
}

/// The Look tab: the appearance TOML, its export and import, and a control per entry, table by table.
fn look_pane() -> impl IntoElement {
    let text = signal(untracked(draft));
    let field = element_ref();
    // the TOML used when it is valid; else its error's line selected, so the eye goes there (the web's `change`)
    let settle = move || {
        let t = text.get_untracked();
        if t == untracked(prefs).appearance {
            return;
        }
        if !apply_toml(t.clone()) {
            if let Some(e) = panel().with_untracked(|p| p.error.clone()).filter(|e| e.line > 0) {
                let (a, b) = line_range(&t, e.line);
                field.set_selection(a, b);
            }
        }
    };
    let (size, line) = style::ui_sized("type.ui_small");
    let toml_input = nsh_base::input()
        .element_ref(&field)
        .test_id("toml")
        .value(text.get_untracked())
        .placeholder("[color]\ntag = \"#008080\"\n\n[type]\nsize = 17")
        .multiline(true)
        .submit_behavior(SubmitBehavior::Newline)
        .auto_correct(false)
        .font_family(style::mono())
        .text_size(px(size))
        .line_height(LineHeight::Points(px(line)))
        .min_h(px(num("space.toml_min")))
        .text_color(color::fg())
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(move || {
            if panel().with(|p| p.error.as_ref().is_some_and(|e| e.line > 0)) {
                AdaptiveColor::from(rgba(style::look().color("color.danger")))
            } else {
                AdaptiveColor::from(color::chip_border())
            }
        })
        .rounded(px(num("radius.block")))
        .px(px(num("space.input_x")))
        .py(px(num("space.input_y")))
        .on_change(move |t: &str| {
            text.set(t.to_owned());
            // checked as it is typed: the error shows at once, the look changes on Apply
            let error = appearance::parse(t).err();
            panel().update(|p| {
                p.draft = Some(t.to_owned());
                p.error = error;
            });
        })
        // letting go of the text applies it as Apply does
        .on_blur(settle);
    let danger = rgba(style::look().color("color.danger"));
    let error = view().test_id("toml-error").child(
        small(move || {
            panel().with(|p| match &p.error {
                Some(e) if e.line > 0 => format!("Line {}: {}", e.line, e.message),
                Some(e) => e.message.clone(),
                None => String::new(),
            })
        })
        .text_color(danger),
    );
    let status = view().test_id("look-status").child(hint(move || panel().with(|p| p.status.clone())));
    let buttons = view()
        .flex_row()
        .flex_wrap()
        .gap(px(num("space.modal_foot_gap")))
        .mb(px(num("space.field_bottom")))
        .child(btn("Apply", false, settle))
        .child(btn("Export .toml", false, export))
        .child(btn("Import .toml", false, import));
    let mut tables: Vec<&'static str> = Vec::new();
    for e in SCHEMA {
        let t = e.id.split_once('.').map_or(e.id, |(t, _)| t);
        if !tables.contains(&t) {
            tables.push(t);
        }
    }
    let changed = appearance::raw_entries(&untracked(prefs).appearance).unwrap_or_default();
    let open = panel().with_untracked(|p| p.tables.clone());
    let controls = view().test_id("entries").children(tables.into_iter().map(|t| {
        let n = changed.iter().filter(|(id, _)| id.split_once('.').is_some_and(|(x, _)| x == t)).count();
        let shown = open.contains(&t);
        let head = nsh_base::button()
            .accessibility_label(Some(format!("[{t}]").into()))
            .flex_row()
            .py(px(num("space.km_y")))
            .on_press(move || {
                panel().update(|p| {
                    if let Some(i) = p.tables.iter().position(|x| *x == t) {
                        p.tables.remove(i);
                    } else {
                        p.tables.push(t);
                    }
                })
            })
            .child(small(format!("{} [{t}]", if shown { "▾" } else { "▸" })).text_color(color::fg()))
            .child(hint(if n > 0 { format!(" {n} changed") } else { String::new() }));
        let mut block = view().child(head);
        if shown {
            block = block.child(
                view().children(
                    SCHEMA
                        .iter()
                        .filter(|e| e.id.split_once('.').is_some_and(|(x, _)| x == t))
                        .map(|e| entry_row(e.id, e.doc, e.kind, e.choices).into_element()),
                ),
            );
        }
        block.into_element()
    }));
    view()
        .test_id("settings-look")
        .child(field_hint())
        .child(
            view()
                .flex_col()
                .gap(px(num("space.field_gap")))
                .mb(px(num("space.field_bottom")))
                .child(ui("Appearance (TOML)"))
                .child(toml_input)
                .child(error)
                .child(status),
        )
        .child(buttons)
        .child(controls)
}

/// The Keymap tab (`keymapEditor`): the commands with their keys, found by a search; × takes a key away, + records
/// a new one (the next key pressed with a modifier, Escape to stop), and a command's keys or all of them go back
/// to the defaults. Changes are kept as the web's `shortcut_override` and bound on desktops at once.
fn keymap_pane() -> impl IntoElement {
    let search = text_field("km-search", panel().with_untracked(|p| p.filter.clone()), "Search commands…".into())
        .mb(px(num("space.field_bottom")))
        .on_change(|t: &str| panel().update(|p| p.filter = t.to_owned()));
    let list = dynamic(|| {
        let p = prefs();
        let (q, recording) = panel().with(|p| (p.filter.to_lowercase(), p.recording));
        let apple = apple_keys();
        let overrides = p.shortcuts();
        let km = crate::keymap::Keymap::new(&overrides, p.flag("workflowy_shortcuts"), apple);
        let rows = crate::keymap::COMMANDS
            .iter()
            .filter(|c| q.is_empty() || c.label.to_lowercase().contains(&q) || c.id.contains(&q))
            .map(|c| keymap_row(c, &km, &overrides, apple, recording == Some(c.id)).into_element());
        let list = view().test_id("km-list").children(rows.collect::<Vec<_>>());
        match recording {
            Some(id) => view().child(recorder(id)).child(list).into_element(),
            None => list.into_element(),
        }
    });
    view()
        .test_id("settings-keymap")
        .child(search)
        .child(list)
        .child(view().mt(px(num("space.field_bottom"))).child(btn("Reset all shortcuts", false, || {
            change(|p| p.set_shortcuts(&BTreeMap::new()));
        })))
}

/// A command's row in the Keymap tab (`.km-row`): its label, its keys (each with ×), + (or "fixed"), and "Reset to
/// default" when its keys were changed.
fn keymap_row(
    c: &'static crate::keymap::Command,
    km: &crate::keymap::Keymap,
    overrides: &BTreeMap<String, Vec<String>>,
    apple: bool,
    recording: bool,
) -> impl IntoElement {
    let fixed = crate::keymap::FIXED.contains(&c.id);
    let id = c.id;
    let chips = km.keys_for(id).into_iter().map(|k| {
        let mut chip = view()
            .test_id("km-key")
            .flex_row()
            .items_center()
            .gap(px(num("space.step2")))
            .px(px(num("space.step3")))
            .bg(color::chip())
            .border(px(num("space.border")))
            .border_color(color::chip_border())
            .rounded(px(num("radius.code")))
            .child(small(crate::keymap::pretty(&k, apple)).text_color(color::fg()));
        if !fixed {
            chip = chip.child(
                nsh_base::button()
                    .accessibility_label(Some(format!("Remove {k} from {}", c.label).into()))
                    .on_press(move || {
                        let p = get();
                        let km = crate::keymap::Keymap::new(&p.shortcuts(), p.flag("workflowy_shortcuts"), apple_keys());
                        let o = km.without_key(&p.shortcuts(), id, &k);
                        change(|p| p.set_shortcuts(&o));
                    })
                    .child(small("×").text_color(color::muted())),
            );
        }
        chip.into_element()
    });
    let mut keys = view().flex_row().flex_wrap().items_center().gap(px(num("space.step3"))).children(chips.collect::<Vec<_>>());
    keys = if fixed {
        keys.child(hint("fixed"))
    } else {
        keys.child(
            nsh_base::button()
                .accessibility_label(Some(format!("Add a key to {}", c.label).into()))
                .px(px(num("space.step4")))
                .border(px(num("space.border")))
                .border_color(color::chip_border())
                .rounded(px(num("radius.block")))
                .on_press(move || panel().update(|p| p.recording = Some(id)))
                .child(small(if recording { "Press keys…" } else { "+" }).text_color(color::fg())),
        )
    };
    if overrides.contains_key(id) {
        keys = keys.child(
            nsh_base::button()
                .accessibility_label(Some(format!("Reset {} to default", c.label).into()))
                .on_press(move || {
                    let mut o = get().shortcuts();
                    o.remove(id);
                    change(|p| p.set_shortcuts(&o));
                })
                .child(small("Reset to default").text_color(color::accent())),
        );
    }
    view()
        .test_id("km-row")
        .flex_row()
        .items_center()
        .gap(px(num("space.km_gap")))
        .py(px(num("space.km_y")))
        .border_b(px(num("space.border")))
        .border_color(color::line())
        .child(view().flex_1().child(small(c.label).text_color(color::fg())))
        .child(keys)
}

/// While a key is recorded for command `id`: every key with a modifier (and the function keys) is listened for;
/// the first one pressed is added to the command, taken from the command that had it. Escape stops.
fn recorder(id: &'static str) -> impl IntoElement {
    let opts = KeyOptions { when: nsh_core::keymap::When::Always, ..Default::default() };
    let apple = apple_keys();
    let os = os();
    on_key_with("escape", opts, |_| panel().update(|p| p.recording = None));
    for key in crate::keymap::recordable_keys() {
        for combo in crate::keymap::combos_of(&key, apple) {
            let Some(ks) = crate::keymap::keystroke(&combo, apple) else { continue };
            let reserved = nsh_core::keymap::Keystroke::parse_with(&ks, apple).ok().and_then(|k| k.reserved(os));
            if reserved.is_some() {
                continue;
            }
            on_key_with(&ks, opts, move |_| {
                let p = get();
                let km = crate::keymap::Keymap::new(&p.shortcuts(), p.flag("workflowy_shortcuts"), apple);
                let o = km.with_key(&p.shortcuts(), id, &combo);
                panel().update(|p| p.recording = None);
                change(|p| p.set_shortcuts(&o));
            });
        }
    }
    view()
}

/// A section of the Sync & data tab (`.set-sec`): its head over its content.
fn data_section(title: &'static str) -> View {
    view().child(section(title))
}

/// The Sync & data tab: the sync server's sign-in as the web shows it with no server (sync is not in the native
/// app yet), the backup and the import.
fn data_pane() -> impl IntoElement {
    let status = view().test_id("data-status").child(hint(|| panel().with(|p| p.data_status.clone())));
    view()
        .test_id("settings-data")
        .child(
            data_section("Sync server")
                .child(field("Sync server URL", text_field("sync-url", String::new(), "https://my-server.example".into())))
                .child(field("Password", text_field("sync-password", String::new(), "Password".into()).masked(true)))
                .child(btn("Connect", true, || {
                    panel().update(|p| p.data_status = "Sync is not in this app yet: documents stay on this device.".into())
                })),
        )
        .child(
            data_section("Backup")
                .child(
                    hint("A zip of every document as plain text and OPML, in its folder structure. The sync server also keeps daily database backups.")
                        .mb(px(num("space.field_bottom"))),
                )
                .child(btn("Download backup", false, download_backup)),
        )
        .child(
            data_section("Import")
                .child(btn("Import OPML / text files…", false, || import_files(None)))
                .child(hint("From WorkFlowy: export as OPML there, then import the file here.").mt(px(num("space.field_gap")))),
        )
        .child(view().mt(px(num("space.field_bottom"))).child(status))
}

/// Download backup (`downloadBackup`): the zip saved where the user says on desktops; phones have no save
/// dialog, so it goes to the app's own folder.
fn download_backup() {
    let now = store::now();
    let bytes = crate::backup::zip(&store::peek(crate::backup::backup_files), now);
    let name = crate::backup::backup_name(now);
    let write = |path: std::path::PathBuf, bytes: Vec<u8>| {
        let status = match std::fs::write(&path, bytes) {
            Ok(()) => format!("Saved {}.", path.display()),
            Err(e) => format!("Could not write {}: {e}", path.display()),
        };
        panel().update(|p| p.data_status = status);
    };
    if style::phone() {
        let Some(dir) = data_dir() else {
            panel().update(|p| p.data_status = "This device keeps no files for the app.".into());
            return;
        };
        let dir = dir.join("backups");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            panel().update(|p| p.data_status = format!("Could not make {}: {e}", dir.display()));
            return;
        }
        write(dir.join(name), bytes);
        return;
    }
    spawn(async move {
        let dialog = SaveDialog::new().file_name(&name).filter(FileFilter::new("Zip", ["zip"]));
        match pick_save_path(dialog).await {
            Ok(Some(path)) => write(path, bytes),
            Ok(None) => {}
            Err(e) => panel().update(|p| p.data_status = format!("Could not save: {e}")),
        }
    });
}

/// Import OPML / text files (`importOpmlFile`): each file a new document (OPML with its title) in folder
/// `parent` (the top level for `None`), the last one opened.
pub(crate) fn import_files(parent: Option<String>) {
    spawn(async move {
        let mut request = FilePickRequest::new(true, 0);
        request.filters = vec![FileFilter::new("OPML or text", ["opml", "xml", "txt", "md"])];
        let files = pick_files(request).await;
        if files.is_empty() {
            return;
        }
        let mut done = Vec::new();
        for f in files {
            let text = String::from_utf8_lossy(f.data.bytes()).into_owned();
            match crate::backup::import_file(&f.name, &text) {
                Some((title, items)) => {
                    crate::workspace::import(parent.as_deref(), &title, &items);
                    done.push(f.name.clone());
                }
                None => done.push(format!("Could not read {}", f.name)),
            }
        }
        panel().update(|p| p.data_status = format!("Imported: {}", done.join(", ")));
    });
}

/// The UTF-16 range of line `line` (from 1) of `text`, without its line break.
fn line_range(text: &str, line: usize) -> (u32, u32) {
    let mut start = 0u32;
    for (i, l) in text.split('\n').enumerate() {
        let len = l.encode_utf16().count() as u32;
        if i + 1 == line {
            return (start, start + len);
        }
        start += len + 1;
    }
    let end = text.encode_utf16().count() as u32;
    (end, end)
}

fn field_hint() -> impl IntoElement {
    hint(
        "Change any part of the look over the chosen theme, size and density. It is a TOML file in the same format as \
         the built-in themes (design/schema.md); only what you change is kept.",
    )
    .mb(px(num("space.field_bottom")))
}

/// The tabs (`.set-tabs`).
fn tabs(on: Tab) -> impl IntoElement {
    let tab = move |which: Tab, label: &'static str| {
        let active = on == which;
        nsh_base::button()
            .accessibility_label(Some(label.into()))
            .selected(active)
            .px(px(num("space.set_tab_x")))
            .py(px(num("space.set_tab_y")))
            .border_b(px(num("space.set_tab_bar")))
            .border_color(if active { color::accent().into() } else { AdaptiveColor::TRANSPARENT })
            .on_press(move || panel().update(|p| p.tab = which))
            .child(ui(label).text_color(if active { color::fg() } else { color::muted() }))
    };
    view()
        .test_id("settings-tabs")
        .flex_row()
        .gap(px(num("space.set_tabs_gap")))
        .border_b(px(num("space.border")))
        .border_color(color::line())
        .mb(px(num("space.set_tabs_bottom")))
        .child(tab(Tab::Prefs, "Preferences"))
        .child(tab(Tab::Look, "Look"))
        .child(tab(Tab::Keymap, "Keymap"))
        .child(tab(Tab::Data, "Sync & data"))
}

/// The settings dialog over the screen, while it is open (`.overlay` > `.modal`): its title, the tabs over the
/// pane, and Done.
pub fn screen() -> impl IntoElement {
    when(
        || panel().with(|p| p.open),
        || {
            let title_size = num("type.modal_title");
            let line = title_size * num("type.ui_line");
            // only what the panes are made from: the draft and its error change as the TOML is typed, without
            // making its input again
            let tab = computed(|| panel().with(|p| p.tab));
            let tables = computed(|| panel().with(|p| p.tables.clone()));
            let advanced = computed(|| panel().with(|p| p.advanced));
            let pane = dynamic(move || match tab.get() {
                Tab::Prefs => {
                    // a choice that leaves the look as it was (true black in a light theme) still shows
                    prefs();
                    advanced.get();
                    prefs_pane().into_element()
                }
                Tab::Look => {
                    // the controls follow the tables shown
                    tables.get();
                    look_pane().into_element()
                }
                Tab::Keymap => keymap_pane().into_element(),
                Tab::Data => data_pane().into_element(),
            });
            let modal = view()
                .test_id("settings")
                .flex_col()
                .bg(color::bg())
                .rounded(px(num("radius.modal")))
                .overflow_hidden()
                .w(|| px(num("space.settings").min(window().width.0 * num("space.settings_vw"))))
                .max_h(|| px(window().height.0 * num("space.settings_vh")))
                .shadow([BoxShadow {
                    blur: px(num("space.drawer_shadow")),
                    color: rgba(style::look().color("color.shadow_modal")).into(),
                    ..Default::default()
                }])
                .child(
                    ui_text("Settings", (title_size, line))
                        .text_color(color::fg())
                        .px(px(num("space.modal_x")))
                        .py(px(num("space.modal_head_y")))
                        .border_b(px(num("space.border")))
                        .border_color(color::line()),
                )
                .child(
                    scroll().test_id("settings-body").flex_shrink(1.0).child(
                        view()
                            .px(px(num("space.modal_x")))
                            .py(px(num("space.modal_body_y")))
                            .child(dynamic(move || tabs(tab.get())))
                            .child(pane),
                    ),
                )
                .child(
                    view()
                        .flex_row()
                        .justify_end()
                        .gap(px(num("space.modal_foot_gap")))
                        .px(px(num("space.modal_x")))
                        .py(px(num("space.modal_foot_y")))
                        .border_t(px(num("space.border")))
                        .border_color(color::line())
                        .child(btn("Done", true, close)),
                );
            view()
                .test_id("settings-overlay")
                .absolute()
                .top(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .items_center()
                .pt(|| px(window().height.0 * num("space.overlay_top")))
                .bg(rgba(style::look().color("color.overlay")))
                .child(modal)
        },
    )
}

/// The top bar's menu (`.menu`), while it shows: the web's items the native app has (version history, print,
/// the sync status and About are not here).
pub fn menu() -> impl IntoElement {
    when(
        || panel().with(|p| p.menu),
        || {
            let item = |label: &'static str, run: fn()| {
                nsh_base::button()
                    .accessibility_label(Some(label.into()))
                    .px(px(num("space.menu_item_x")))
                    .py(px(num("space.menu_item_y")))
                    .on_press(move || {
                        panel().update(|p| p.menu = false);
                        run();
                    })
                    .child(ui(label))
            };
            view()
                .absolute()
                .top(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .child(
                    view()
                        .test_id("menu-scrim")
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(0.0))
                        .right(px(0.0))
                        .on_press(toggle_menu),
                )
                .child(
                    view()
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .right(px(0.0))
                        .pt_safe_area()
                        .items_end()
                        .pr(px(num("space.topbar_pad")))
                        .child(
                            menu_box()
                                .child(item("Shortcuts & formatting", || crate::help::toggle(Some(true))))
                                .child(item("Settings", || open(Tab::Prefs)))
                                .child(item("Capture to inbox", crate::inbox::capture))
                                .child(item("Go to inbox", crate::inbox::goto))
                                .child(item("Search everywhere", crate::find::search_everywhere))
                                .child(item("Search and replace", crate::find::replace_dialog))
                                .child(item("Export…", crate::export::open))
                                .child(item("Download backup", download_backup))
                                .child(item("Import OPML / text…", || import_files(None)))
                                .child(item("Tutorial", crate::help::tutorial))
                                .child(item("What's new", crate::help::whats_new)),
                        ),
                )
        },
    )
}

/// The box of the phone's menu, right under the top bar.
fn menu_box() -> View {
    view()
        .test_id("menu")
        .mt(px(style::topbar_h()))
        .min_w(px(num("space.menu_min")))
        .py(px(num("space.menu_y")))
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.menu")))
        .shadow([BoxShadow {
            blur: px(num("space.drawer_shadow")),
            color: rgba(style::look().color("color.shadow_menu")).into(),
            ..Default::default()
        }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_system_and_true_black_darkens_dark() {
        let p = Prefs::default();
        assert_eq!(p.modes(false).theme, "light");
        assert_eq!(p.modes(true).theme, "dark");
        let black = Prefs { true_black: true, ..Prefs::default() };
        assert_eq!(black.modes(true).theme, "black");
        assert_eq!(black.modes(false).theme, "light");
        let sepia = Prefs { theme: "sepia".into(), true_black: true, ..Prefs::default() };
        assert_eq!(sepia.modes(true).theme, "sepia");
    }

    #[test]
    fn an_error_line_is_found_in_utf16() {
        let t = "[color]\nfg = \"한\"\ntag = 1\n";
        assert_eq!(line_range(t, 1), (0, 7));
        assert_eq!(line_range(t, 2), (8, 16));
        assert_eq!(line_range(t, 3), (17, 24));
        assert_eq!(line_range(t, 4), (25, 25));
        assert_eq!(line_range(t, 9), (25, 25));
    }

    #[test]
    fn every_choice_is_a_mode() {
        for (group, choices) in
            [("theme", THEMES), ("size", SIZES), ("density", DENSITIES), ("font", FONTS), ("eafont", EA_FONTS)]
        {
            for (c, _) in choices.iter().filter(|(c, _)| *c != AUTO) {
                assert!(Modes::parse(&format!("{group}={c}")).is_ok(), "{group}={c}");
            }
        }
    }
}
