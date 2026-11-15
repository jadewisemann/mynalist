//! Shared by the replay tests: fixture loading and a checker that counts and prints mismatches.
#![allow(dead_code)]

use outliner_core::{SerializedTree, Tree};
use serde_json::Value;

/// Reads `fixtures/<name>.json`, recorded by `fixtures/<generator>`.
pub fn fixture(name: &str, generator: &str) -> Value {
    let path = format!("{}/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{path} is missing: run `node native/crates/outliner-core/fixtures/{generator}`"));
    serde_json::from_str(&text).expect("fixture parses")
}

pub fn json<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("serializable")
}

pub fn de<T: serde::de::DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}

pub fn tree(v: &Value) -> Tree {
    Tree::from_serialized(&de::<SerializedTree>(v))
}

pub fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

pub fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not an integer: {v}"))
}

pub fn arr(v: &Value) -> &Vec<Value> {
    v.as_array().unwrap_or_else(|| panic!("not an array: {v}"))
}

/// Numbers as JS writes them: whole numbers as integers, so `-25.0` equals TS's `-25`.
pub fn norm(v: Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9e15 => Value::from(f as i64),
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, norm(v))).collect()),
        v => v,
    }
}

/// Compares and reports the case on a mismatch (all mismatches are counted, the first few printed).
#[derive(Default)]
pub struct Check {
    pub failures: usize,
    pub checks: usize,
}

impl Check {
    pub fn eq(&mut self, what: &str, case: &Value, got: Value, want: &Value) {
        self.checks += 1;
        if norm(got.clone()) != norm(want.clone()) {
            self.failures += 1;
            if self.failures <= 12 {
                eprintln!("MISMATCH {what}\n  case: {}\n  got:  {got}\n  want: {want}", trunc(case));
            }
        }
    }

    pub fn add(&mut self, other: (usize, usize)) {
        self.failures += other.0;
        self.checks += other.1;
    }

    pub fn done(&self, what: &str) {
        eprintln!("{what}: {} checks", self.checks);
        assert_eq!(self.failures, 0, "{} of {} checks differ from TS", self.failures, self.checks);
        assert!(self.checks > 0, "the fixture has cases");
    }
}

pub fn trunc(v: &Value) -> String {
    let t = v.to_string();
    if t.len() > 600 { format!("{}…", &t[..t.char_indices().take_while(|(k, _)| *k < 600).last().map_or(0, |(k, _)| k)]) } else { t }
}

/// Runs `f` over each zone's cases with `TZ` set (in a fresh thread: chrono caches the zone per thread).
pub fn zones(c: &mut Check, zones: &Value, f: fn(&mut Check, &[Value])) {
    let zones = zones.as_object().expect("zones");
    for (zone, cases) in zones {
        std::env::set_var("TZ", zone);
        let cases = arr(cases).clone();
        let r = std::thread::spawn(move || {
            let mut c = Check::default();
            f(&mut c, &cases);
            (c.failures, c.checks)
        })
        .join()
        .unwrap();
        eprintln!("{zone}: {} checks, {} mismatches", r.1, r.0);
        c.add(r);
    }
    assert!(zones.len() >= 3, "the fixture has the time zones");
}
