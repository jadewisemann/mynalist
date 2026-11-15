//! Checks the search query language and the fuzzy matcher against what the TS core does: queries over random
//! trees (in several time zones), operator time values, whole-word matching, fuzzy matching and filtering.
//!
//!     node native/crates/outliner-core/fixtures/gen-search.ts
//!
//! A single test: the zone cases set `TZ` for the process.

mod common;

use common::*;
use outliner_core::search::{compile_query, forces_show_checked, fuzzy_filter, fuzzy_match, parse_query_time, search_tree, whole_word_match};
use serde_json::{json, Value};

fn zone_cases(c: &mut Check, cases: &[Value]) {
    for z in cases {
        let now = i(&z["now"]);
        let t = tree(&z["tree"]);
        let mut ids: Vec<String> = t.nodes().map(|n| n.id.clone()).collect();
        ids.push("missing".into());
        for case in arr(&z["cases"]) {
            let q = s(&case["q"]);
            let m = compile_query(q, now, case["typing"].as_bool().unwrap());
            c.eq("terms", case, json(&m.terms), &case["terms"]);
            c.eq("forcesShowChecked", case, json(&m.forces_show_checked), &case["forces"]);
            c.eq("empty", case, json(&m.empty), &case["empty"]);
            c.eq("forcesShowChecked(q)", case, json(&forces_show_checked(q)), &case["forcesOnly"]);
            let mut hits: Vec<&String> = ids.iter().filter(|id| m.matches(&t, id)).collect();
            hits.sort();
            let mut want: Vec<String> = arr(&case["hits"]).iter().map(|v| s(v).to_owned()).collect();
            want.sort();
            c.eq("matches", case, json(&hits), &json(&want));
            let res = search_tree(&t, &m, Some(s(&case["scope"])), case["showChecked"].as_bool());
            c.eq("searchTree matches", case, json(&res.matches), &case["matches"]);
            let mut vis: Vec<&String> = res.visible.iter().collect();
            vis.sort();
            c.eq("searchTree visible", case, json(&vis), &case["visible"]);
        }
        for tv in arr(&z["times"]) {
            c.eq("parseQueryTime", tv, json(&parse_query_time(s(&tv[0]), now)), &tv[1]);
        }
    }
}

#[test]
fn search_matches_ts() {
    let fx = fixture("search", "gen-search.ts");
    let mut c = Check::default();
    for w in arr(&fx["words"]) {
        let hit = whole_word_match(s(&w["hay"]), s(&w["needle"]), w["cs"].as_bool().unwrap(), w["prefix"].as_bool().unwrap());
        c.eq("wholeWordMatch", w, json(&hit), &w["hit"]);
    }
    for f in arr(&fx["fuzzy"]) {
        c.eq("fuzzyMatch", f, json(&fuzzy_match(s(&f["q"]), s(&f["target"]))), &f["m"]);
    }
    for f in arr(&fx["filters"]) {
        let items: Vec<String> = arr(&f["items"]).iter().map(|v| s(v).to_owned()).collect();
        let out: Vec<Value> = fuzzy_filter(s(&f["q"]), &items, |x| x.as_str()).into_iter().map(|(it, m)| json!([it, m])).collect();
        c.eq("fuzzyFilter", f, json(&out), &f["out"]);
    }
    zones(&mut c, &fx["zones"], zone_cases);
    c.done("search");
}
