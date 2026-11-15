//! The date picker (the web's `ui/datepicker.ts`, opened as `Popups.addDate` and a typed `!` open it): a month
//! calendar, an optional time, an optional range with its end time, a repeat with its presets or a custom rule,
//! "From completion"; Done writes the `!(…)` token, Delete removes the date being edited, Cancel puts the caret back.
//! It opens from the phone toolbar's date button, Mod+D, and `!` typed after a space.
//!
//! While it shows, nothing is edited (the web keeps the item's caret and passes the keys to the picker; here the
//! row's input lets go so the picker's own keys and fields take them), and the caret comes back when it closes.
//! Its sizes are the web's `.dp*` tokens (`space.datepicker`, `space.dp_*`, the `space.stepN` scale).
//! On desktops it is a popover under the caret (or above it when there is more room there), at the caret's left
//! as the web's `placePopup`; it sits in the middle when the caret's place is not known (items selected). Phones
//! show it as a sheet at the bottom. The week's first day and the 12/24-hour clock are the settings' (`start_day`,
//! `time_format`); a new date gets the local timezone with "Encode timezone in new dates" on.

use nsh_base::{button, input};
use nsh_core::keymap::When;
use nsh_core::prelude::*;
use outliner_core::dates::{
    self, describe_recurrence, format_date_token, format_time, DateSpec, DateTime, RecurUnit, Recurrence, MONTH_NAMES,
    WEEKDAY_NAMES,
};
use outliner_core::item;

use crate::commands as cmd;
use crate::store::{self, Caret};
use crate::style::{self, color, num};

const TIME_ERROR: &str = "Please enter a valid time format like 22:42";
const COUNT_ERROR: &str = "Enter a whole number of 1 or more";
/// Times are 12-hour (`time_format`, the web's default) or 24-hour.
fn hour12() -> bool {
    crate::settings::get().pick("time_format") == "12h"
}
/// The share of the accent behind the days inside a range (`.dp-day.in-range`: `color-mix(.. 14%, transparent)`).
const IN_RANGE_MIX: f32 = 0.14;

/// A step of the web-only screens' spacing scale (`--step-N`, `space.stepN`).
fn step(n: u8) -> f32 {
    num(&format!("space.step{n}"))
}

/// A calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Day {
    pub year: i64,
    pub month: i64,
    pub day: i64,
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_of(d: Day) -> i64 {
    let y = if d.month <= 2 { d.year - 1 } else { d.year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (d.month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d.day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn day_of(z: i64) -> Day {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    Day { year: yoe + era * 400 + i64::from(month <= 2), month, day }
}

impl Day {
    fn add(self, n: i64) -> Day {
        day_of(days_of(self) + n)
    }
    /// 0 = Sunday, as `getDay()`.
    fn weekday(self) -> i64 {
        (days_of(self) + 4).rem_euclid(7)
    }
    /// ISO weekday: 1 = Monday .. 7 = Sunday.
    fn iso(self) -> i64 {
        match self.weekday() {
            0 => 7,
            w => w,
        }
    }
    fn add_months(self, n: i64) -> Day {
        let idx = self.year * 12 + self.month - 1 + n;
        let (year, month) = (idx.div_euclid(12), idx.rem_euclid(12) + 1);
        Day { year, month, day: self.day.min(dates::days_in_month(year, month)) }
    }
    fn first(self) -> Day {
        Day { day: 1, ..self }
    }
    fn today() -> Day {
        let f = dates::fields(store::now());
        Day { year: f.year, month: f.month0 + 1, day: f.day }
    }
    fn of(dt: &DateTime) -> Day {
        Day { year: dt.year, month: dt.month, day: dt.day }
    }
}

/// A typed time: "22:42", "9", "3pm", "3:30 pm", "1530", "All day" (or nothing), "Now", "noon", "midnight"
/// (`parseTimeInput`). `Some(None)`: all day; `None`: not a time.
pub fn parse_time(raw: &str, now: i64) -> Option<Option<(i64, i64)>> {
    let s = raw.trim().to_lowercase().replace('.', "");
    match s.as_str() {
        "" | "all day" | "allday" => return Some(None),
        "now" => {
            let f = dates::fields(now);
            return Some(Some((f.hours, f.minutes)));
        }
        "noon" => return Some(Some((12, 0))),
        "midnight" => return Some(Some((0, 0))),
        _ => {}
    }
    // ^(\d{1,2})(?:(?::|h)?(\d{2}))?\s*(am|pm|a|p)?$
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && i < 2 && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let mut hour: i64 = s[..i].parse().ok()?;
    let mut rest = &s[i..];
    let mut minute = 0;
    let after_sep = rest.strip_prefix(':').or_else(|| rest.strip_prefix('h')).unwrap_or(rest);
    if after_sep.len() >= 2 && after_sep.as_bytes()[..2].iter().all(u8::is_ascii_digit) {
        minute = after_sep[..2].parse().ok()?;
        rest = &after_sep[2..];
    } else if after_sep.len() != rest.len() {
        // a separator with no minutes after it
        return None;
    }
    let ap = rest.trim_start();
    if !matches!(ap, "" | "am" | "pm" | "a" | "p") {
        return None;
    }
    if minute > 59 {
        return None;
    }
    if !ap.is_empty() {
        if !(1..=12).contains(&hour) {
            return None;
        }
        hour = hour % 12 + if ap.starts_with('p') { 12 } else { 0 };
    } else if hour > 23 {
        return None;
    }
    Some(Some((hour, minute)))
}

fn ordinal(n: i64) -> String {
    let v = n % 100;
    if (11..=13).contains(&v) {
        return format!("{n}th");
    }
    let suffix = match n % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn time_text(dt: Option<&DateTime>) -> String {
    match dt.and_then(|d| d.hour.map(|h| (h, d.minute.unwrap_or(0)))) {
        Some((h, m)) => format_time(h, m, hour12()),
        None => String::new(),
    }
}

/// The custom repeat being made (the web's "Custom recurrence" dialog).
#[derive(Debug, Clone, PartialEq)]
struct Custom {
    unit: RecurUnit,
    weekdays: Vec<i64>,
    error: bool,
}

/// The open picker: where its date goes, and what it shows.
#[derive(Debug, Clone, PartialEq, Default)]
struct Picker {
    open: bool,
    /// The item and field, and the range of the text the token replaces.
    id: String,
    note: bool,
    s: u32,
    e: u32,
    /// Opened by a typed `!` (the `!` is replaced), or inserting a new date (spaces around it as needed).
    typed: bool,
    insert: bool,
    /// Editing a date already there (Delete shows).
    existing: bool,
    start: Day,
    end: Day,
    range: bool,
    picking_end: bool,
    view: Day,
    recur: Option<Recurrence>,
    from_completion: bool,
    error: Option<String>,
    custom: Option<Custom>,
    /// The caret's box in the screen when the picker opened (left, top, bottom), if known: the desktop popover
    /// goes under it.
    anchor: Option<(f32, f32, f32)>,
}

/// An item, its note (or content), and a caret's left, top and bottom in the screen.
type CaretAt = (String, bool, f32, f32, f32);

thread_local! {
    /// Where the caret of an input editing an item was last seen in the screen: the item, its note or content,
    /// and the caret's left, top and bottom. Not a signal: only opening the picker reads it.
    static CARET_BOX: std::cell::RefCell<Option<CaretAt>> = const { std::cell::RefCell::new(None) };
}

/// The caret of the input editing `id`'s content (or note) moved to `b` in the screen (the inputs report it).
pub fn caret_at(id: &str, note: bool, b: Bounds<Points>) {
    if b.size.height.0 > 0.0 {
        let v = Some((id.to_owned(), note, b.origin.x.0, b.origin.y.0, b.origin.y.0 + b.size.height.0));
        CARET_BOX.with(|c| *c.borrow_mut() = v);
    }
}

fn picker_state() -> Signal<Picker> {
    global::<Picker>()
}

/// What is typed in the picker's fields: kept apart from [`Picker`], which the picker's views are made from, so
/// that typing does not make the fields again.
#[derive(Debug, Clone, PartialEq, Default)]
struct Typed {
    time: String,
    end_time: String,
    n: String,
}

fn typed_state() -> Signal<Typed> {
    global::<Typed>()
}

fn is_ws(c: Option<u16>) -> bool {
    c.is_none_or(|c| char::from_u32(u32::from(c)).is_some_and(char::is_whitespace))
}

/// Open the picker on the text `s..e` of `id`'s content or note: the date there (`spec`), or a new one.
fn open(id: &str, note: bool, s: u32, e: u32, spec: Option<DateSpec>, typed: bool, insert: bool) {
    let today = Day::today();
    let start = spec.as_ref().map_or(today, |s| Day::of(&s.start));
    let end = spec.as_ref().and_then(|s| s.end.as_ref()).map_or(start, Day::of);
    let recur = spec.as_ref().and_then(|s| s.recur.clone()).map(|r| Recurrence { from_completion: false, ..r });
    let p = Picker {
        open: true,
        id: id.to_owned(),
        note,
        s,
        e,
        typed,
        insert,
        existing: spec.is_some(),
        start,
        end,
        range: spec.as_ref().is_some_and(|s| s.end.is_some()),
        picking_end: false,
        view: start.first(),
        from_completion: spec.as_ref().and_then(|s| s.recur.as_ref()).is_some_and(|r| r.from_completion),
        recur,
        error: None,
        custom: None,
        anchor: CARET_BOX
            .with(|c| c.borrow().clone())
            .filter(|(i, n, ..)| i == id && *n == note)
            .map(|(_, _, x, top, bottom)| (x, top, bottom)),
    };
    // the row's input lets go: the picker's keys and fields take the keys
    store::edit(|s| {
        s.editing = None;
        s.caret_rev += 1;
    });
    typed_state().set(Typed {
        time: time_text(spec.as_ref().map(|s| &s.start)),
        end_time: time_text(spec.as_ref().and_then(|s| s.end.as_ref())),
        n: String::new(),
    });
    picker_state().set(p);
}

/// The toolbar's date button and Mod+D (`Popups.addDate`): edits the date the caret is in, or inserts a new one at
/// the caret (after the word it is in); with items selected, the first one's first date, or a new one at its end.
pub fn add_date() {
    let target = store::peek(|s| {
        if let Some(c) = &s.editing {
            return Some((c.clone(), true));
        }
        let id = s.block.as_ref()?.ids.first()?.clone();
        let len = cmd::utf16_len(item::content(s.tree().get(&id)));
        Some((Caret::text(&id, len), false))
    });
    let Some((c, editing)) = target else { return };
    let text = store::peek(|s| {
        let n = s.tree().get(&c.id);
        if c.note { item::note(n) } else { item::content(n) }.to_owned()
    });
    let found = dates::find_dates(&text);
    let at = if editing {
        found.into_iter().find(|d| c.at as usize >= d.start && c.at as usize <= d.end)
    } else {
        found.into_iter().next()
    };
    if let Some(d) = at {
        open(&c.id, c.note, d.start as u32, d.end as u32, Some(d.spec), false, false);
        return;
    }
    let u: Vec<u16> = text.encode_utf16().collect();
    let mut caret = c.at as usize;
    // don't split a word: insert after it
    while caret < u.len() && !is_ws(u.get(caret).copied()) && !is_ws(caret.checked_sub(1).and_then(|i| u.get(i).copied())) {
        caret += 1;
    }
    open(&c.id, c.note, caret as u32, caret as u32, None, false, true);
}

/// A press on a date chip (`openDateChip`): the picker on the date at `s..e` of `id`'s content (or note), under
/// the chip (`chip`: its left, top and bottom in the screen).
pub fn open_chip(id: &str, note: bool, s: u32, e: u32, chip: (f32, f32, f32)) {
    let text = field_text(id, note);
    let found = dates::find_dates(&text).into_iter().find(|d| d.start as u32 == s || (d.start as u32) < e && s < d.end as u32);
    let Some(d) = found else { return };
    CARET_BOX.with(|c| *c.borrow_mut() = Some((id.to_owned(), note, chip.0, chip.1, chip.2)));
    open(id, note, d.start as u32, d.end as u32, Some(d.spec), false, false);
}

/// Typing: `old` became `new` in `id`'s content or note. A `!` typed after a space (or at the start) opens the
/// picker to replace it.
pub fn typed(id: &str, note: bool, old: &str, new: &str) {
    let (a, b): (Vec<u16>, Vec<u16>) = (old.encode_utf16().collect(), new.encode_utf16().collect());
    if b.len() != a.len() + 1 {
        return;
    }
    let i = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len());
    if b[i] != u16::from(b'!') || b[i + 1..] != a[i..] {
        return;
    }
    if is_ws(i.checked_sub(1).map(|k| b[k])) {
        open(id, note, i as u32, i as u32 + 1, None, true, false);
    }
}

fn close() {
    picker_state().update(|p| p.open = false);
}

/// Cancel (Escape): the caret back where it was.
fn cancel() {
    let p = picker_state().get_untracked();
    close();
    let at = if p.typed || p.insert { p.e } else { p.s };
    put_caret(&p.id, p.note, at);
}

fn put_caret(id: &str, note: bool, at: u32) {
    store::edit(|s| {
        if s.tree().contains(id) {
            s.editing = Some(Caret { note, ..Caret::text(id, at) });
            s.block = None;
            s.caret_rev += 1;
        }
    });
}

fn field_text(id: &str, note: bool) -> String {
    store::peek(|s| {
        let n = s.tree().get(id);
        if note { item::note(n) } else { item::content(n) }.to_owned()
    })
}

/// Done (Return): the date written in place of `s..e`, the caret after it; or the time's error.
fn done() {
    let p = picker_state().get_untracked();
    let typed = typed_state().get_untracked();
    let now = store::now();
    let st = parse_time(&typed.time, now);
    let et = if p.range { parse_time(&typed.end_time, now) } else { Some(None) };
    let (Some(st), Some(et)) = (st, et) else {
        picker_state().update(|p| p.error = Some(TIME_ERROR.into()));
        return;
    };
    let with_time = |d: Day, t: Option<(i64, i64)>| DateTime {
        year: d.year,
        month: d.month,
        day: d.day,
        hour: t.map(|t| t.0),
        minute: t.map(|t| t.1),
        tz: None,
    };
    let (mut a, mut b) = (p.start, p.end);
    if b < a {
        std::mem::swap(&mut a, &mut b);
    }
    let spec = DateSpec {
        start: with_time(a, st),
        end: p.range.then(|| with_time(b, et)),
        recur: p.recur.clone().map(|r| Recurrence { from_completion: p.from_completion, ..r }),
    };
    close();
    let text = field_text(&p.id, p.note);
    let u: Vec<u16> = text.encode_utf16().collect();
    // "Encode timezone in new dates": a date with a time gets the device's zone (`withLocalTz`)
    let spec = if crate::settings::get().flag("show_timezone") { dates::with_local_tz(&spec) } else { spec };
    let mut token = format_date_token(&spec);
    if p.insert {
        if p.s > 0 && !is_ws(u.get(p.s as usize - 1).copied()) {
            token.insert(0, ' ');
        }
        if (p.e as usize) < u.len() && !is_ws(u.get(p.e as usize).copied()) {
            token.push(' ');
        }
    }
    let after = p.s + cmd::utf16_len(&token);
    cmd::replace(&p.id, p.note, p.s, p.e, &token, after);
}

/// Delete date: the date and one space beside it removed.
fn remove() {
    let p = picker_state().get_untracked();
    close();
    let text = field_text(&p.id, p.note);
    let u: Vec<u16> = text.encode_utf16().collect();
    let at = |i: i64| usize::try_from(i).ok().and_then(|i| u.get(i).copied());
    let (mut s, mut e) = (p.s, p.e);
    if at(i64::from(e)) == Some(u16::from(b' ')) && is_ws(at(i64::from(s) - 1)) {
        e += 1;
    } else if s > 0 && at(i64::from(s) - 1) == Some(u16::from(b' ')) && (e as usize >= u.len() || is_ws(at(i64::from(e)))) {
        s -= 1;
    }
    cmd::replace(&p.id, p.note, s, e, "", s);
}

fn click_day(d: Day) {
    picker_state().update(|p| {
        if p.range && p.picking_end {
            if d < p.start {
                p.start = d;
            } else {
                p.end = d;
                p.picking_end = false;
            }
        } else {
            p.start = d;
            if p.end < d || !p.range {
                p.end = d;
            }
            if p.range {
                p.picking_end = true;
            }
        }
        if d.month != p.view.month || d.year != p.view.year {
            p.view = d.first();
        }
    });
}

fn move_selected(n: i64) {
    picker_state().update(|p| {
        if p.range && p.picking_end {
            p.end = p.end.add(n).max(p.start);
        } else {
            let span = (days_of(p.end) - days_of(p.start)).max(0);
            p.start = p.start.add(n);
            p.end = p.start.add(span);
        }
        p.view = if p.range && p.picking_end { p.end } else { p.start }.first();
    });
}

fn move_month(n: i64) {
    picker_state().update(|p| {
        if p.range && p.picking_end {
            p.end = p.end.add_months(n);
        } else {
            p.start = p.start.add_months(n);
            // without a range the end goes along (the web's PageDown leaves it, and Done then swaps it in as the
            // start when it is earlier: a web bug, not copied)
            if !p.range {
                p.end = p.start;
            }
        }
        p.view = if p.range && p.picking_end { p.end } else { p.start }.first();
    });
}

/// The repeat's presets: key, label, rule.
fn presets(s: Day) -> Vec<(&'static str, String, Option<Recurrence>)> {
    let r = |unit| Some(Recurrence { n: 1, unit, weekdays: Vec::new(), from_completion: false });
    vec![
        ("", "Does not repeat".into(), None),
        ("d", "Daily".into(), r(RecurUnit::D)),
        ("w", format!("Weekly on {}", WEEKDAY_NAMES[s.weekday() as usize]), r(RecurUnit::W)),
        ("m", format!("Monthly on the {}", ordinal(s.day)), r(RecurUnit::M)),
        ("y", format!("Annually on {} {}", MONTH_NAMES[s.month as usize - 1], s.day), r(RecurUnit::Y)),
    ]
}

/// Which preset the rule is ("cur": none of them).
fn repeat_value(r: Option<&Recurrence>, start: Day) -> &'static str {
    let Some(r) = r else { return "" };
    if r.n != 1 {
        return "cur";
    }
    match r.unit {
        RecurUnit::D => "d",
        RecurUnit::W if r.weekdays.is_empty() || r.weekdays == [start.iso()] => "w",
        RecurUnit::W => "cur",
        RecurUnit::M => "m",
        RecurUnit::Y => "y",
    }
}

fn finish_custom() {
    let typed_n = typed_state().with_untracked(|t| t.n.clone());
    picker_state().update(|p| {
        let Some(c) = p.custom.clone() else { return };
        let n = typed_n.trim().parse::<i64>().ok().filter(|n| *n >= 1);
        let Some(n) = n else {
            if let Some(c) = p.custom.as_mut() {
                c.error = true;
            }
            return;
        };
        let mut wd = if c.unit == RecurUnit::W { c.weekdays.clone() } else { Vec::new() };
        wd.sort_unstable();
        // a single weekday equal to the start's is the same as none
        if wd.len() == 1 && wd[0] == p.start.iso() {
            wd.clear();
        }
        p.recur = Some(Recurrence { n, unit: c.unit, weekdays: wd, from_completion: false });
        p.custom = None;
    });
}

// ---- views ----------------------------------------------------------------------------------------------

fn ui_text(t: impl IntoReactive<SharedString>, id: &str) -> Text {
    let (size, line) = style::ui_sized(id);
    text(t).font_family(style::font()).text_size(px(size)).min_h(px(line)).text_color(color::fg())
}

fn small_button(label: impl Into<String>, on: bool, run: impl Fn() + 'static) -> impl IntoElement {
    let label = label.into();
    button()
        .accessibility_label(Some(label.clone().into()))
        .selected(on)
        .px(px(num("space.input_x")))
        .py(px(num("space.input_y")))
        .bg(if on { color::sel() } else { color::bg() })
        .border(px(num("space.border")))
        .border_color(if on { color::accent() } else { color::chip_border() })
        .rounded(px(num("radius.block")))
        .on_press(run)
        .child(ui_text(label, "type.ui_small"))
}

fn foot_button(label: &'static str, kind: u8, run: impl Fn() + 'static) -> impl IntoElement {
    // 0 plain, 1 primary (Done), 2 danger (Delete date)
    let danger = rgba(style::look().color("color.danger"));
    let (bg, fg, border) = match kind {
        1 => (color::accent(), rgba(style::look().color("color.on_accent")), color::accent()),
        2 => (color::bg(), danger, color::chip_border()),
        _ => (color::bg(), color::fg(), color::chip_border()),
    };
    button()
        .accessibility_label(Some(label.into()))
        .px(px(step(7)))
        .py(px(step(4)))
        .bg(bg)
        .border(px(num("space.border")))
        .border_color(border)
        .rounded(px(num("radius.block")))
        .on_press(run)
        .child(ui_text(label, "type.ui_small").text_color(fg))
}

fn check_row(label: &'static str, on: bool, enabled: bool, flip: impl Fn() + 'static) -> impl IntoElement {
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
    button()
        .accessibility_label(Some(label.into()))
        .selected(on)
        .flex_row()
        .items_center()
        .gap(px(step(5)))
        .on_press(move || {
            if enabled {
                flip();
            }
        })
        .child(boxed)
        .child(ui_text(label, "type.ui_small").text_color(if enabled { color::fg() } else { color::faint() }))
}

fn time_field(test_id: &'static str, end: bool) -> impl IntoElement {
    let value = typed_state().with_untracked(|x| if end { x.end_time.clone() } else { x.time.clone() });
    let (size, line) = style::ui_sized("type.ui_small");
    input()
        .test_id(test_id)
        .value(value)
        .placeholder("All day")
        .auto_correct(false)
        .flex_1()
        .font_family(style::font())
        .text_size(px(size))
        .line_height(nsh_core::LineHeight::Points(px(line)))
        .text_color(color::fg())
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.block")))
        .px(px(step(5)))
        .py(px(step(4)))
        .on_change(move |t: &str| {
            let t = t.to_owned();
            typed_state().update(move |x| if end { x.end_time = t } else { x.time = t });
            if picker_state().with_untracked(|p| p.error.is_some()) {
                picker_state().update(|p| p.error = None);
            }
        })
        .on_blur(move || {
            // an error shows as it is left (`normalizeTime`)
            let v = typed_state().with_untracked(|x| if end { x.end_time.clone() } else { x.time.clone() });
            if parse_time(&v, store::now()).is_none() && picker_state().with_untracked(|p| p.open) {
                picker_state().update(|p| p.error = Some(TIME_ERROR.into()));
            }
        })
}

fn row(label: &'static str, child: impl IntoElement) -> impl IntoElement {
    row_with(label, num("space.dp_label"), child)
}

/// A field's row with its label `w` wide (`.dp-custom .dp-row`: wider).
fn row_with(label: &'static str, w: f32, child: impl IntoElement) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap(px(step(6)))
        .child(ui_text(label, "type.ui_small").text_color(color::muted()).w(px(w)))
        .child(child)
}

/// The calendar: the month's title between its arrows, the weekdays, six weeks of days.
fn calendar(p: &Picker) -> View {
    let v = p.view;
    let today = Day::today();
    let day_h = num(if style::phone() { "space.dp_day_phone" } else { "space.dp_day" });
    let nav = |d: i64, label: &'static str| {
        button()
            .accessibility_label(Some(label.into()))
            .w(px(step(12)))
            .h(px(num("space.dp_nav_h")))
            .rounded(px(num("radius.block")))
            .items_center()
            .justify_center()
            .on_press(move || picker_state().update(|p| p.view = p.view.add_months(d)))
            .child(text(if d < 0 { "‹" } else { "›" }).text_size(px(num("type.dp_nav"))).text_color(color::muted()))
    };
    let head = view()
        .flex_row()
        .items_center()
        .justify_between()
        .mb(px(step(4)))
        .child(nav(-1, "Previous month"))
        .child(
            ui_text(format!("{} {}", MONTH_NAMES[v.month as usize - 1], v.year), "type.ui")
                .font_weight(FontWeight::SEMIBOLD)
                .test_id("dp-title"),
        )
        .child(nav(1, "Next month"));
    let sd = crate::settings::get().start_day().rem_euclid(7);
    let week = |cells: Vec<AnyElement>| view().flex_row().gap(px(step(2))).children(cells);
    let names = (0..7)
        .map(|i| {
            let name = WEEKDAY_NAMES[((sd + i) % 7) as usize];
            view()
                .flex_1()
                .items_center()
                .py(px(step(2)))
                .child(ui_text(&name[..2], "type.ui_micro").text_color(color::muted()))
                .into_element()
        })
        .collect();
    let lead = (v.weekday() - sd).rem_euclid(7);
    let first = v.add(-lead);
    let weeks = (0..6).map(|w| {
        week(
            (0..7)
                .map(|k| {
                    let d = first.add(w * 7 + k);
                    let other = d.month != v.month;
                    let sel = d == p.start || (p.range && d == p.end);
                    let in_range = p.range && d > p.start && d < p.end;
                    let mut fg = if other { color::faint() } else { color::fg() };
                    let mut weight = FontWeight::NORMAL;
                    if d == today {
                        fg = color::accent();
                        weight = FontWeight(650);
                    }
                    let mut b = button()
                        .accessibility_label(Some(format!("{}-{:02}-{:02}", d.year, d.month, d.day).into()))
                        .selected(sel)
                        .flex_1()
                        .h(px(day_h))
                        .items_center()
                        .justify_center()
                        .rounded(px(day_h / 2.0))
                        .on_press(move || click_day(d));
                    if sel {
                        b = b.bg(color::accent());
                        fg = rgba(style::look().color("color.on_accent"));
                        weight = FontWeight::SEMIBOLD;
                    } else if in_range {
                        let mut c = color::accent();
                        c.a *= IN_RANGE_MIX;
                        b = b.bg(c).rounded(px(num("radius.code")));
                    }
                    b.child(ui_text(d.day.to_string(), "type.ui_small").text_color(fg).font_weight(weight).tabular_nums())
                        .into_element()
                })
                .collect(),
        )
        .into_element()
    });
    view().child(head).child(week(names)).child(view().flex_col().gap(px(step(2))).children(weeks))
}

fn summary(p: &Picker) -> String {
    let fmt = |x: Day| {
        format!(
            "{}, {} {}",
            &WEEKDAY_NAMES[x.weekday() as usize][..3],
            &MONTH_NAMES[x.month as usize - 1][..3],
            x.day
        )
    };
    if p.range {
        let pick = if p.picking_end { " · pick the end date" } else { "" };
        format!("{} – {}{pick}", fmt(p.start), fmt(p.end))
    } else if p.start.year != Day::today().year {
        format!("{} {}", fmt(p.start), p.start.year)
    } else {
        fmt(p.start)
    }
}

/// The repeat's choices (the web's `<select>`, its options in view): the presets, the current custom rule, and
/// Custom… which shows the custom rule's fields.
fn repeat(p: &Picker) -> View {
    let val = repeat_value(p.recur.as_ref(), p.start);
    let mut opts: Vec<AnyElement> = presets(p.start)
        .into_iter()
        .map(|(k, label, rule)| {
            small_button(label, k == val, move || {
                let rule = rule.clone();
                picker_state().update(move |p| {
                    p.recur = rule;
                    p.custom = None;
                })
            })
            .into_element()
        })
        .collect();
    if val == "cur" {
        if let Some(r) = &p.recur {
            let label = describe_recurrence(r);
            let mut cs = label.chars();
            let label: String = cs.next().map(|c| c.to_uppercase().chain(cs).collect()).unwrap_or_default();
            opts.push(small_button(label, true, || {}).into_element());
        }
    }
    let start = p.start;
    opts.push(
        small_button("Custom…", p.custom.is_some(), move || {
            picker_state().update(|p| {
                let cur = p.recur.clone().unwrap_or(Recurrence {
                    n: 1,
                    unit: RecurUnit::W,
                    weekdays: vec![start.iso()],
                    from_completion: false,
                });
                let weekdays =
                    if cur.unit == RecurUnit::W && !cur.weekdays.is_empty() { cur.weekdays } else { vec![start.iso()] };
                typed_state().update(|t| t.n = cur.n.to_string());
                p.custom = Some(Custom { unit: cur.unit, weekdays, error: false });
            })
        })
        .into_element(),
    );
    view().flex_row().flex_wrap().gap(px(step(4))).flex_1().children(opts)
}

fn custom(c: &Custom) -> View {
    let (size, line) = style::ui_sized("type.ui_small");
    let n = input()
        .test_id("dp-n")
        .value(typed_state().with_untracked(|t| t.n.clone()))
        .keyboard_type(KeyboardType::Number)
        .w(px(num("space.dp_every")))
        .font_family(style::font())
        .text_size(px(size))
        .line_height(nsh_core::LineHeight::Points(px(line)))
        .text_color(color::fg())
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.block")))
        .px(px(step(5)))
        .py(px(step(4)))
        .on_change(|t: &str| {
            let t = t.to_owned();
            typed_state().update(move |x| x.n = t);
        });
    let units = [(RecurUnit::D, "Day"), (RecurUnit::W, "Week"), (RecurUnit::M, "Month"), (RecurUnit::Y, "Year")];
    let unit_row = view().flex_row().flex_wrap().gap(px(step(4))).children(units.into_iter().map(|(u, l)| {
        small_button(l, c.unit == u, move || {
            picker_state().update(move |p| {
                if let Some(c) = p.custom.as_mut() {
                    c.unit = u;
                }
            })
        })
        .into_element()
    }));
    let mut body = view()
        .test_id("dp-custom")
        .flex_col()
        .gap(px(step(5)))
        .child(row_with("Repeat every", num("space.dp_custom_label"), view().flex_row().gap(px(step(5))).child(n).child(unit_row)));
    if c.unit == RecurUnit::W {
        let days = view().flex_row().flex_wrap().gap(px(step(4))).children((1..=7).map(|d| {
            let on = c.weekdays.contains(&d);
            let letter = &"MTWTFSS"[d as usize - 1..d as usize];
            button()
                .accessibility_label(Some(WEEKDAY_NAMES[(d % 7) as usize].into()))
                .selected(on)
                .w(px(step(12)))
                .h(px(step(12)))
                .rounded(px(step(12) / 2.0))
                .items_center()
                .justify_center()
                .border(px(num("space.border")))
                .border_color(if on { color::accent() } else { color::chip_border() })
                .bg(if on { color::accent() } else { color::bg() })
                .on_press(move || {
                    picker_state().update(move |p| {
                        if let Some(c) = p.custom.as_mut() {
                            if let Some(i) = c.weekdays.iter().position(|x| *x == d) {
                                c.weekdays.remove(i);
                            } else {
                                c.weekdays.push(d);
                            }
                        }
                    })
                })
                .child(ui_text(letter, "type.ui_tiny").text_color(if on {
                    rgba(style::look().color("color.on_accent"))
                } else {
                    color::fg()
                }))
                .into_element()
        }));
        body = body.child(row_with("Repeat on", num("space.dp_custom_label"), days));
    }
    if c.error {
        body = body.child(ui_text(COUNT_ERROR, "type.ui_tiny").text_color(rgba(style::look().color("color.danger"))));
    }
    body.child(
        view()
            .flex_row()
            .gap(px(step(5)))
            .justify_end()
            .child(foot_button("Cancel", 0, || picker_state().update(|p| p.custom = None)))
            .child(foot_button("Set repeat", 1, finish_custom)),
    )
}

fn body(p: &Picker) -> View {
    let fields = view()
        .flex_col()
        .gap(px(step(5)))
        .mt(px(step(5)))
        .pt(px(step(6)))
        .border_t(px(num("space.border")))
        .border_color(color::line())
        .child(row("Time", time_field("dp-time", false)));
    let mut fields = match &p.error {
        Some(e) => fields.child(
            ui_text(e.clone(), "type.ui_tiny").test_id("dp-error").text_color(rgba(style::look().color("color.danger"))),
        ),
        None => fields,
    };
    fields = fields.child(check_row("Create date range", p.range, true, || {
        picker_state().update(|p| {
            p.range = !p.range;
            p.picking_end = p.range;
            if p.end < p.start {
                p.end = p.start;
            }
        })
    }));
    if p.range {
        fields = fields.child(row("End time", time_field("dp-end-time", true)));
    }
    fields = fields.child(row("Repeat", repeat(p)));
    if let Some(c) = &p.custom {
        fields = fields.child(custom(c));
    }
    fields = fields.child(check_row("From completion", p.from_completion, p.recur.is_some(), || {
        picker_state().update(|p| p.from_completion = !p.from_completion)
    }));
    let mut foot = view().flex_row().items_center().gap(px(step(5))).mt(px(step(7)));
    if p.existing {
        foot = foot.child(foot_button("Delete date", 2, remove));
    }
    foot = foot.child(view().flex_1()).child(foot_button("Cancel", 0, cancel)).child(foot_button("Done", 1, done));
    view()
        .child(calendar(p))
        .child(
            view()
                .items_center()
                .mt(px(step(5)))
                .mb(px(step(2)))
                .child(ui_text(summary(p), "type.ui_tiny").text_color(color::muted()).test_id("dp-summary")),
        )
        .child(fields)
        .child(foot)
}

/// The picker's keys while it shows (`DatePicker.handleKey`): Escape cancels and Return is Done even in its time
/// fields; the arrows move the day and Page Up / Down the month when no field has the focus.
fn keys() -> impl IntoElement {
    let always = KeyOptions { when: When::Always, ..Default::default() };
    on_key_with("escape", always, |_| cancel());
    on_key_with("enter", always, |_| {
        if picker_state().with_untracked(|p| p.custom.is_some()) {
            finish_custom();
        } else {
            done();
        }
    });
    let opts = KeyOptions { repeat: true, ..Default::default() };
    for (k, n) in [("left", -1), ("right", 1), ("up", -7), ("down", 7)] {
        on_key_with(k, opts, move |_| move_selected(n));
    }
    on_key_with("pageup", opts, |_| move_month(-1));
    on_key_with("pagedown", opts, |_| move_month(1));
    view()
}

/// Where the desktop popover goes (`placePopup`): under the caret's box `(left, top, bottom)`, or above it when
/// there is more room there; at the caret's left, kept on the screen `(width, height)`; its height `h` cut to the
/// room it has. Its left, top and the height it may take.
fn place(anchor: (f32, f32, f32), w: f32, h: f32, screen: (f32, f32)) -> (f32, f32, f32) {
    let (left, top, bottom) = anchor;
    let gap = step(4);
    let below = screen.1 - bottom - gap - gap;
    let above = top - gap - gap;
    let down = h <= below || below >= above;
    let room = (if down { below } else { above }).max(num("space.popup_room"));
    let h = h.min(room);
    let y = if down { bottom + gap } else { top - gap - h };
    let x = left.min(screen.0 - w - gap).max(gap);
    (x, y.max(gap), room)
}

/// The picker, while it is open: a sheet at the bottom on phones, over a scrim that cancels it; on desktops a
/// popover under the caret ([`place`]) or, where the caret is not known, in the middle.
pub fn picker() -> impl IntoElement {
    when(
        || picker_state().with(|p| p.open),
        || {
            let phone = style::phone();
            let anchor = picker_state().with_untracked(|p| p.anchor).filter(|_| !phone);
            // the picker's natural height, to place it (measured inside its scroll)
            let natural = signal(0.0_f32);
            let content = view()
                .on_layout(move |b: &Bounds<Points>| natural.set(b.size.height.0))
                .child(dynamic(|| picker_state().with(body).into_element()));
            let card = view()
                .test_id("datepicker")
                .bg(color::bg())
                .border(px(num("space.border")))
                .border_color(color::chip_border())
                .rounded(px(num("radius.menu")))
                .px(px(step(8)))
                .py(px(step(7)))
                .child(keys());
            let layer = view().absolute().inset(px(0.0));
            let Some(at) = anchor else {
                let card = card.child(content);
                let card = if phone { card.self_stretch() } else { card.w(px(num("space.datepicker"))) };
                let layer = layer.items_center();
                let layer = if phone { layer.justify_end() } else { layer.justify_center() };
                return layer
                    .child(view().test_id("dp-scrim").absolute().inset(px(0.0)).bg(color::scrim()).on_press(cancel))
                    .child(card)
                    .into_element();
            };
            // the popover: what is around it stays in view, a press there cancels it
            let w = num("space.datepicker");
            let pad = 2.0 * (step(7) + num("space.border"));
            let spot = move || {
                let win = window();
                place(at, w, natural.get() + pad, (win.width.0, win.height.0))
            };
            let card = card
                .absolute()
                .w(px(w))
                .left(move || px(spot().0))
                .top(move || px(spot().1))
                .max_h(move || px(spot().2))
                .child(scroll().flex_shrink(1.0).child(content));
            layer.child(view().test_id("dp-scrim").absolute().inset(px(0.0)).on_press(cancel)).child(card).into_element()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_round_trip_and_know_their_weekday() {
        for z in [-800_000, -1, 0, 1, 19_000, 20_000, 2_000_000] {
            assert_eq!(days_of(day_of(z)), z);
        }
        // 2024-06-15 was a Saturday
        let d = Day { year: 2024, month: 6, day: 15 };
        assert_eq!(d.weekday(), 6);
        assert_eq!(d.iso(), 6);
        assert_eq!(Day { year: 2024, month: 1, day: 31 }.add_months(1), Day { year: 2024, month: 2, day: 29 });
        assert_eq!(Day { year: 2024, month: 12, day: 31 }.add(1), Day { year: 2025, month: 1, day: 1 });
    }

    #[test]
    fn times_parse_as_the_web_parses_them() {
        let now = 0;
        assert_eq!(parse_time("", now), Some(None));
        assert_eq!(parse_time("All day", now), Some(None));
        assert_eq!(parse_time("22:42", now), Some(Some((22, 42))));
        assert_eq!(parse_time("9", now), Some(Some((9, 0))));
        assert_eq!(parse_time("3pm", now), Some(Some((15, 0))));
        assert_eq!(parse_time("3:30 pm", now), Some(Some((15, 30))));
        assert_eq!(parse_time("12 am", now), Some(Some((0, 0))));
        assert_eq!(parse_time("1530", now), Some(Some((15, 30))));
        assert_eq!(parse_time("9h05", now), Some(Some((9, 5))));
        assert_eq!(parse_time("noon", now), Some(Some((12, 0))));
        assert_eq!(parse_time("24", now), None);
        assert_eq!(parse_time("13pm", now), None);
        assert_eq!(parse_time("9:61", now), None);
        assert_eq!(parse_time("9:", now), None);
        assert_eq!(parse_time("soon", now), None);
    }

    #[test]
    fn a_rule_names_its_preset() {
        let start = Day { year: 2024, month: 6, day: 15 };
        let r = |n, unit, weekdays: Vec<i64>| Recurrence { n, unit, weekdays, from_completion: false };
        assert_eq!(repeat_value(None, start), "");
        assert_eq!(repeat_value(Some(&r(1, RecurUnit::W, vec![])), start), "w");
        assert_eq!(repeat_value(Some(&r(1, RecurUnit::W, vec![6])), start), "w");
        assert_eq!(repeat_value(Some(&r(1, RecurUnit::W, vec![1, 3])), start), "cur");
        assert_eq!(repeat_value(Some(&r(2, RecurUnit::D, vec![])), start), "cur");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(22), "22nd");
    }
}
