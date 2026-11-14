//! Checks the inline parser and the date helpers against what the TS core does on the same input: the web's own
//! test strings plus random markup, and date cases recorded in several time zones.
//!
//! The fixture is recorded by `fixtures/gen-inline.ts` (not committed; it is regenerated from `packages/core`):
//!
//!     node native/crates/outliner-core/fixtures/gen-inline.ts
//!
//! This file holds a single test: the date cases set `TZ` for the process, so nothing else may run beside them.

use outliner_core::dates::{self, DateFormatOptions, DateSpec};
use outliner_core::inline;
use serde_json::{json, Value};

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/inline.json");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("{path} is missing: run `node native/crates/outliner-core/fixtures/gen-inline.ts`"));
    serde_json::from_str(&text).expect("fixture parses")
}

fn json<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("serializable")
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

fn opt_str(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

/// Compares and reports the case on a mismatch (all mismatches are counted, the first few printed).
struct Check {
    failures: usize,
    checks: usize,
}

impl Check {
    fn eq(&mut self, what: &str, case: &Value, got: Value, want: &Value) {
        self.checks += 1;
        if &got != want {
            self.failures += 1;
            if self.failures <= 12 {
                eprintln!("MISMATCH {what}\n  case: {}\n  got:  {got}\n  want: {want}", trunc(case));
            }
        }
    }
}

fn trunc(v: &Value) -> String {
    let t = v.to_string();
    if t.len() > 600 { format!("{}…", &t[..t.char_indices().take_while(|(k, _)| *k < 600).last().map_or(0, |(k, _)| k)]) } else { t }
}

fn link_title(url: &str) -> Option<String> {
    // the generator's: odd UTF-16 lengths have a title
    (url.encode_utf16().count() % 2 == 1).then(|| format!("Title of {url}"))
}

fn inline_cases(c: &mut Check, cases: &[Value]) {
    for case in cases {
        let src = s(&case["src"]);
        c.eq("tokens", case, json(&inline::parse_inline(src)), &case["tokens"]);
        c.eq("plain", case, json(&inline::inline_to_plain(src, None)), &case["plain"]);
        c.eq("plainTitled", case, json(&inline::inline_to_plain(src, Some(&link_title))), &case["plainTitled"]);
        c.eq("tags", case, json(&inline::extract_tags(src)), &case["tags"]);
        c.eq("links", case, json(&inline::extract_links(src)), &case["links"]);
        c.eq("dates", case, json(&inline::extract_dates(src)), &case["dates"]);
        c.eq("unescape", case, json(&inline::unescape(src)), &case["unescaped"]);
        c.eq("escape", case, json(&inline::escape_inline(src)), &case["escaped"]);
        c.eq("normalizeUrl", case, json(&inline::normalize_url(src)), &case["normalized"]);
        c.eq("safeUrl", case, json(&inline::safe_url(src, false)), &case["safe"]);
        c.eq("safeUrl image", case, json(&inline::safe_url(src, true)), &case["safeImage"]);
        let r = &case["rename"];
        c.eq("replaceTag", case, json(&inline::replace_tag(src, s(&r[0]), Some("#new"))), &r[1]);
        c.eq("deleteTag", case, json(&inline::replace_tag(src, s(&r[0]), None)), &r[2]);
        let at = i(&case["path"][0]) as usize;
        let tokens = inline::parse_inline(src);
        let path: Vec<[usize; 2]> = inline::token_path_at(&tokens, at).iter().map(|t| [t.start, t.end]).collect();
        c.eq("tokenPathAt", case, json(&path), &case["path"][1]);
        let terms: Vec<&str> = case["matches"][0].as_array().unwrap().iter().map(s).collect();
        let ranges: Vec<[usize; 2]> = inline::match_ranges(src, &terms).into_iter().map(|(a, b)| [a, b]).collect();
        c.eq("matchRanges", case, json(&ranges), &case["matches"][1]);
    }
}

fn zone_cases(c: &mut Check, cases: &[Value]) {
    for case in cases {
        let now = i(&case["now"]);
        let spec = dates::parse_date_body(s(&case["body"]));
        c.eq("parseDateBody", case, json(&spec), &case["spec"]);
        if let (Some(spec), false) = (&spec, case["spec"].is_null()) {
            let want: DateSpec = serde_json::from_value(case["spec"].clone()).unwrap();
            assert_eq!(spec, &want);
            c.eq("canonical", case, json(&dates::format_date_body(spec)), &case["canonical"]);
            c.eq("toDate", case, json(&dates::to_date(&spec.start)), &case["start"]);
            let (a, b) = dates::date_range(spec);
            c.eq("dateRange", case, json!({ "start": a, "end": b }), &case["range"]);
            c.eq("isOverdue", case, json(&dates::is_overdue(spec, now)), &case["overdue"]);
            let o = &case["opts"];
            let opts = DateFormatOptions {
                now: i(&o["now"]),
                hour12: o["hour12"].as_bool().unwrap(),
                format: opt_str(&o["format"]),
                format_no_time: opt_str(&o["formatNoTime"]),
            };
            c.eq("formatDateLabel", case, json(&dates::format_date_label(spec, &opts)), &case["label"]);
            c.eq("relativeDayLabel", case, json(&dates::relative_day_label(&spec.start, now)), &case["relDay"]);
            c.eq("describeRecurrence", case, json(&spec.recur.as_ref().map(dates::describe_recurrence)), &case["recur"]);
            c.eq("nextOccurrence", case, json(&dates::next_occurrence(spec, now)), &case["next"]);
            c.eq("withLocalTz", case, json(&dates::format_date_body(&dates::with_local_tz(spec))), &case["localized"]);
        }
        let text = s(&case["text"]);
        let found: Vec<Value> = dates::find_dates(text)
            .into_iter()
            .map(|f| json!({ "start": f.start, "end": f.end, "spec": f.spec }))
            .collect();
        c.eq("findDates", case, json(&found), &case["found"]);
        let adv = dates::advance_recurrence(text, now).map(|(text, spec)| json!({ "text": text, "spec": spec }));
        c.eq("advanceRecurrence", case, json(&adv), &case["advanced"]);
        c.eq("currentDateToken", case, json(&dates::current_date_token(now, false)), &case["currentDay"]);
        c.eq("currentDateToken time", case, json(&dates::current_date_token(now, true)), &case["currentTime"]);
        c.eq("startOfDay", case, json(&dates::start_of_day(now)), &case["sod"]);
        let k = i(&case["plusDays"][0]);
        c.eq("addDays", case, json(&dates::add_days(now, k)), &case["plusDays"][1]);
        let delta = i(&case["fromNow"][0]);
        c.eq("dayDiff", case, json(&dates::day_diff(now, now + delta)), &case["diff"]);
        c.eq("fromNow", case, json(&dates::from_now(delta)), &case["fromNow"][1]);
        c.eq("localTz", case, json(&dates::local_tz(now)), &case["tz"]);
        let p = &case["pattern"];
        c.eq("formatPattern", case, json(&dates::format_pattern(now + delta, s(&p[0]))), &p[1]);
    }
}

#[test]
fn inline_and_dates_match_ts() {
    let fx = fixture();
    let mut c = Check { failures: 0, checks: 0 };
    inline_cases(&mut c, fx["inline"].as_array().unwrap());
    let zones = fx["zones"].as_object().unwrap();
    for (zone, cases) in zones {
        // chrono caches the zone per thread: a fresh thread reads the TZ just set
        std::env::set_var("TZ", zone);
        let cases = cases.as_array().unwrap().clone();
        let (fails, checks) = std::thread::spawn(move || {
            let mut c = Check { failures: 0, checks: 0 };
            zone_cases(&mut c, &cases);
            (c.failures, c.checks)
        })
        .join()
        .unwrap();
        eprintln!("{zone}: {checks} checks, {fails} mismatches");
        c.failures += fails;
        c.checks += checks;
    }
    assert!(zones.len() >= 5, "the fixture has the time zones");
    assert_eq!(c.failures, 0, "{} of {} checks differ from TS", c.failures, c.checks);
    eprintln!("{} checks", c.checks);
}
