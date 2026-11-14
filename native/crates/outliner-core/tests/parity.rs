//! Checks the Rust port against what the TS core does on the same random input.
//!
//! The fixture is recorded by `fixtures/gen.ts` (not committed; it is regenerated from `packages/core`):
//!
//!     node native/crates/outliner-core/fixtures/gen.ts

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use outliner_core::{compact, DocSession, Op, SerializedTree, SyncRequest, SyncResponse, Tree};
use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/core.json");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("{path} is missing: run `node native/crates/outliner-core/fixtures/gen.ts`"));
    serde_json::from_str(&text).expect("fixture parses")
}

fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn op(v: &Value) -> Op {
    serde_json::from_value(v.clone()).expect("op parses")
}

fn ops(v: &Value) -> Vec<Op> {
    serde_json::from_value(v.clone()).expect("ops parse")
}

/// Compare ops as JSON values, so prop key order does not matter but every field does.
fn json<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("serializable")
}

/// TS keeps a property that was absent as `null` in a `set` inverse, and drops `undefined`: same as ours.
#[test]
fn tree_apply_matches_ts() {
    let fx = fixture();
    for (ci, case) in fx["tree"].as_array().unwrap().iter().enumerate() {
        let mut t = Tree::default();
        for (si, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let inv = t.apply(&op(&step["op"]));
            assert_eq!(
                inv.as_ref().map(json).unwrap_or(Value::Null),
                step["inverse"],
                "case {ci} step {si}: inverse of {}",
                step["op"]
            );
            assert_eq!(fnv(&t.canonical()), step["canonical"].as_str().unwrap(), "case {ci} step {si}: tree differs");
        }
        assert_eq!(t.canonical(), case["canonical"].as_str().unwrap(), "case {ci}: canonical string");

        // the serialized tree holds the same nodes (order aside), and round-trips
        let want: SerializedTree = serde_json::from_value(case["final"].clone()).unwrap();
        let mut got = t.to_serialized().nodes;
        let mut want_nodes = want.nodes.clone();
        got.sort_by(|a, b| a.id.cmp(&b.id));
        want_nodes.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(got, want_nodes, "case {ci}: serialized nodes");
        assert_eq!(Tree::from_serialized(&want).canonical(), t.canonical());
    }
}

#[test]
fn purge_matches_ts() {
    let fx = fixture();
    for (ci, case) in fx["tree"].as_array().unwrap().iter().enumerate() {
        let want: SerializedTree = serde_json::from_value(case["final"].clone()).unwrap();
        let mut t = Tree::from_serialized(&want);
        // gen.ts purges with `clock - 200_000`, where clock is the time of the last step
        let last_ts = case["steps"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find_map(|s| s["op"]["ts"].as_i64())
            .unwrap();
        t.purge_detached(last_ts - 200_000);
        let mut ids: Vec<String> = t.nodes().map(|n| n.id.clone()).collect();
        ids.sort();
        assert_eq!(serde_json::to_string(&ids).unwrap(), case["purged"].as_str().unwrap(), "case {ci}: purge");
    }
}

#[test]
fn session_matches_ts() {
    let fx = fixture();
    for (ci, case) in fx["session"].as_array().unwrap().iter().enumerate() {
        let clock = Arc::new(AtomicI64::new(0));
        let mut clients: Vec<DocSession<i64>> = (0..2)
            .map(|_| {
                let mut s = DocSession::new(None);
                let c = clock.clone();
                s.set_clock(move || c.load(Ordering::SeqCst));
                s
            })
            .collect();
        for (ai, a) in case["actions"].as_array().unwrap().iter().enumerate() {
            let at = format!("case {ci} action {ai} ({})", a["a"]);
            let client = a["client"].as_u64().map(|c| c as usize);
            clock.store(a["now"].as_i64().expect("action time"), Ordering::SeqCst);
            match a["a"].as_str().unwrap() {
                "tick" => {}
                "transact" => {
                    let s = &mut clients[client.unwrap()];
                    let ops = ops(&a["ops"]);
                    let (applied, _) = s.transact(
                        ops,
                        a["before"].as_i64().unwrap(),
                        a["after"].as_i64().unwrap(),
                        a["undoable"].as_bool().unwrap(),
                    );
                    assert_eq!(json(&applied), a["applied"], "{at}: applied ops");
                }
                kind @ ("undo" | "redo") => {
                    let s = &mut clients[client.unwrap()];
                    let (sel, _) = if kind == "undo" { s.undo() } else { s.redo() };
                    assert_eq!(sel.map(Value::from).unwrap_or(Value::Null), a["sel"], "{at}: selection");
                    assert_eq!(s.undone_first.map(Value::from).unwrap_or(Value::Null), a["undoneFirst"], "{at}");
                }
                "sync" => {
                    let s = &mut clients[client.unwrap()];
                    let req = s.begin_sync().expect("no sync in flight");
                    let want: SyncRequest = serde_json::from_value(a["request"].clone()).unwrap();
                    assert_eq!(json(&req), json(&want), "{at}: sync request");
                    let res: SyncResponse = serde_json::from_value(a["response"].clone()).unwrap();
                    s.end_sync(&res);
                }
                "fail" => {
                    let s = &mut clients[client.unwrap()];
                    let req = s.begin_sync().expect("no sync in flight");
                    let want: SyncRequest = serde_json::from_value(a["request"].clone()).unwrap();
                    assert_eq!(json(&req), json(&want), "{at}: sync request");
                    s.fail_sync();
                }
                "check" => {
                    let s = &clients[client.unwrap()];
                    let canonical = s.live.canonical();
                    let want = a["canonical"].as_str().unwrap();
                    if want.len() == 16 {
                        assert_eq!(fnv(&canonical), want, "{at}: live tree");
                    }
                    assert_eq!(s.can_undo(), a["canUndo"].as_bool().unwrap(), "{at}: can_undo");
                    assert_eq!(s.can_redo(), a["canRedo"].as_bool().unwrap(), "{at}: can_redo");
                    assert_eq!(s.dirty(), a["dirty"].as_bool().unwrap(), "{at}: dirty");
                    if !a["snapshot"].is_null() {
                        let mut got = json(&s.snapshot());
                        let mut want = a["snapshot"].clone();
                        for v in [&mut got, &mut want] {
                            v["confirmed"]["nodes"]
                                .as_array_mut()
                                .unwrap()
                                .sort_by(|x, y| x["id"].as_str().cmp(&y["id"].as_str()));
                        }
                        assert_eq!(got, want, "{at}: snapshot");
                    }
                }
                other => panic!("unknown action {other}"),
            }
        }
    }
}

#[test]
fn compact_matches_ts() {
    let fx = fixture();
    for case in fx["compact"].as_array().unwrap() {
        assert_eq!(json(&compact(&ops(&case["ops"]))), case["out"]);
    }
}
