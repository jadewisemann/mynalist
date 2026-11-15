//! Checks the iCalendar feed against what the TS core writes for random documents of dated items.
//!
//!     node native/crates/outliner-core/fixtures/gen-calendar.ts

mod common;

use common::*;
use outliner_core::calendar::{build_calendar, CalendarDoc, CalendarOptions};
use outliner_core::Tree;
use serde_json::json;

#[test]
fn calendar_matches_ts() {
    let fx = fixture("calendar", "gen-calendar.ts");
    let mut c = Check::default();
    let url = |doc: &str, id: &str| format!("https://app.example/#d={doc}&z={id}");
    for case in arr(&fx["cases"]) {
        let trees: Vec<(String, String, Tree)> =
            arr(&case["docs"]).iter().map(|d| (s(&d["id"]).to_owned(), s(&d["title"]).to_owned(), tree(&d["tree"]))).collect();
        let docs: Vec<CalendarDoc<'_>> = trees.iter().map(|(id, title, tree)| CalendarDoc { id, title, tree }).collect();
        let opts = CalendarOptions {
            name: case["name"].as_str(),
            item_url: if case["withUrl"].as_bool().unwrap() { Some(&url) } else { None },
            stamp: Some(i(&case["stamp"])),
        };
        c.eq("buildCalendar", &json!({ "docs": case["docs"], "name": case["name"] }), json(&build_calendar(&docs, &opts)), &case["ics"]);
    }
    c.done("calendar");
}
