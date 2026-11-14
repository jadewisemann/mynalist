//! The display settings, as the web app's (`apps/web/src/ui/settings.ts` and `ui/appearance.ts`): the theme
//! (following the system's light or dark appearance by default), the text size, the list density, the fonts, and
//! the user's appearance TOML (`design/schema.md`). The choices are kept on the device (NSH storage, under the web's
//! pref names) and applied at once: the look is resolved again (`appearance::set`) and the screen made again
//! from it.
//!
//! The settings screen is the web's settings dialog with the tabs the native app has: Preferences (the modes) and
//! Look (the appearance: its TOML text, checked with the error at its line, one control per schema entry, export
//! and import as a .toml file). Desktops open it with the top bar's gear, phones from the top bar's menu.

use nsh_core::file_dialog::{FileFilter, SaveDialog};
use nsh_core::picker::FilePickRequest;
use nsh_core::prelude::*;
use nsh_core::storage::Storage;
use nsh_core::LineHeight;

use crate::appearance::{self, Modes};
use crate::design::{Kind, SCHEMA};
use crate::sidebar::ui_text;
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
        }
    }
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
        }
    }

    /// Keeps the prefs on the device: a default is not kept.
    fn store(&self) {
        let d = Prefs::default();
        let keep = |key: &str, v: &str, default: &str| {
            if v == default {
                Storage::remove_item(key);
            } else if let Err(e) = Storage::set_item(key, v) {
                eprintln!("settings: {key} not kept: {e:?}");
            }
        };
        keep(THEME, &self.theme, &d.theme);
        keep(TRUE_BLACK, if self.true_black { "true" } else { "false" }, "false");
        keep(SIZE, &self.size, &d.size);
        keep(DENSITY, &self.density, &d.density);
        keep(FONT, &self.font, &d.font);
        keep(EAFONT, &self.eafont, &d.eafont);
        keep(APPEARANCE, &self.appearance, "");
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
            font: self.font.clone(),
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
    let mut p = untracked(prefs);
    f(&mut p);
    p.store();
    current().set(Current(Some(p)));
}

/// Applies the prefs and the system's appearance to the look (an effect of the app's root: it runs again when
/// either changes).
pub fn apply() {
    let p = prefs();
    let modes = p.modes(dark());
    if appearance::set(modes.clone(), &p.appearance).is_err() {
        // kept only once valid: this is a stored value from elsewhere, so the look goes without it
        let _ = appearance::set(modes, "");
    }
}

/// Which tab the settings show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Prefs,
    Look,
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
        p.draft = None;
        p.error = None;
        p.status.clear();
    });
}

/// Opens or closes the phone's top bar menu.
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

/// The Preferences tab: the settings of the look the native app has.
fn prefs_pane() -> impl IntoElement {
    let p = untracked(prefs);
    view()
        .test_id("settings-prefs")
        .child(section("Appearance"))
        .child(field("Theme", choice("Theme", THEMES, p.theme.clone(), |p, v| p.theme = v.into())))
        .child(field("List density", choice("List density", DENSITIES, p.density.clone(), |p, v| p.density = v.into())))
        .child(check_field("Use true black (dark theme)", p.true_black, |p| p.true_black = !p.true_black))
        .child(section("Font options"))
        .child(field("Font size", choice("Font size", SIZES, p.size.clone(), |p, v| p.size = v.into())))
        .child(field("Font", choice("Font", FONTS, p.font.clone(), |p, v| p.font = v.into())))
        .child(field(
            "Eastern-Asian font",
            choice("Eastern-Asian font", EA_FONTS, p.eafont, |p, v| p.eafont = v.into()),
        ))
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
    let (size, line) = style::ui_sized("type.ui_small");
    let toml_input = nsh_base::input()
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
        });
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
        .child(btn("Apply", false, move || {
            apply_toml(text.get_untracked());
        }))
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
            let pane = dynamic(move || match tab.get() {
                Tab::Prefs => {
                    // a choice that leaves the look as it was (true black in a light theme) still shows
                    prefs();
                    prefs_pane().into_element()
                }
                Tab::Look => {
                    // the controls follow the tables shown
                    tables.get();
                    look_pane().into_element()
                }
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

/// The phone top bar's menu (`.menu`), while it shows: the settings (the web's other items are not here yet).
pub fn menu() -> impl IntoElement {
    when(
        || panel().with(|p| p.menu),
        || {
            let item = |label: &'static str, run: fn()| {
                nsh_base::button()
                    .accessibility_label(Some(label.into()))
                    .px(px(num("space.menu_item_x")))
                    .py(px(num("space.menu_item_y")))
                    .on_press(run)
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
                        .child(menu_box().child(item("Settings", || open(Tab::Prefs)))),
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
