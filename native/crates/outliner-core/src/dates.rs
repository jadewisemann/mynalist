//! Date syntax `!(START[ - END][ | RECURRENCE])`: parsing, formatting, relative labels, overdue checks and
//! recurrence advancement. The Rust port of `packages/core/src/dates.ts`.
//!
//! All functions are pure: anything that depends on the current time takes `now` (ms since the epoch). Calendar
//! math happens in the local time zone (chrono's `Local`: `TZ`, or the system's zone) unless an endpoint carries an
//! explicit offset (`+09:00` / `Z`). The local-time helpers below reproduce what a JS `Date` does, quirks included
//! (years 0..99 in `new Date(y, m, d)` mean 1900..1999; a time in a DST gap moves forward; a repeated time is the
//! earlier one), so labels and recurrences match the web app.

use chrono::{Local, Offset, TimeZone};
use serde::{Deserialize, Serialize};

pub const DAY: i64 = 86_400_000;
const HOUR: i64 = 3_600_000;
const MINUTE: i64 = 60_000;

/// One endpoint. All-day when `hour` is `None`. `month` is 1-12.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateTime {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hour: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minute: Option<i64>,
    /// explicit UTC offset as written (`Z`, `+09:00`, `-0500`); only with a time
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tz: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecurUnit {
    D,
    W,
    M,
    Y,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recurrence {
    /// interval, >= 1
    pub n: i64,
    pub unit: RecurUnit,
    /// ISO weekdays 1 (Mon) .. 7 (Sun), sorted, only meaningful for `W`
    pub weekdays: Vec<i64>,
    /// `~` prefix: next occurrence is computed from the completion date
    pub from_completion: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateSpec {
    pub start: DateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<DateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recur: Option<Recurrence>,
}

// ---- JS Date arithmetic --------------------------------------------------------------

/// What JS `/\s/` and `String.prototype.trim` take for white space (Rust's `char::is_whitespace` differs: it
/// takes U+0085 and not U+FEFF).
pub fn js_ws(c: u32) -> bool {
    matches!(c, 0x09..=0x0d | 0x20 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
}

/// `s.trim()` as JS trims.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| js_ws(c as u32))
}

/// JS `Math.round`: halves round up.
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Days from 1970-01-01 to a proleptic Gregorian date (H. Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date `z` days after 1970-01-01: (year, month 1-12, day).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// `new Date(y, m, …)` and `Date.UTC` read years 0..99 as 1900..1999.
fn js_year(y: i64) -> i64 {
    if (0..=99).contains(&y) { 1900 + y } else { y }
}

/// ECMAScript `MakeDay`: month `m0` is 0-based and may overflow either way, as may the day.
fn make_day(y: i64, m0: i64, d: i64) -> i64 {
    days_from_civil(y + m0.div_euclid(12), m0.rem_euclid(12) + 1, 1) + d - 1
}

/// `Date.UTC(y, m0, d, h, mi)`.
pub fn utc_ms(y: i64, m0: i64, d: i64, h: i64, mi: i64) -> i64 {
    make_day(js_year(y), m0, d) * DAY + h * HOUR + mi * MINUTE
}

/// The local zone's offset (ms, local minus UTC) at an instant.
pub fn offset_at(ms: i64) -> i64 {
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(t) => i64::from(Local.offset_from_utc_datetime(&t.naive_utc()).fix().local_minus_utc()) * 1000,
        None => 0,
    }
}

/// The offset for a local wall time `t` (ms as if it were UTC), as JS picks it: the earlier of two instants
/// when the time repeats, the offset before the change when it falls in a gap. Offsets a day before and after
/// are the candidates (zones change at most once a day); a candidate holds if the instant it gives has it.
fn offset_for_local(t: i64) -> i64 {
    let before = offset_at(t - DAY);
    let after = offset_at(t + DAY);
    let holds = |o: i64| offset_at(t - o) == o;
    match (holds(before), holds(after)) {
        (true, true) => before.max(after),
        (true, false) => before,
        (false, true) => after,
        (false, false) => before,
    }
}

/// `new Date(y, m0, d, h, mi, s, ms).getTime()`: local time, fields may overflow.
pub fn local_ms(y: i64, m0: i64, d: i64, h: i64, mi: i64, s: i64, ms: i64) -> i64 {
    let t = make_day(js_year(y), m0, d) * DAY + h * HOUR + mi * MINUTE + s * 1000 + ms;
    t - offset_for_local(t)
}

/// The local calendar fields of an instant, as a JS `Date`'s getters give them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fields {
    pub year: i64,
    /// 0-based, as `getMonth()`
    pub month0: i64,
    pub day: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub ms: i64,
    /// 0 = Sunday, as `getDay()`
    pub weekday: i64,
}

pub fn fields(t: i64) -> Fields {
    let l = t + offset_at(t);
    let days = l.div_euclid(DAY);
    let tod = l.rem_euclid(DAY);
    let (year, month, day) = civil_from_days(days);
    Fields {
        year,
        month0: month - 1,
        day,
        hours: tod / HOUR,
        minutes: tod % HOUR / MINUTE,
        seconds: tod % MINUTE / 1000,
        ms: tod % 1000,
        weekday: (days + 4).rem_euclid(7),
    }
}

// ---- parsing ------------------------------------------------------------------------

pub fn days_in_month(year: i64, month: i64) -> i64 {
    // new Date(year, month, 0).getDate()
    civil_from_days(make_day(js_year(year), month, 0)).2
}

fn digits(b: &[u8], at: usize, n: usize) -> Option<i64> {
    let s = b.get(at..at + n)?;
    s.iter().all(u8::is_ascii_digit).then(|| s.iter().fold(0, |a, d| a * 10 + i64::from(d - b'0')))
}

/// Parse `YYYY-MM-DD[ HH:mm[tz]]`.
pub fn parse_date_time(s: &str) -> Option<DateTime> {
    let b = js_trim(s).as_bytes();
    let year = digits(b, 0, 4)?;
    if b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return None;
    }
    let month = digits(b, 5, 2)?;
    let day = digits(b, 8, 2)?;
    let mut at = 10;
    let mut time = None;
    if at < b.len() {
        if !matches!(b[at], b' ' | b'T') {
            return None;
        }
        at += 1;
        let hl = if b.get(at + 1).is_some_and(u8::is_ascii_digit) { 2 } else { 1 };
        let hour = digits(b, at, hl)?;
        at += hl;
        if b.get(at) != Some(&b':') {
            return None;
        }
        let minute = digits(b, at + 1, 2)?;
        at += 3;
        let mut tz = None;
        if at < b.len() {
            let z = if b[at] == b' ' { at + 1 } else { at };
            let rest = std::str::from_utf8(&b[z..]).ok()?;
            let rb = rest.as_bytes();
            let ok = rest == "Z"
                || (matches!(rb.first(), Some(b'+' | b'-'))
                    && digits(rb, 1, 2).is_some()
                    && match rb.len() {
                        5 => digits(rb, 3, 2).is_some(),
                        6 => rb[3] == b':' && digits(rb, 4, 2).is_some(),
                        _ => false,
                    });
            if !ok {
                return None;
            }
            tz = Some(rest.to_owned());
            at = b.len();
        }
        if at != b.len() {
            return None;
        }
        time = Some((hour, minute, tz));
    }
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    let mut dt = DateTime { year, month, day, hour: None, minute: None, tz: None };
    if let Some((hour, minute, tz)) = time {
        if hour > 23 || minute > 59 {
            return None;
        }
        dt.hour = Some(hour);
        dt.minute = Some(minute);
        dt.tz = tz;
    }
    Some(dt)
}

/// Parse a recurrence like `1w135`, `~2d`, `m` (missing n = 1).
pub fn parse_recurrence(s: &str) -> Option<Recurrence> {
    let s = js_trim(s);
    let (from_completion, s) = match s.strip_prefix('~') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let nd = s.bytes().take_while(u8::is_ascii_digit).count();
    let unit = match s.as_bytes().get(nd)? {
        b'd' => RecurUnit::D,
        b'w' => RecurUnit::W,
        b'm' => RecurUnit::M,
        b'y' => RecurUnit::Y,
        _ => return None,
    };
    let days = &s[nd + 1..];
    if !days.bytes().all(|c| (b'1'..=b'7').contains(&c)) {
        return None;
    }
    // huge intervals (TS keeps them as floats) are out of reach here
    let n = if nd > 0 { s[..nd].parse::<i64>().ok().filter(|n| *n <= 1_000_000)? } else { 1 };
    if n < 1 || (!days.is_empty() && unit != RecurUnit::W) {
        return None;
    }
    let mut weekdays: Vec<i64> = days.bytes().map(|c| i64::from(c - b'0')).collect();
    weekdays.sort_unstable();
    weekdays.dedup();
    Some(Recurrence { n, unit, weekdays, from_completion })
}

/// `s.split(/\s+-\s+/)`.
fn split_range(s: &str) -> Vec<&str> {
    let cs: Vec<(usize, char)> = s.char_indices().collect();
    let ws = |k: usize| cs.get(k).is_some_and(|(_, c)| js_ws(*c as u32));
    let byte = |k: usize| cs.get(k).map_or(s.len(), |(b, _)| *b);
    let mut out = Vec::new();
    let (mut last, mut i) = (0, 0);
    while i < cs.len() {
        if !ws(i) {
            i += 1;
            continue;
        }
        let mut j = i;
        while ws(j) {
            j += 1;
        }
        if cs.get(j).is_some_and(|(_, c)| *c == '-') && ws(j + 1) {
            let mut k = j + 1;
            while ws(k) {
                k += 1;
            }
            out.push(&s[last..byte(i)]);
            last = byte(k);
            i = k;
        } else {
            i = j;
        }
    }
    out.push(&s[last..]);
    out
}

/// Parse the body of a date token (the text between `!(` and `)`).
pub fn parse_date_body(body: &str) -> Option<DateSpec> {
    let (main, recur) = match body.find('|') {
        Some(bar) => (&body[..bar], Some(parse_recurrence(&body[bar + 1..])?)),
        None => (body, None),
    };
    let parts = split_range(js_trim(main));
    if parts.len() > 2 {
        return None;
    }
    let start = parse_date_time(parts[0])?;
    let end = match parts.get(1) {
        Some(p) => Some(parse_date_time(p)?),
        None => None,
    };
    Some(DateSpec { start, end, recur })
}

/// Parse a whole `!(…)` token; `None` if it isn't one.
pub fn parse_date_token(src: &str) -> Option<DateSpec> {
    let body = src.strip_prefix("!(")?.strip_suffix(')')?;
    if body.contains(['(', ')', '\n']) {
        return None;
    }
    parse_date_body(body)
}

// ---- serialization ------------------------------------------------------------------

/// `String(n).padStart(w, '0')`.
fn pad(n: i64, w: usize) -> String {
    let s = n.to_string();
    if s.len() >= w { s } else { format!("{}{s}", "0".repeat(w - s.len())) }
}

pub fn format_date_time(dt: &DateTime) -> String {
    let mut s = format!("{}-{}-{}", pad(dt.year, 4), pad(dt.month, 2), pad(dt.day, 2));
    if let Some(h) = dt.hour {
        s += &format!(" {}:{}", pad(h, 2), pad(dt.minute.unwrap_or(0), 2));
        match dt.tz.as_deref() {
            Some("Z") => s.push('Z'),
            Some(tz) if !tz.is_empty() => s += &format!(" {tz}"),
            _ => {}
        }
    }
    s
}

pub fn format_recurrence(r: &Recurrence) -> String {
    let unit = match r.unit {
        RecurUnit::D => "d",
        RecurUnit::W => "w",
        RecurUnit::M => "m",
        RecurUnit::Y => "y",
    };
    let days: String = if r.unit == RecurUnit::W { r.weekdays.iter().map(i64::to_string).collect() } else { String::new() };
    format!("{}{}{unit}{days}", if r.from_completion { "~" } else { "" }, r.n)
}

/// Canonical body text (without `!(` `)`).
pub fn format_date_body(spec: &DateSpec) -> String {
    let mut s = format_date_time(&spec.start);
    if let Some(e) = &spec.end {
        s += &format!(" - {}", format_date_time(e));
    }
    if let Some(r) = &spec.recur {
        s += &format!(" | {}", format_recurrence(r));
    }
    s
}

pub fn format_date_token(spec: &DateSpec) -> String {
    format!("!({})", format_date_body(spec))
}

/// The UTC offset of the local zone at an instant as `+HH:MM` (for "Encode timezone in new dates").
pub fn local_tz(t: i64) -> String {
    let off = offset_at(t) / MINUTE;
    let a = off.abs();
    format!("{}{}:{}", if off < 0 { '-' } else { '+' }, pad(a / 60, 2), pad(a % 60, 2))
}

/// Give timed endpoints without an offset the local offset of that moment.
pub fn with_local_tz(spec: &DateSpec) -> DateSpec {
    let fix = |dt: &DateTime| {
        let mut dt = dt.clone();
        if let (Some(h), None) = (dt.hour, &dt.tz) {
            dt.tz = Some(local_tz(local_ms(dt.year, dt.month - 1, dt.day, h, dt.minute.unwrap_or(0), 0, 0)));
        }
        dt
    };
    DateSpec { start: fix(&spec.start), end: spec.end.as_ref().map(fix), recur: spec.recur.clone() }
}

/// `!(YYYY-MM-DD)` for today, or `!(YYYY-MM-DD HH:mm)` with the current time.
pub fn current_date_token(now: i64, with_time: bool) -> String {
    format_date_token(&DateSpec { start: from_date(now, with_time), end: None, recur: None })
}

// ---- conversion ---------------------------------------------------------------------

fn tz_offset_minutes(tz: &str) -> i64 {
    if tz == "Z" {
        return 0;
    }
    let b = tz.as_bytes();
    let sign = match b.first() {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return 0,
    };
    let (h, m) = match b.len() {
        5 => (digits(b, 1, 2), digits(b, 3, 2)),
        6 if b[3] == b':' => (digits(b, 1, 2), digits(b, 4, 2)),
        _ => (None, None),
    };
    match (h, m) {
        (Some(h), Some(m)) => sign * (h * 60 + m),
        _ => 0,
    }
}

/// Instant of an endpoint, in ms (all-day = local midnight starting that day).
pub fn to_date(dt: &DateTime) -> i64 {
    match (dt.hour, dt.tz.as_deref()) {
        (Some(h), Some(tz)) if !tz.is_empty() => {
            utc_ms(dt.year, dt.month - 1, dt.day, h, dt.minute.unwrap_or(0)) - tz_offset_minutes(tz) * MINUTE
        }
        _ => local_ms(dt.year, dt.month - 1, dt.day, dt.hour.unwrap_or(0), dt.minute.unwrap_or(0), 0, 0),
    }
}

pub fn from_date(t: i64, with_time: bool) -> DateTime {
    let f = fields(t);
    let mut dt = DateTime { year: f.year, month: f.month0 + 1, day: f.day, hour: None, minute: None, tz: None };
    if with_time {
        dt.hour = Some(f.hours);
        dt.minute = Some(f.minutes);
    }
    dt
}

fn has_tz(dt: &DateTime) -> bool {
    dt.tz.as_deref().is_some_and(|t| !t.is_empty())
}

/// Endpoint as seen in local time (resolves explicit offsets).
fn local_view(dt: &DateTime) -> DateTime {
    if dt.hour.is_none() || !has_tz(dt) {
        return dt.clone();
    }
    from_date(to_date(dt), true)
}

/// Local midnight at the start of the day containing `t`.
pub fn start_of_day(t: i64) -> i64 {
    let f = fields(t);
    local_ms(f.year, f.month0, f.day, 0, 0, 0, 0)
}

/// Add calendar days in local time (DST-safe).
pub fn add_days(t: i64, n: i64) -> i64 {
    let f = fields(t);
    local_ms(f.year, f.month0, f.day + n, f.hours, f.minutes, f.seconds, f.ms)
}

/// Calendar-day difference b - a (local days).
pub fn day_diff(a: i64, b: i64) -> i64 {
    js_round((start_of_day(b) - start_of_day(a)) as f64 / DAY as f64) as i64
}

/// Time span covered by a date, as [start, end) in ms. All-day endpoints cover the whole local day; a timed
/// date without an end is an instant.
pub fn date_range(spec: &DateSpec) -> (i64, i64) {
    let start = to_date(&spec.start);
    let last = spec.end.as_ref().unwrap_or(&spec.start);
    let end = if last.hour.is_none() { add_days(to_date(last), 1) } else { to_date(last) };
    (start, start.max(end))
}

/// True if the date (its end, for ranges) lies in the past.
pub fn is_overdue(spec: &DateSpec, now: i64) -> bool {
    let last = spec.end.as_ref().unwrap_or(&spec.start);
    let (_, end) = date_range(spec);
    if last.hour.is_none() { end <= now } else { end < now }
}

// ---- display ------------------------------------------------------------------------

pub const MONTH_NAMES: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November",
    "December",
];
pub const WEEKDAY_NAMES: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateFormatOptions {
    pub now: i64,
    /// 12h clock (default) vs 24h
    pub hour12: bool,
    /// custom format for dates with a time: moment-style pattern or `relative`
    pub format: Option<String>,
    /// custom format for all-day dates: moment-style pattern or `relative`
    pub format_no_time: Option<String>,
}

impl DateFormatOptions {
    pub fn new(now: i64) -> Self {
        DateFormatOptions { now, hour12: true, format: None, format_no_time: None }
    }
}

/// A custom pattern that is set and is not `relative`.
fn custom(f: &Option<String>) -> Option<&str> {
    f.as_deref().filter(|f| !f.is_empty() && *f != "relative")
}

/// "3 pm", "3:30 pm" (12h) or "15:30" (24h).
pub fn format_time(hour: i64, minute: i64, hour12: bool) -> String {
    if !hour12 {
        return format!("{}:{}", pad(hour, 2), pad(minute, 2));
    }
    let h = if hour % 12 == 0 { 12 } else { hour % 12 };
    let ap = if hour < 12 { "am" } else { "pm" };
    if minute != 0 { format!("{h}:{} {ap}", pad(minute, 2)) } else { format!("{h} {ap}") }
}

/// Format an instant with a moment-style pattern. Supported tokens: YYYY YY MMMM MMM MM M Do DD D dddd ddd HH H
/// hh h mm A a; `[text]` is literal.
pub fn format_pattern(t: i64, pattern: &str) -> String {
    const TOKENS: [&str; 18] =
        ["YYYY", "YY", "MMMM", "MMM", "MM", "M", "Do", "DD", "D", "dddd", "ddd", "HH", "H", "hh", "h", "mm", "A", "a"];
    let f = fields(t);
    let h = f.hours;
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(c) = rest.chars().next() {
        if c == '[' {
            if let Some(close) = rest.find(']') {
                out += &rest[1..close];
                rest = &rest[close + 1..];
                continue;
            }
        }
        let tok = TOKENS.iter().find(|t| rest.starts_with(**t)).copied();
        let Some(tok) = tok else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
            continue;
        };
        out += &match tok {
            "YYYY" => pad(f.year, 4),
            "YY" => pad(f.year % 100, 2),
            "MMMM" => MONTH_NAMES[f.month0 as usize].to_owned(),
            "MMM" => MONTH_NAMES[f.month0 as usize][..3].to_owned(),
            "MM" => pad(f.month0 + 1, 2),
            "M" => (f.month0 + 1).to_string(),
            "Do" => ordinal(f.day),
            "DD" => pad(f.day, 2),
            "D" => f.day.to_string(),
            "dddd" => WEEKDAY_NAMES[f.weekday as usize].to_owned(),
            "ddd" => WEEKDAY_NAMES[f.weekday as usize][..3].to_owned(),
            "HH" => pad(h, 2),
            "H" => h.to_string(),
            "hh" => pad(h12, 2),
            "h" => h12.to_string(),
            "mm" => pad(f.minutes, 2),
            "A" => (if h < 12 { "AM" } else { "PM" }).to_owned(),
            _ => (if h < 12 { "am" } else { "pm" }).to_owned(),
        };
        rest = &rest[tok.len()..];
    }
    out
}

/// "Today", "Tomorrow", "In 3 days", "2 days ago" (|diff| < 7), else "Mon, January 3rd 2022".
pub fn relative_day_label(dt: &DateTime, now: i64) -> String {
    let local = local_view(dt);
    let d = local_ms(local.year, local.month - 1, local.day, 0, 0, 0, 0);
    let diff = day_diff(now, d);
    match diff {
        0 => "Today".to_owned(),
        1 => "Tomorrow".to_owned(),
        -1 => "Yesterday".to_owned(),
        2..=6 => format!("In {diff} days"),
        -6..=-2 => format!("{} days ago", -diff),
        _ => format_pattern(d, "ddd, MMMM Do YYYY"),
    }
}

fn endpoint_label(dt: &DateTime, o: &DateFormatOptions, show_day: bool) -> String {
    let local = local_view(dt);
    let timed = local.hour.is_some();
    if let Some(c) = custom(if timed { &o.format } else { &o.format_no_time }) {
        return format_pattern(to_date(&local), c);
    }
    let time = match local.hour {
        Some(h) => format_time(h, local.minute.unwrap_or(0), o.hour12),
        None => String::new(),
    };
    if !show_day {
        return time;
    }
    let day = relative_day_label(&local, o.now);
    if time.is_empty() { day } else { format!("{day}, {time}") }
}

fn same_day(a: &DateTime, b: &DateTime) -> bool {
    a.year == b.year && a.month == b.month && a.day == b.day
}

/// Human label for a date chip, as Dynalist shows it: "Today, 3 pm", "Tomorrow - In 3 days", "Today, 3 pm - 5 pm",
/// "Tue, January 2nd 2024, every week".
pub fn format_date_label(spec: &DateSpec, o: &DateFormatOptions) -> String {
    let local = local_view(&spec.start);
    let relative = custom(&o.format).is_none();
    // a single timed date on today's calendar day reads "in 3 hours" / "20 minutes ago" (Dynalist)
    let label = if spec.end.is_none()
        && relative
        && local.hour.is_some()
        && day_diff(o.now, local_ms(local.year, local.month - 1, local.day, 0, 0, 0, 0)) == 0
    {
        from_now(to_date(&local) - o.now)
    } else {
        range_label(spec, o)
    };
    match &spec.recur {
        Some(r) => format!("{label}, {}", describe_recurrence(r)),
        None => label,
    }
}

/// moment.js `fromNow()` wording and thresholds (what Dynalist displays).
pub fn from_now(delta_ms: i64) -> String {
    let r = |unit: f64| js_round((delta_ms as f64).abs() / unit);
    let (sec, min, hr, day, mon, yr) =
        (r(1000.0), r(60000.0), r(3_600_000.0), r(86_400_000.0), r(2_629_800_000.0), r(31_557_600_000.0));
    let phrase = if sec <= 44.0 {
        "a few seconds".to_owned()
    } else if min <= 1.0 {
        "a minute".to_owned()
    } else if min < 45.0 {
        format!("{min} minutes")
    } else if hr <= 1.0 {
        "an hour".to_owned()
    } else if hr < 22.0 {
        format!("{hr} hours")
    } else if day <= 1.0 {
        "a day".to_owned()
    } else if day < 26.0 {
        format!("{day} days")
    } else if mon <= 1.0 {
        "a month".to_owned()
    } else if mon < 11.0 {
        format!("{mon} months")
    } else if yr <= 1.0 {
        "a year".to_owned()
    } else {
        format!("{yr} years")
    };
    if delta_ms >= 0 { format!("in {phrase}") } else { format!("{phrase} ago") }
}

fn range_label(spec: &DateSpec, o: &DateFormatOptions) -> String {
    let start = endpoint_label(&spec.start, o, true);
    let Some(end) = &spec.end else { return start };
    let a = local_view(&spec.start);
    let b = local_view(end);
    let custom_end = custom(if b.hour.is_some() { &o.format } else { &o.format_no_time });
    let time_only = same_day(&a, &b) && a.hour.is_some() && b.hour.is_some() && custom_end.is_none();
    format!("{start} - {}", endpoint_label(end, o, !time_only))
}

/// "every day", "every 2 weeks on Mon, Wed", "every month (from completion)".
pub fn describe_recurrence(r: &Recurrence) -> String {
    let (one, many) = match r.unit {
        RecurUnit::D => ("day", "days"),
        RecurUnit::W => ("week", "weeks"),
        RecurUnit::M => ("month", "months"),
        RecurUnit::Y => ("year", "years"),
    };
    let mut s = if r.n == 1 { format!("every {one}") } else { format!("every {} {many}", r.n) };
    if r.unit == RecurUnit::W && !r.weekdays.is_empty() {
        let days: Vec<&str> = r.weekdays.iter().map(|w| &WEEKDAY_NAMES[(w % 7) as usize][..3]).collect();
        s += &format!(" on {}", days.join(", "));
    }
    if r.from_completion {
        s += " (from completion)";
    }
    s
}

// ---- recurrence ---------------------------------------------------------------------

fn shift_date(dt: &DateTime, days: i64) -> DateTime {
    let f = fields(local_ms(dt.year, dt.month - 1, dt.day + days, 0, 0, 0, 0));
    DateTime { year: f.year, month: f.month0 + 1, day: f.day, ..dt.clone() }
}

fn add_months_clamped(dt: &DateTime, months: i64) -> DateTime {
    let idx = dt.year * 12 + (dt.month - 1) + months;
    let year = idx.div_euclid(12);
    // `%` truncates, as in JS
    let month = idx % 12 + 1;
    DateTime { year, month, day: dt.day.min(days_in_month(year, month)), ..dt.clone() }
}

/// The days-since-epoch number of a calendar date (`Date.UTC(y, m - 1, d) / DAY`).
fn day_number(dt: &DateTime) -> i64 {
    make_day(js_year(dt.year), dt.month - 1, dt.day)
}

/// ISO weekday 1..7 of a calendar date.
fn iso_weekday(dt: &DateTime) -> i64 {
    match (day_number(dt) + 4).rem_euclid(7) {
        0 => 7,
        w => w,
    }
}

fn next_start(base: &DateTime, r: &Recurrence) -> DateTime {
    match r.unit {
        RecurUnit::D => shift_date(base, r.n),
        RecurUnit::M => add_months_clamped(base, r.n),
        RecurUnit::Y => add_months_clamped(base, 12 * r.n),
        RecurUnit::W => {
            if r.weekdays.is_empty() {
                return shift_date(base, 7 * r.n);
            }
            // weeks are Monday-based; eligible weeks are multiples of n from the base week
            let base_monday = day_number(base) - (iso_weekday(base) - 1);
            for i in 1..=7 * r.n + 7 {
                let cand = shift_date(base, i);
                let week = (day_number(&cand) - base_monday).div_euclid(7);
                if week % r.n == 0 && r.weekdays.contains(&iso_weekday(&cand)) {
                    return cand;
                }
            }
            shift_date(base, 7 * r.n)
        }
    }
}

/// The date after checking off a recurring item: start advanced by one interval (from today when
/// `from_completion`), range length preserved, all-day stays all-day, times of day kept. `None` if the spec has
/// no recurrence.
pub fn next_occurrence(spec: &DateSpec, now: i64) -> Option<DateSpec> {
    let r = spec.recur.as_ref()?;
    let mut base = spec.start.clone();
    if r.from_completion {
        let today = from_date(now, false);
        base = DateTime { year: today.year, month: today.month, day: today.day, ..spec.start.clone() };
    }
    let start = next_start(&base, r);
    let end = spec.end.as_ref().map(|e| shift_date(e, day_number(&start) - day_number(&spec.start)));
    Some(DateSpec { start, end, recur: Some(r.clone()) })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundDate {
    /// UTF-16 offset of `!`
    pub start: usize,
    /// UTF-16 offset after `)`
    pub end: usize,
    pub spec: DateSpec,
}

/// All valid `!(…)` dates in raw text (a plain scan, as TS's regex; see `inline::extract_dates` for a
/// syntax-aware one). Offsets are UTF-16.
pub fn find_dates(src: &str) -> Vec<FoundDate> {
    let u: Vec<u16> = src.encode_utf16().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < u.len() {
        if u[i] != u16::from(b'!') || u[i + 1] != u16::from(b'(') {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        while j < u.len() && !b"()\n".map(u16::from).contains(&u[j]) {
            j += 1;
        }
        if j >= u.len() || u[j] != u16::from(b')') {
            i += 1;
            continue;
        }
        if !(i > 0 && u[i - 1] == u16::from(b'\\')) {
            if let Some(spec) = parse_date_body(&String::from_utf16_lossy(&u[i + 2..j])) {
                out.push(FoundDate { start: i, end: j + 1, spec });
            }
        }
        i = j + 1;
    }
    out
}

/// If the first date in `src` recurs, the text with that date advanced to its next occurrence (and the new
/// spec); otherwise `None`.
pub fn advance_recurrence(src: &str, now: i64) -> Option<(String, DateSpec)> {
    let first = find_dates(src).into_iter().next()?;
    first.spec.recur.as_ref()?;
    let spec = next_occurrence(&first.spec, now)?;
    let u: Vec<u16> = src.encode_utf16().collect();
    let text = String::from_utf16_lossy(&u[..first.start]) + &format_date_token(&spec) + &String::from_utf16_lossy(&u[first.end..]);
    Some((text, spec))
}
