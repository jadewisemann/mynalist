//! Search query language (in-document filter, flat and global search) and a fuzzy matcher for the file/item
//! finders and the `[[` link picker. Port of `packages/core/src/search.ts`.
//!
//! Offsets (fuzzy ranges) are UTF-16 units, as in TS. Case folding is `toLowerCase` (Rust's `to_lowercase`, the
//! same Unicode mapping). Known difference: TS's `Date.parse` fallback for operator times takes every format V8
//! reads; here it takes ISO 8601 (`2026-10-22T10:00:30.5Z`, `2026-10`, `2026`) and `2026/10/22 [10:00[:30]]`,
//! `10/22/2026`, the forms people type.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use crate::dates::{
    add_days, date_range, days_in_month, fields as date_fields, js_trim, js_ws, local_ms, parse_date_time, start_of_day,
    to_date, utc_ms,
};
use crate::inline::extract_dates;
use crate::item::COLOR_NAMES;
use crate::tree::{is_falsy, NodeId, TNode, Tree, ROOT};

// ---- tokenizer -----------------------------------------------------------------

struct RawTerm {
    neg: bool,
    /// term text with quotes removed
    v: String,
    /// the whole term was a quoted phrase
    quoted: bool,
    /// operator key (lowercase, without colon) if `key:value`
    op: Option<String>,
}

const OPERATORS: &[&str] =
    &["is", "has", "color", "heading", "since", "until", "within", "edited", "created", "parent", "ancestor", "time", "in"];

fn ws(c: char) -> bool {
    js_ws(c as u32)
}

fn tokenize(q: &str) -> Vec<RawTerm> {
    let mut out = Vec::new();
    let n = q.len();
    let mut i = 0;
    let at = |i: usize| q.get(i..).and_then(|r| r.chars().next());
    let skip_ws = |i: &mut usize| {
        while let Some(c) = at(*i).filter(|&c| ws(c)) {
            *i += c.len_utf8();
        }
    };
    // at the opening quote
    let read_quoted = |i: &mut usize| -> String {
        let close = q[*i + 1..].find('"').map(|k| k + *i + 1);
        let v = q[*i + 1..close.unwrap_or(n)].to_owned();
        *i = close.map_or(n, |c| c + 1);
        v
    };
    let read_word = |i: &mut usize| -> String {
        let s = *i;
        while let Some(c) = at(*i).filter(|&c| !ws(c)) {
            *i += c.len_utf8();
        }
        q[s..*i].to_owned()
    };
    loop {
        skip_ws(&mut i);
        if i >= n {
            break;
        }
        let mut neg = false;
        if q.as_bytes()[i] == b'-' && at(i + 1).is_some_and(|c| !ws(c)) {
            neg = true;
            i += 1;
        }
        // a quote without its closing quote is just a character of the word (as in Dynalist)
        if q.as_bytes().get(i) == Some(&b'"') && q[i + 1..].contains('"') {
            out.push(RawTerm { neg, v: read_quoted(&mut i), quoted: true, op: None });
            continue;
        }
        let letters = q.as_bytes()[i..].iter().take_while(|c| c.is_ascii_alphabetic()).count();
        if letters > 0 && q.as_bytes().get(i + letters) == Some(&b':') {
            let op = q[i..i + letters].to_ascii_lowercase();
            if OPERATORS.contains(&op.as_str()) {
                i += letters + 1;
                // "op: value" joins with the next word
                if at(i).is_none_or(ws) {
                    skip_ws(&mut i);
                }
                let v = if q.as_bytes().get(i) == Some(&b'"') {
                    read_quoted(&mut i)
                } else if i < n {
                    read_word(&mut i)
                } else {
                    String::new()
                };
                out.push(RawTerm { neg, v, quoted: false, op: Some(op) });
                continue;
            }
        }
        out.push(RawTerm { neg, v: read_word(&mut i), quoted: false, op: None });
    }
    out
}

// ---- time values -----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RelTime {
    pub n: i64,
    /// `d`, `w`, `m` or `h`
    pub unit: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TimeRange {
    /// inclusive
    pub start: i64,
    /// exclusive
    pub end: i64,
    /// relative amount (`4d`, `-2w`, `3h`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel: Option<RelTime>,
}

fn add_months_ms(t: i64, n: i64) -> i64 {
    let d = date_fields(t);
    let target = date_fields(local_ms(d.year, d.month0 + n, 1, 0, 0, 0, 0));
    let dim = days_in_month(target.year, target.month0 + 1);
    local_ms(target.year, target.month0, d.day.min(dim), 0, 0, 0, 0)
}

fn num(b: &[u8]) -> Option<i64> {
    (!b.is_empty() && b.iter().all(u8::is_ascii_digit)).then(|| b.iter().fold(0i64, |a, d| a.saturating_mul(10).saturating_add(i64::from(d - b'0'))))
}

/// `Date.parse` for ISO 8601 and the slash forms (see the module doc); `None` for anything else.
pub fn parse_instant(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    // YYYY/M/D[ H:mm[:ss]] and M/D/YYYY: local time
    let parts: Vec<&[u8]> = b.split(|&c| c == b'/').collect();
    if parts.len() == 3 {
        let (date3, time) = match parts[2].iter().position(|&c| c == b' ') {
            Some(k) => (&parts[2][..k], Some(&parts[2][k + 1..])),
            None => (parts[2], None),
        };
        let (y, m, d) = if parts[0].len() == 4 && (1..=2).contains(&parts[1].len()) && (1..=2).contains(&date3.len()) {
            (num(parts[0])?, num(parts[1])?, num(date3)?)
        } else if time.is_none() && (1..=2).contains(&parts[0].len()) && (1..=2).contains(&parts[1].len()) && date3.len() == 4 {
            (num(date3)?, num(parts[0])?, num(parts[1])?)
        } else {
            return None;
        };
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        let (h, mi, sec) = match time {
            None => (0, 0, 0),
            Some(t) => {
                let tp: Vec<&[u8]> = t.split(|&c| c == b':').collect();
                if !(2..=3).contains(&tp.len()) || !(1..=2).contains(&tp[0].len()) || tp[1..].iter().any(|p| p.len() != 2) {
                    return None;
                }
                let h = num(tp[0])?;
                let mi = num(tp[1])?;
                let sec = if tp.len() == 3 { num(tp[2])? } else { 0 };
                if h > 23 || mi > 59 || sec > 59 {
                    return None;
                }
                (h, mi, sec)
            }
        };
        return Some(local_ms(y, m - 1, d, h, mi, sec, 0));
    }
    // ISO: ([+-]YYYYYY|YYYY)(-MM(-DD)?)?(THH:mm(:ss(.s+)?)?(Z|[+-]HH:mm)?)?
    let (date, time) = match b.iter().position(|&c| c == b'T') {
        Some(k) => (&b[..k], Some(&b[k + 1..])),
        None => (b, None),
    };
    let (year, rest) = match date.first() {
        Some(&sign @ (b'+' | b'-')) => {
            let y = num(date.get(1..7)?)?;
            if sign == b'-' && y == 0 {
                return None;
            }
            (if sign == b'-' { -y } else { y }, &date[7..])
        }
        _ => (num(date.get(..4)?)?, &date[4..]),
    };
    let (month, day) = match rest {
        [] => (1, 1),
        [b'-', m @ ..] if m.len() == 2 => (num(m)?, 1),
        [b'-', m1, m2, b'-', d @ ..] if d.len() == 2 => (num(&[*m1, *m2])?, num(d)?),
        _ => return None,
    };
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(if (0..=99).contains(&year) { year + 400 } else { year }, month) {
        return None;
    }
    let Some(time) = time else {
        return Some(iso_utc(year, month, day, 0, 0, 0, 0));
    };
    let tz_at = time.iter().position(|&c| c == b'Z' || c == b'+' || c == b'-').unwrap_or(time.len());
    let (clock, tz) = time.split_at(tz_at);
    let (hms, frac) = match clock.iter().position(|&c| c == b'.') {
        Some(k) => (&clock[..k], Some(&clock[k + 1..])),
        None => (clock, None),
    };
    let tp: Vec<&[u8]> = hms.split(|&c| c == b':').collect();
    if !(2..=3).contains(&tp.len()) || tp.iter().any(|p| p.len() != 2) || (frac.is_some() && tp.len() != 3) {
        return None;
    }
    let (h, mi) = (num(tp[0])?, num(tp[1])?);
    let sec = if tp.len() == 3 { num(tp[2])? } else { 0 };
    let ms = match frac {
        Some(f) => {
            num(f)?;
            let mut d = f.iter().take(3).fold(0, |a, c| a * 10 + i64::from(c - b'0'));
            for _ in f.len().min(3)..3 {
                d *= 10;
            }
            d
        }
        None => 0,
    };
    if h > 24 || mi > 59 || sec > 59 || (h == 24 && (mi, sec, ms) != (0, 0, 0)) {
        return None;
    }
    let off = match tz {
        [] => return Some(local_ms(year, month - 1, day, h, mi, sec, ms)),
        [b'Z'] => 0,
        [s @ (b'+' | b'-'), h1, h2, b':', m1, m2] => {
            let (oh, om) = (num(&[*h1, *h2])?, num(&[*m1, *m2])?);
            if oh > 23 || om > 59 {
                return None;
            }
            (if *s == b'-' { -1 } else { 1 }) * (oh * 60 + om)
        }
        _ => return None,
    };
    Some(iso_utc(year, month, day, h, mi, sec, ms) - off * 60_000)
}

/// UTC time of a full year (no 0..99 → 1900s mapping: ISO years are years).
fn iso_utc(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64, ms: i64) -> i64 {
    // utc_ms maps years 0..99 to 1900..1999; shift by 400 years (same calendar) and back
    let shift = if (0..=99).contains(&y) { 400 } else { 0 };
    let base = utc_ms(y + shift, m - 1, d, h, mi) + s * 1000 + ms;
    base - if shift > 0 { utc_ms(2000, 0, 1, 0, 0) - utc_ms(1600, 0, 1, 0, 0) } else { 0 }
}

/// Parse an operator time value: now, today, yesterday, tomorrow, `<n>d|w|m|h` (negative = past; days/weeks/
/// months snap to local midnight and cover that whole day), `YYYY-MM-DD` (whole day), or a date/time (instant).
pub fn parse_query_time(x: &str, now: i64) -> Option<TimeRange> {
    let t = now;
    let today = start_of_day(t);
    let day = |d: i64| TimeRange { start: d, end: add_days(d, 1), rel: None };
    let v = js_trim(x).to_lowercase();
    match v.as_str() {
        "" => return None,
        "now" => return Some(TimeRange { start: t, end: t + 1, rel: None }),
        "today" => return Some(day(today)),
        "yesterday" => return Some(day(add_days(today, -1))),
        "tomorrow" => return Some(day(add_days(today, 1))),
        _ => {}
    }
    let vb = v.as_bytes();
    if let Some((&unit, body)) = vb.split_last().filter(|(u, _)| b"dwmh".contains(u)) {
        let (sign, digits) = match body.first() {
            Some(b'-') => (-1, &body[1..]),
            Some(b'+') => (1, &body[1..]),
            _ => (1, body),
        };
        if let Some(n) = num(digits) {
            let n = sign * n;
            let rel = Some(RelTime { n, unit: unit as char });
            if unit == b'h' {
                let at = t + n * 3_600_000;
                return Some(TimeRange { start: at, end: at + 1, rel });
            }
            let d = match unit {
                b'd' => add_days(today, n),
                b'w' => add_days(today, 7 * n),
                _ => add_months_ms(today, n),
            };
            return Some(TimeRange { rel, ..day(d) });
        }
    }
    if let Some(dt) = parse_date_time(js_trim(x)) {
        let s = to_date(&dt);
        return Some(if dt.hour.is_none() { day(s) } else { TimeRange { start: s, end: s + 1, rel: None } });
    }
    if v.bytes().any(|c| c.is_ascii_digit()) {
        if let Some(p) = parse_instant(x) {
            return Some(TimeRange { start: p, end: p + 1, rel: None });
        }
    }
    None
}

/// Threshold for edited:/created: ("within the past X").
fn past_threshold(r: &TimeRange, now: i64) -> i64 {
    let Some(rel) = r.rel else { return r.start };
    let n = -rel.n.abs();
    let today = start_of_day(now);
    match rel.unit {
        'h' => now + n * 3_600_000,
        'd' => add_days(today, n),
        'w' => add_days(today, 7 * n),
        _ => add_months_ms(today, n),
    }
}

// ---- matching helpers -----------------------------------------------------------

fn gc_of(u: u16) -> Option<GeneralCategoryGroup> {
    char::from_u32(u32::from(u)).map(|c| c.general_category_group())
}

/// `/[\p{L}\p{N}_-]/u` on one UTF-16 unit (a lone surrogate is not a word character).
fn is_word_unit(u: Option<u16>) -> bool {
    let Some(u) = u else { return false };
    u == u16::from(b'_') || u == u16::from(b'-') || matches!(gc_of(u), Some(GeneralCategoryGroup::Letter | GeneralCategoryGroup::Number))
}

fn is_cat(u: u16, cat: unicode_properties::GeneralCategory) -> bool {
    char::from_u32(u32::from(u)).is_some_and(|c| c.general_category() == cat)
}

fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn index_of(h: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.len() > h.len() {
        return None;
    }
    (from..=h.len() - needle.len()).find(|&k| h[k..k + needle.len()] == *needle)
}

/// Whole-word occurrence of `needle` in `hay` (boundaries = whitespace/punctuation).
pub fn whole_word_match(hay: &str, needle: &str, case_sensitive: bool, prefix: bool) -> bool {
    if needle.is_empty() {
        return true;
    }
    let hay16 = u16s(hay);
    let h = if case_sensitive { hay16.clone() } else { u16s(&hay.to_lowercase()) };
    let nd = if case_sensitive { u16s(needle) } else { u16s(&needle.to_lowercase()) };
    let mut from = 0;
    while let Some(i) = index_of(&h, &nd, from) {
        let before = i == 0 || !is_word_unit(hay16.get(i - 1).copied());
        let j = i + nd.len();
        let after = prefix || j >= hay16.len() || !is_word_unit(Some(hay16[j]));
        if before && after {
            return true;
        }
        from = i + 1;
    }
    false
}

fn prop_str<'a>(n: Option<&'a TNode>, k: &str) -> &'a str {
    n.and_then(|n| n.props.get(k)).and_then(Value::as_str).unwrap_or("")
}

fn truthy(n: &TNode, k: &str) -> bool {
    n.props.get(k).is_some_and(|v| !is_falsy(v))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    All,
    Title,
    Note,
}

fn field_texts(n: &TNode, scope: Scope) -> Vec<&str> {
    match scope {
        Scope::Title => vec![prop_str(Some(n), "c")],
        Scope::Note => vec![prop_str(Some(n), "n")],
        Scope::All => vec![prop_str(Some(n), "c"), prop_str(Some(n), "n")],
    }
}

fn item_dates(n: &TNode) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    for src in [prop_str(Some(n), "c"), prop_str(Some(n), "n")] {
        if src.contains("!(") {
            out.extend(extract_dates(src).iter().map(|d| date_range(&d.spec)));
        }
    }
    out
}

/// Last instant covered by a date range (all-day ranges are exclusive at the end).
fn last_instant(d: (i64, i64)) -> i64 {
    if d.1 > d.0 { d.1 - 1 } else { d.1 }
}

fn pick_values(v: &str, values: &[&'static str], prefix: bool) -> Option<Vec<&'static str>> {
    let lv = v.to_lowercase();
    if let Some(x) = values.iter().find(|x| **x == lv) {
        return Some(vec![x]);
    }
    if prefix && !lv.is_empty() {
        let c: Vec<&'static str> = values.iter().copied().filter(|x| x.starts_with(&lv)).collect();
        if !c.is_empty() {
            return Some(c);
        }
    }
    None
}

// ---- compiler --------------------------------------------------------------------

type Pred = Box<dyn Fn(&Tree, &TNode) -> bool>;

const IS_VALUES: &[&str] = &["completed", "checked", "checklist", "heading"];
const HAS_VALUES: &[&str] = &["checkbox", "note", "color", "children", "date"];

fn is_completed(tree: &Tree, n: &TNode) -> bool {
    let mut cur = Some(n);
    while let Some(c) = cur {
        if truthy(c, "ck") {
            return true;
        }
        cur = c.parent.as_deref().and_then(|p| tree.get(p));
    }
    false
}

fn is_pred(v: &str) -> fn(&Tree, &TNode) -> bool {
    match v {
        "completed" | "checked" => is_completed,
        "checklist" => |_, n| truthy(n, "cb"),
        _ => |_, n| truthy(n, "h"),
    }
}

fn has_pred(v: &str) -> fn(&Tree, &TNode) -> bool {
    match v {
        "checkbox" => |_, n| truthy(n, "cb"),
        "note" => |_, n| !js_trim(prop_str(Some(n), "n")).is_empty(),
        "color" => |_, n| truthy(n, "cl"),
        "children" => |_, n| !n.children.is_empty(),
        _ => |_, n| !item_dates(n).is_empty(),
    }
}

fn any_of(preds: Vec<fn(&Tree, &TNode) -> bool>) -> Pred {
    Box::new(move |t, n| preds.iter().any(|p| p(t, n)))
}

/// A number prop compared with `===` after `?? 0`: absent/null is 0, a non-number matches nothing.
fn num_prop(n: &TNode, k: &str) -> f64 {
    match n.props.get(k) {
        None | Some(Value::Null) => 0.0,
        Some(Value::Number(x)) => x.as_f64().unwrap_or(f64::NAN),
        Some(_) => f64::NAN,
    }
}

struct Compiled {
    pred: Option<Pred>,
    /// term text to highlight
    hl: Option<String>,
    forces_checked: bool,
}

fn compiled(pred: Pred) -> Compiled {
    Compiled { pred: Some(pred), hl: None, forces_checked: false }
}

fn contains_lower(fields: &[&str], lv: &str) -> bool {
    fields.iter().any(|f| f.to_lowercase().contains(lv))
}

fn compile_term(term: &RawTerm, scope: Scope, prefix: bool, now: i64) -> Compiled {
    let none = || Compiled { pred: None, hl: None, forces_checked: false };
    let Some(op) = term.op.as_deref() else {
        let v = term.v.clone();
        if v.is_empty() {
            return none();
        }
        let hl = Some(v.clone());
        if term.quoted {
            let pred: Pred = Box::new(move |_, n| field_texts(n, scope).iter().any(|f| whole_word_match(f, &v, true, prefix)));
            return Compiled { pred: Some(pred), hl, forces_checked: false };
        }
        if v.starts_with('#') || v.starts_with('@') {
            let pred: Pred = Box::new(move |_, n| field_texts(n, scope).iter().any(|f| whole_word_match(f, &v, false, prefix)));
            return Compiled { pred: Some(pred), hl, forces_checked: false };
        }
        let lv = v.to_lowercase();
        return Compiled { pred: Some(Box::new(move |_, n| contains_lower(&field_texts(n, scope), &lv))), hl, forces_checked: false };
    };

    let v = js_trim(&term.v).to_owned();
    let as_text = || -> Compiled {
        if prefix {
            return none(); // incomplete operator while typing: ignore
        }
        let raw = format!("{op}:{}", term.v);
        let lv = raw.to_lowercase();
        Compiled { pred: Some(Box::new(move |_, n| contains_lower(&field_texts(n, scope), &lv))), hl: Some(raw), forces_checked: false }
    };

    match op {
        "is" => {
            let Some(vals) = pick_values(&v, IS_VALUES, prefix) else { return as_text() };
            let forces = vals.len() == 1 && (vals[0] == "completed" || vals[0] == "checked");
            Compiled { pred: Some(any_of(vals.iter().map(|x| is_pred(x)).collect())), hl: None, forces_checked: forces }
        }
        "has" => {
            let Some(vals) = pick_values(&v, HAS_VALUES, prefix) else { return as_text() };
            compiled(any_of(vals.iter().map(|x| has_pred(x)).collect()))
        }
        "time" => compiled(Box::new(has_pred("date"))),
        "color" => {
            let Some(vals) = pick_values(&v, &COLOR_NAMES, prefix) else { return as_text() };
            let nums: Vec<f64> = vals.iter().map(|x| COLOR_NAMES.iter().position(|c| c == x).unwrap_or(0) as f64).collect();
            compiled(Box::new(move |_, n| nums.contains(&num_prop(n, "cl"))))
        }
        "heading" => {
            let b = v.as_bytes();
            if !(b.len() == 1 && (b'0'..=b'3').contains(&b[0])) {
                return as_text();
            }
            let h = f64::from(b[0] - b'0');
            compiled(Box::new(move |_, n| num_prop(n, "h") == h))
        }
        "parent" | "ancestor" => {
            if v.is_empty() {
                return as_text();
            }
            let lv = v.to_lowercase();
            if op == "parent" {
                return compiled(Box::new(move |t, n| {
                    n.parent.as_deref().is_some_and(|p| prop_str(t.get(p), "c").to_lowercase().contains(&lv))
                }));
            }
            compiled(Box::new(move |t, n| {
                let mut p = n.parent.as_deref().and_then(|p| t.get(p));
                while let Some(a) = p {
                    if prop_str(Some(a), "c").to_lowercase().contains(&lv) {
                        return true;
                    }
                    p = a.parent.as_deref().and_then(|x| t.get(x));
                }
                false
            }))
        }
        "since" | "until" | "within" | "edited" | "created" => {
            let Some(r) = parse_query_time(&v, now) else { return as_text() };
            match op {
                "edited" => {
                    let th = past_threshold(&r, now);
                    compiled(Box::new(move |_, n| n.edited >= th))
                }
                "created" => {
                    let th = past_threshold(&r, now);
                    compiled(Box::new(move |_, n| n.created >= th))
                }
                "since" => compiled(Box::new(move |_, n| item_dates(n).into_iter().any(|d| last_instant(d) >= r.start))),
                "until" => compiled(Box::new(move |_, n| item_dates(n).into_iter().any(|d| d.0 < r.end))),
                _ => {
                    let w = if v.to_lowercase() == "today" {
                        (r.start, r.end)
                    } else if r.end > now {
                        (now, r.end)
                    } else {
                        (r.start, add_days(start_of_day(now), 1))
                    };
                    compiled(Box::new(move |_, n| item_dates(n).into_iter().any(|d| d.0 < w.1 && last_instant(d) >= w.0)))
                }
            }
        }
        _ => as_text(),
    }
}

/// A compiled query: `matches` is the predicate; the flags are read off it.
pub struct Matcher {
    groups: Vec<Vec<Pred>>,
    /// the query contains a positive `is:completed`: checked items must be shown
    pub forces_show_checked: bool,
    /// positive text terms, for highlighting matches in rendered text
    pub terms: Vec<String>,
    /// no effective terms: everything matches
    pub empty: bool,
}

impl Matcher {
    pub fn matches(&self, tree: &Tree, id: &str) -> bool {
        if self.groups.is_empty() {
            return true;
        }
        let Some(n) = tree.get(id) else { return false };
        self.groups.iter().any(|g| g.iter().all(|p| p(tree, n)))
    }
}

/// Compile a search query. Terms are ANDed; a bare `OR` starts a new alternative group; `-term` negates;
/// `"phrase"` is a case-sensitive whole-word match; `#tag`/`@tag` match whole words; `in:title` / `in:note`
/// restrict the following terms. `typing`: the user is still typing, so the last term (if not followed by a
/// space) is a prefix match.
pub fn compile_query(q: &str, now: i64, typing: bool) -> Matcher {
    let raw = tokenize(q);
    let last_is_prefix = typing && !q.is_empty() && !q.chars().next_back().is_some_and(ws);
    let mut groups: Vec<Vec<Pred>> = vec![Vec::new()];
    let mut terms = Vec::new();
    let mut forces = false;
    let mut scope = Scope::All;
    for (idx, term) in raw.iter().enumerate() {
        if term.op.is_none() && !term.quoted && !term.neg && term.v == "OR" {
            groups.push(Vec::new());
            continue;
        }
        if term.op.as_deref() == Some("in") {
            match term.v.to_lowercase().as_str() {
                "title" | "content" => scope = Scope::Title,
                "note" | "notes" => scope = Scope::Note,
                "all" | "any" | "both" => scope = Scope::All,
                _ => {}
            }
            continue;
        }
        let c = compile_term(term, scope, last_is_prefix && idx == raw.len() - 1, now);
        let Some(p) = c.pred else { continue };
        let group = groups.last_mut().expect("a group");
        if term.neg {
            group.push(Box::new(move |t, n| !p(t, n)));
        } else {
            group.push(p);
            terms.extend(c.hl);
            forces |= c.forces_checked;
        }
    }
    groups.retain(|g| !g.is_empty());
    let empty = groups.is_empty();
    Matcher { groups, forces_show_checked: forces, terms, empty }
}

/// True if the query contains a positive `is:completed`.
pub fn forces_show_checked(q: &str) -> bool {
    compile_query(q, 0, false).forces_show_checked
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchResult {
    /// matching items in document order
    pub matches: Vec<NodeId>,
    /// matches plus their ancestors below `scope` (for `View::filter`)
    pub visible: HashSet<NodeId>,
}

/// Run a matcher over the subtree under `scope` (exclusive; the root by default). With `show_checked` false (and
/// the query not forcing it), checked items and their descendants are skipped.
pub fn search_tree(tree: &Tree, matcher: &Matcher, scope: Option<&str>, show_checked: Option<bool>) -> SearchResult {
    let mut res = SearchResult::default();
    let hide_checked = show_checked == Some(false) && !matcher.forces_show_checked;
    fn walk(tree: &Tree, m: &Matcher, hide: bool, id: &str, chain: &mut Vec<NodeId>, res: &mut SearchResult) {
        let Some(n) = tree.get(id) else { return };
        for c in &n.children {
            let Some(cn) = tree.get(c) else { continue };
            if hide && truthy(cn, "ck") {
                continue;
            }
            if m.matches(tree, c) {
                res.matches.push(c.clone());
                res.visible.insert(c.clone());
                res.visible.extend(chain.iter().cloned());
            }
            chain.push(c.clone());
            walk(tree, m, hide, c, chain, res);
            chain.pop();
        }
    }
    walk(tree, matcher, hide_checked, scope.unwrap_or(ROOT), &mut Vec::new(), &mut res);
    res
}

// ---- fuzzy matching ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FuzzyMatch {
    /// higher is better (always <= 0)
    pub score: f64,
    /// matched [start, end) ranges in the target (UTF-16), merged and sorted
    pub ranges: Vec<[usize; 2]>,
}

fn is_boundary(t: &[u16], p: usize) -> bool {
    if p == 0 {
        return true;
    }
    let prev = t[p - 1];
    if !is_word_unit(Some(prev)) {
        return true;
    }
    use unicode_properties::GeneralCategory::{LowercaseLetter, UppercaseLetter};
    is_cat(prev, LowercaseLetter) && t.get(p).is_some_and(|&c| is_cat(c, UppercaseLetter))
}

fn score_ranges(target: &[u16], ranges: &[[usize; 2]], char_mode: bool) -> FuzzyMatch {
    let mut merged: Vec<[usize; 2]> = Vec::new();
    for r in ranges {
        match merged.last_mut() {
            Some(last) if last[1] == r[0] => last[1] = r[1],
            _ => merged.push(*r),
        }
    }
    let mut misses = 0;
    let mut matched = 0;
    for &[s, e] in &merged {
        if !is_boundary(target, s) {
            misses += 1;
        }
        matched += e - s;
    }
    let first = merged.first().map_or(0, |r| r[0]);
    let last_end = merged.last().map_or(0, |r| r[1]);
    let spread = last_end as f64 - first as f64 - matched as f64;
    let penalty = merged.len() as f64 * 10.0
        + misses as f64 * 6.0
        + spread * 1.0
        + first as f64 * 0.5
        + target.len() as f64 * 0.1
        + if char_mode { 25.0 } else { 0.0 };
    FuzzyMatch { score: -penalty, ranges: merged }
}

fn token_match(t: &[u16], tokens: &[Vec<u16>], prefer_boundary: bool, target: &[u16]) -> Option<Vec<[usize; 2]>> {
    let mut out = Vec::new();
    let mut pos = 0;
    for tok in tokens {
        let mut at = index_of(t, tok, pos)?;
        if prefer_boundary {
            let mut j = Some(at);
            while let Some(k) = j {
                if is_boundary(target, k) {
                    at = k;
                    break;
                }
                j = index_of(t, tok, k + 1);
            }
        }
        out.push([at, at + tok.len()]);
        pos = at + tok.len();
    }
    Some(out)
}

/// Fuzzy match for finders: first tries the query's words in order as substrings, then falls back to matching
/// individual characters in order. Case-insensitive. `None` if no match.
pub fn fuzzy_match(query: &str, target: &str) -> Option<FuzzyMatch> {
    let lq = query.to_lowercase();
    let tokens: Vec<Vec<u16>> = lq.split(ws).filter(|s| !s.is_empty()).map(u16s).collect();
    if tokens.is_empty() {
        return Some(FuzzyMatch { score: 0.0, ranges: Vec::new() });
    }
    let target16 = u16s(target);
    let t = u16s(&target.to_lowercase());
    if t.len() != target16.len() {
        return None;
    }
    let mut best: Option<FuzzyMatch> = None;
    let consider = |best: &mut Option<FuzzyMatch>, r: Option<Vec<[usize; 2]>>, char_mode: bool| {
        if let Some(r) = r {
            let m = score_ranges(&target16, &r, char_mode);
            if best.as_ref().is_none_or(|b| m.score > b.score) {
                *best = Some(m);
            }
        }
    };
    consider(&mut best, token_match(&t, &tokens, true, &target16), false);
    consider(&mut best, token_match(&t, &tokens, false, &target16), false);
    if best.is_some() {
        return best;
    }
    let chars: Vec<u16> = tokens.concat();
    let mut s = index_of(&t, &chars[..1], 0);
    while let Some(start) = s {
        let mut r = vec![[start, start + 1]];
        let mut pos = start + 1;
        let mut ok = true;
        for &c in &chars[1..] {
            match index_of(&t, &[c], pos) {
                Some(at) => {
                    r.push([at, at + 1]);
                    pos = at + 1;
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            break;
        }
        consider(&mut best, Some(r), true);
        s = index_of(&t, &chars[..1], start + 1);
    }
    best
}

/// Fuzzy score (higher is better); -infinity when there is no match.
pub fn fuzzy_score(query: &str, target: &str) -> f64 {
    fuzzy_match(query, target).map_or(f64::NEG_INFINITY, |m| m.score)
}

/// Sort candidates by fuzzy score (best first), dropping non-matches. Stable for ties.
pub fn fuzzy_filter<'a, T>(query: &str, items: &'a [T], key: impl Fn(&T) -> &str) -> Vec<(&'a T, FuzzyMatch)> {
    let mut out: Vec<(&'a T, FuzzyMatch)> = items.iter().filter_map(|it| fuzzy_match(query, key(it)).map(|m| (it, m))).collect();
    // a stable sort keeps the input order for ties
    out.sort_by(|a, b| b.1.score.partial_cmp(&a.1.score).unwrap_or(std::cmp::Ordering::Equal));
    out
}
