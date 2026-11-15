//! iCalendar (RFC 5545) feed of every dated item: a read-only calendar subscription. Port of
//! `packages/core/src/calendar.ts`. Times without an offset are written as floating local times, as the app shows
//! them.

use crate::dates::{civil_from_days, js_trim, js_ws, utc_ms, DateSpec, DateTime, RecurUnit, DAY};
use crate::inline::{extract_dates, inline_to_plain};
use crate::text16;
use crate::tree::{is_falsy, NodeId, Tree, ROOT};

fn icsdt(ms: i64) -> String {
    let (y, m, d) = civil_from_days(ms.div_euclid(DAY));
    let tod = ms.rem_euclid(DAY);
    format!("{y}{m:02}{d:02}T{:02}{:02}{:02}", tod / 3_600_000, tod % 3_600_000 / 60_000, tod % 60_000 / 1000)
}

/// `;VALUE=DATE:…` for a day, `:…` for a floating local time, `:…Z` (in UTC) for a time with an offset.
fn ics_date(dt: &DateTime) -> String {
    let Some(hour) = dt.hour else {
        return format!(";VALUE=DATE:{}{:02}{:02}", dt.year, dt.month, dt.day);
    };
    let minute = dt.minute.unwrap_or(0);
    let Some(tz) = dt.tz.as_deref().filter(|t| !t.is_empty()) else {
        return format!(":{}{:02}{:02}T{hour:02}{minute:02}00", dt.year, dt.month, dt.day);
    };
    // explicit offset (`±HH:MM` or `±HHMM`; anything else counts as UTC): convert to UTC
    let b = tz.as_bytes();
    let digits = |s: &[u8]| s.iter().all(u8::is_ascii_digit).then(|| s.iter().fold(0, |a, d| a * 10 + i64::from(d - b'0')));
    let off = match b {
        [s @ (b'+' | b'-'), h1, h2, b':', m1, m2] | [s @ (b'+' | b'-'), h1, h2, m1, m2] => {
            match (digits(&[*h1, *h2]), digits(&[*m1, *m2])) {
                (Some(h), Some(m)) => (if *s == b'-' { -1 } else { 1 }) * (h * 60 + m),
                _ => 0,
            }
        }
        _ => 0,
    };
    let t = utc_ms(dt.year, dt.month - 1, dt.day, hour, minute - off);
    format!(":{}Z", icsdt(t))
}

/// The day after (all-day DTEND is exclusive).
fn next_day(dt: &DateTime) -> DateTime {
    let (year, month, day) = civil_from_days(utc_ms(dt.year, dt.month - 1, dt.day + 1, 0, 0).div_euclid(DAY));
    DateTime { year, month, day, hour: None, minute: None, tz: None }
}

fn rrule(spec: &DateSpec) -> Option<String> {
    let r = spec.recur.as_ref()?;
    let freq = match r.unit {
        RecurUnit::D => "DAILY",
        RecurUnit::W => "WEEKLY",
        RecurUnit::M => "MONTHLY",
        RecurUnit::Y => "YEARLY",
    };
    let mut s = format!("RRULE:FREQ={freq};INTERVAL={}", r.n);
    if r.unit == RecurUnit::W && !r.weekdays.is_empty() {
        const DAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
        let days: Vec<&str> = r.weekdays.iter().map(|d| usize::try_from(d - 1).ok().and_then(|i| DAYS.get(i)).copied().unwrap_or("")).collect();
        s += &format!(";BYDAY={}", days.join(","));
    }
    Some(s)
}

/// Text value escaping: `;` and `,` get a backslash, line breaks become `\n`.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            ';' | ',' => {
                out.push('\\');
                out.push(c);
            }
            '\r' if it.peek() == Some(&'\n') => {
                it.next();
                out += "\\n";
            }
            '\n' => out += "\\n",
            _ => out.push(c),
        }
    }
    out
}

/// Fold content lines at 75 octets (approximately: 74 characters a line), as TS's `/.{1,74}/gu` does it: line
/// terminators are not matched by `.`, so they drop out.
fn fold(line: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut n = 0;
    for c in line.chars() {
        if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
            if n > 0 {
                parts.push(std::mem::take(&mut cur));
                n = 0;
            }
            continue;
        }
        cur.push(c);
        n += 1;
        if n == 74 {
            parts.push(std::mem::take(&mut cur));
            n = 0;
        }
    }
    if n > 0 {
        parts.push(cur);
    }
    if parts.is_empty() { line.to_owned() } else { parts.join("\r\n ") }
}

/// The link of an item: `(doc, id)` → URL.
pub type ItemUrl<'a> = &'a dyn Fn(&str, &str) -> String;

/// A document of the feed.
pub struct CalendarDoc<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub tree: &'a Tree,
}

#[derive(Default)]
pub struct CalendarOptions<'a> {
    /// calendar name (default "Outliner")
    pub name: Option<&'a str>,
    /// link of an item: `(doc, id)`
    pub item_url: Option<ItemUrl<'a>>,
    /// DTSTAMP time, ms (default now)
    pub stamp: Option<i64>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn prop_str<'a>(t: &'a Tree, id: &str, k: &str) -> &'a str {
    t.get(id).and_then(|n| n.props.get(k)).and_then(|v| v.as_str()).unwrap_or("")
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::new();
    let mut ws = false;
    for c in s.chars() {
        if js_ws(c as u32) {
            ws = true;
        } else {
            if ws {
                out.push(' ');
            }
            ws = false;
            out.push(c);
        }
    }
    if ws {
        out.push(' ');
    }
    out
}

/// The feed: one event per date of every unchecked item (recurring dates get an RRULE).
pub fn build_calendar(docs: &[CalendarDoc<'_>], opts: &CalendarOptions<'_>) -> String {
    let stamp = icsdt(opts.stamp.unwrap_or_else(now_ms)) + "Z";
    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//Outliner//Calendar feed//EN".into(),
        "CALSCALE:GREGORIAN".into(),
        format!("X-WR-CALNAME:{}", esc(opts.name.unwrap_or("Outliner"))),
    ];
    for doc in docs {
        let t = doc.tree;
        // children after their descendants, as the TS walk
        fn walk(t: &Tree, id: &str, out: &mut Vec<NodeId>) {
            for c in t.get(id).map(|n| n.children.as_slice()).unwrap_or_default() {
                walk(t, c, out);
                out.push(c.clone());
            }
        }
        let mut order = Vec::new();
        walk(t, ROOT, &mut order);
        for c in &order {
            let Some(n) = t.get(c) else { continue };
            let src = prop_str(t, c, "c");
            if n.props.get("ck").is_some_and(|v| !is_falsy(v)) || !src.contains("!(") {
                continue;
            }
            let dates = extract_dates(src);
            // the summary is the item without its date chips
            let mut text = src.to_owned();
            for d in dates.iter().rev() {
                text = format!("{}{}", text16::slice(&text, 0, d.start), text16::slice_from(&text, d.end));
            }
            let summary = collapse_ws(&inline_to_plain(&text, None));
            let summary = match js_trim(&summary) {
                "" => "(untitled)",
                s => s,
            };
            for (i, d) in dates.iter().enumerate() {
                let s = &d.spec;
                let all_day = s.start.hour.is_none();
                let end = s.end.as_ref().or(if all_day { Some(&s.start) } else { None });
                lines.push("BEGIN:VEVENT".into());
                lines.push(format!("UID:{}-{c}-{i}@outliner", doc.id));
                lines.push(format!("DTSTAMP:{stamp}"));
                lines.push(format!("DTSTART{}", ics_date(&s.start)));
                if let Some(end) = end {
                    let end = if all_day && end.hour.is_none() { next_day(end) } else { end.clone() };
                    lines.push(format!("DTEND{}", ics_date(&end)));
                }
                lines.push(fold(&format!("SUMMARY:{}", esc(summary))));
                let note = inline_to_plain(prop_str(t, c, "n"), None);
                let mut where_ = vec![doc.title.to_owned()];
                for a in t.ancestors(c) {
                    if a != ROOT {
                        where_.push(inline_to_plain(prop_str(t, &a, "c"), None));
                    }
                }
                let where_ = where_.join(" › ");
                let desc: Vec<&str> = [where_.as_str(), note.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
                lines.push(fold(&format!("DESCRIPTION:{}", esc(&desc.join("\n\n")))));
                if let Some(url) = opts.item_url {
                    lines.push(fold(&format!("URL:{}", url(doc.id, c))));
                }
                if let Some(rr) = rrule(s) {
                    lines.push(rr);
                }
                lines.push("END:VEVENT".into());
            }
        }
    }
    lines.push("END:VCALENDAR".into());
    lines.join("\r\n") + "\r\n"
}
