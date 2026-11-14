//! Checks the editor commands and view helpers against what the TS core does on the same random input.
//!
//! The fixture is recorded by `fixtures/gen-commands.ts` (not committed; it is regenerated from `packages/core`):
//!
//!     node native/crates/outliner-core/fixtures/gen-commands.ts

use std::collections::HashSet;

use outliner_core::commands::{self as cmd, CheckOptions, Dir, Keep, Portable, Sel, SortMode};
use outliner_core::view::{self, View};
use outliner_core::{NodeId, SerializedTree, Tree, Tx};
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/commands.json");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("{path} is missing: run `node native/crates/outliner-core/fixtures/gen-commands.ts`"));
    serde_json::from_str(&text).expect("fixture parses")
}

fn de<T: serde::de::DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}

fn json<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("serializable")
}

fn tree(v: &Value) -> Tree {
    Tree::from_serialized(&de::<SerializedTree>(v))
}

fn view_of(v: &Value) -> View {
    View {
        zoom: v["zoom"].as_str().unwrap().to_owned(),
        show_checked: v["showChecked"].as_bool().unwrap(),
        filter: v["filter"].as_array().map(|f| f.iter().map(|x| x.as_str().unwrap().to_owned()).collect::<HashSet<_>>()),
    }
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn n(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}

fn opt_id(v: &Value) -> Value {
    v.clone()
}

/// `null` for "nothing", as TS returns.
fn result<T: serde::Serialize>(r: Option<T>) -> Value {
    r.map(|r| json(&r)).unwrap_or(Value::Null)
}

#[test]
fn commands_match_ts() {
    let fx = fixture();
    let cases = fx["cases"].as_array().unwrap();
    let mut failures = Vec::new();
    for (i, c) in cases.iter().enumerate() {
        let mut t = tree(&c["tree"]);
        let v = view_of(&c["view"]);
        let a = &c["args"];
        let ids = || de::<Vec<NodeId>>(&a["ids"]);
        let sel = || de::<Sel>(&a["sel"]);
        let name = s(&c["cmd"]);
        let mut tx = Tx::new(&mut t, c["ts"].as_i64().unwrap());
        tx.ids = de::<Vec<NodeId>>(&c["ids"]).into();
        let got = match name {
            "enter" => result(cmd::enter(&mut tx, &v, s(&a["id"]), n(&a["s"]), n(&a["e"]))),
            "backspaceAtStart" => result(cmd::backspace_at_start(&mut tx, &v, s(&a["id"]))),
            "deleteAtEnd" => result(cmd::delete_at_end(&mut tx, &v, s(&a["id"]))),
            "indent" => result(cmd::indent(&mut tx, &v, &ids(), sel())),
            "outdent" => result(cmd::outdent(&mut tx, &v, &ids(), sel(), a["inPlace"].as_bool().unwrap())),
            "moveUp" => result(cmd::move_up(&mut tx, &v, &ids(), sel())),
            "moveDown" => result(cmd::move_down(&mut tx, &v, &ids(), sel())),
            "deleteItems" => {
                let dir = if s(&a["dir"]) == "left" { Dir::Left } else { Dir::Right };
                result(cmd::delete_items(&mut tx, &v, &ids(), dir))
            }
            "replaceWithEmpty" => result(cmd::replace_with_empty(&mut tx, &v, &ids())),
            "toggleChecked" => {
                let opts = CheckOptions { stay: a["stay"].as_bool().unwrap() };
                result(cmd::toggle_checked(&mut tx, &v, &ids(), sel(), &opts))
            }
            "toggleProp" => json(&cmd::toggle_prop(&mut tx, &ids(), s(&a["key"]), sel())),
            "setLevel" => json(&cmd::set_level(&mut tx, &ids(), s(&a["key"]), a["value"].as_i64().unwrap(), sel())),
            "cycleLevel" => json(&cmd::cycle_level(&mut tx, &ids(), s(&a["key"]), a["max"].as_i64().unwrap(), sel())),
            "uncheckDescendants" => json(&cmd::uncheck_descendants(&mut tx, &ids(), sel())),
            "setChildrenCheckbox" => json(&cmd::set_children_checkbox(&mut tx, &ids(), sel())),
            "toggleCollapse" => {
                cmd::toggle_collapse(&mut tx, &v, &ids(), a["recursive"].as_bool().unwrap());
                Value::Null
            }
            "expandToLevel" => {
                cmd::expand_to_level(&mut tx, &ids(), n(&a["level"]));
                Value::Null
            }
            "toggleSiblings" => {
                cmd::toggle_siblings(&mut tx, s(&a["id"]));
                Value::Null
            }
            "duplicate" => result(cmd::duplicate(&mut tx, &ids())),
            "pasteItems" => {
                let items: Vec<Portable> = de(&a["items"]);
                result(cmd::paste_items(&mut tx, &v, s(&a["id"]), n(&a["s"]), n(&a["e"]), &items))
            }
            "sortChildren" => {
                let mode: SortMode = de(&a["mode"]);
                json(&cmd::sort_children(&mut tx, &ids(), mode, sel(), &|_| None))
            }
            "toggleWrapItems" => {
                cmd::toggle_wrap_items(&mut tx, &ids(), s(&a["d"]));
                Value::Null
            }
            "merge" => {
                let keep = if s(&a["keep"]) == "a" { Keep::A } else { Keep::B };
                match cmd::merge(&mut tx, s(&a["a"]), s(&a["b"]), keep) {
                    Some((id, at)) => serde_json::json!({ "id": id, "at": at }),
                    None => Value::Null,
                }
            }
            other => panic!("unknown command {other}"),
        };
        let (ops, inverse) = (json(&tx.ops), json(&tx.inverse));
        let broken = t.nodes().flat_map(|n| n.children.iter()).find(|c| !t.contains(c)).cloned();
        if let Some(b) = broken {
            failures.push(format!("case {i} {name} {a}: child {b} missing; ops {ops}"));
            continue;
        }
        let canonical = t.canonical();
        let mut diffs = Vec::new();
        if got != c["result"] {
            diffs.push(format!("result\n    rust: {got}\n    ts:   {}", c["result"]));
        }
        if ops != c["ops"] {
            diffs.push(format!("ops\n    rust: {ops}\n    ts:   {}", c["ops"]));
        }
        if inverse != c["inverse"] {
            diffs.push("inverse".to_owned());
        }
        if canonical != s(&c["canonical"]) {
            diffs.push("tree".to_owned());
        }
        if !diffs.is_empty() {
            failures.push(format!("case {i} {name} {a}: {}", diffs.join("; ")));
        }
    }
    assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), cases.len(), failures[..failures.len().min(8)].join("\n"));
}

#[test]
fn view_helpers_match_ts() {
    let fx = fixture();
    for (i, c) in fx["views"].as_array().unwrap().iter().enumerate() {
        let t = tree(&c["tree"]);
        let v = view_of(&c["view"]);
        assert_eq!(json(&view::flatten(&t, &v)), c["flatten"], "case {i}: flatten");
        for p in c["per"].as_array().unwrap() {
            let id = s(&p["id"]);
            let at = format!("case {i} item {id}");
            assert_eq!(json(&view::visible_children(&t, id, &v)), p["visibleChildren"], "{at}: visible_children");
            assert_eq!(json(&view::prev_visible(&t, id, &v)), opt_id(&p["prevVisible"]), "{at}: prev_visible");
            assert_eq!(json(&view::next_visible(&t, id, &v)), p["nextVisible"], "{at}: next_visible");
            assert_eq!(
                json(&view::next_visible_non_descendant(&t, id, &v)),
                p["nextVisibleNonDescendant"],
                "{at}: next_visible_non_descendant"
            );
            assert_eq!(json(&view::last_visible_descendant(&t, id, &v)), p["lastVisibleDescendant"], "{at}");
            assert_eq!(json(&view::prev_visible_sibling(&t, id, &v)), p["prevVisibleSibling"], "{at}");
            assert_eq!(json(&view::next_visible_sibling(&t, id, &v)), p["nextVisibleSibling"], "{at}");
            assert_eq!(view::depth(&t, id, &v), n(&p["depth"]), "{at}: depth");
        }
        for p in c["pairs"].as_array().unwrap() {
            assert_eq!(json(&view::select_between(&t, s(&p["a"]), s(&p["b"]), &v)), p["between"], "case {i}: {p}");
        }
        let norm = &c["normalize"];
        assert_eq!(json(&view::normalize_block(&t, &de::<Vec<NodeId>>(&norm["ids"]))), norm["out"], "case {i}");
        let port = &c["portable"];
        let id = s(&port["id"]);
        assert_eq!(json(&cmd::to_portable(&t, id, None)), port["plain"], "case {i}: to_portable");
        assert_eq!(json(&cmd::to_portable(&t, id, Some(&v))), port["shown"], "case {i}: to_portable in view");
    }
}

#[test]
fn text_edits_match_ts() {
    let fx = fixture();
    for e in fx["edits"].as_array().unwrap() {
        let (str, a, b, d) = (s(&e["str"]), n(&e["s"]), n(&e["e"]), s(&e["d"]));
        assert_eq!(json(&cmd::toggle_wrap(str, a, b, d)), e["wrap"], "toggle_wrap {e}");
        assert_eq!(json(&cmd::wrap_link(str, a, b)), e["link"], "wrap_link {e}");
    }
}
