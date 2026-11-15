//! Checks the link helpers against what the TS core does: `parseItemUrl`, backlinks and backlink counts across
//! documents, and find/replace over a subtree.
//!
//!     node native/crates/outliner-core/fixtures/gen-refs.ts

mod common;

use std::collections::BTreeMap;

use common::*;
use outliner_core::refs::{backlink_counts, count_matches, find_backlinks, parse_item_url, replace_all, DocTree};
use outliner_core::{Tree, Tx};
use serde_json::json;

#[test]
fn refs_match_ts() {
    let fx = fixture("refs", "gen-refs.ts");
    let mut c = Check::default();
    for u in arr(&fx["urls"]) {
        c.eq("parseItemUrl", u, json(&parse_item_url(s(&u[0]))), &u[1]);
    }
    for case in arr(&fx["cases"]) {
        let trees: Vec<(String, Tree)> = arr(&case["docs"]).iter().map(|d| (s(&d["doc"]).to_owned(), tree(&d["tree"]))).collect();
        let docs: Vec<DocTree<'_>> = trees.iter().map(|(doc, tree)| DocTree { doc, tree }).collect();
        for b in arr(&case["back"]) {
            c.eq("findBacklinks", b, json(&find_backlinks(&docs, s(&b[0]), s(&b[1]))), &b[2]);
        }
        for k in arr(&case["counts"]) {
            let counts: BTreeMap<_, _> = backlink_counts(&docs, s(&k[0])).into_iter().collect();
            c.eq("backlinkCounts", k, json(&counts), &k[1]);
        }
        for f in arr(&case["finds"]) {
            let mut t = trees[i(&f["k"]) as usize].1.clone();
            let (scope, find, rep, mc) = (s(&f["scope"]), s(&f["find"]), s(&f["replace"]), f["matchCase"].as_bool().unwrap());
            c.eq("countMatches", f, json(&count_matches(&t, scope, find, mc)), &f["count"]);
            let mut tx = Tx::new(&mut t, 7000);
            let n = replace_all(&mut tx, scope, find, rep, mc);
            c.eq("replaceAll", f, json!([n, tx.ops]), &json!([f["n"], f["ops"]]));
        }
    }
    c.done("refs");
}
