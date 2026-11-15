//! Internal links between items: parsing app URLs, backlinks, and search & replace over a subtree. Port of
//! `packages/core/src/refs.ts`.

use std::collections::HashMap;

use serde_json::Value;

use crate::inline::{canonicalize, extract_links};
use crate::tree::{NodeId, Props, Tree, ROOT};
use crate::tx::Tx;

/// `application/x-www-form-urlencoded` decoding of one name or value, as `URLSearchParams` does it: `+` is a
/// space, `%XX` a byte, the bytes UTF-8 (invalid sequences become U+FFFD).
fn form_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                out.push(hex(b[i + 1]) * 16 + hex(b[i + 2]));
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The first value of `key` in a query string, as `new URLSearchParams(query).get(key)`.
fn query_get(query: &str, key: &str) -> Option<String> {
    let query = query.strip_prefix('?').unwrap_or(query);
    query.split('&').filter(|p| !p.is_empty()).find_map(|p| {
        let (k, v) = p.split_once('=').unwrap_or((p, ""));
        (form_decode(k) == key).then(|| form_decode(v))
    })
}

/// An app-internal URL (`#d=<doc>&z=<item>`, optionally after origin/path).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ItemUrl {
    pub doc: String,
    pub zoom: NodeId,
}

/// `{doc, zoom}` of an app-internal URL; `zoom` is the root when the URL has none.
pub fn parse_item_url(url: &str) -> Option<ItemUrl> {
    let (_, frag) = url.split_once('#')?;
    let doc = query_get(frag, "d").filter(|d| !d.is_empty())?;
    Some(ItemUrl { doc, zoom: query_get(frag, "z").unwrap_or_else(|| ROOT.to_owned()) })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Backlink {
    pub doc: String,
    pub id: NodeId,
}

/// A document's tree, for the link lookups that span documents.
pub struct DocTree<'a> {
    pub doc: &'a str,
    pub tree: &'a Tree,
}

fn texts(props: &Props) -> impl Iterator<Item = &str> {
    ["c", "n"].into_iter().filter_map(|k| props.get(k).and_then(Value::as_str))
}

/// Items of `src` linking to item `zoom` of `doc`.
fn links_to(src: &str, doc: &str, zoom: &str) -> bool {
    src.contains('#')
        && extract_links(src).iter().any(|l| parse_item_url(&l.url).is_some_and(|u| u.doc == doc && u.zoom == zoom))
}

/// Attached items (in `trees`) whose text or note links to item `zoom` of `doc`, in document order.
pub fn find_backlinks(trees: &[DocTree<'_>], doc: &str, zoom: &str) -> Vec<Backlink> {
    fn walk(tree: &Tree, d: &str, id: &str, doc: &str, zoom: &str, out: &mut Vec<Backlink>) {
        for c in tree.get(id).map(|n| n.children.as_slice()).unwrap_or_default() {
            let Some(n) = tree.get(c) else { continue };
            let hit = texts(&n.props).any(|src| links_to(src, doc, zoom));
            if hit && !(d == doc && c == zoom) {
                out.push(Backlink { doc: d.to_owned(), id: c.clone() });
            }
            walk(tree, d, c, doc, zoom, out);
        }
    }
    let mut out = Vec::new();
    for t in trees {
        walk(t.tree, t.doc, ROOT, doc, zoom, &mut out);
    }
    out
}

/// Number of backlinks per item of `doc` (for the badge next to bullets).
pub fn backlink_counts(trees: &[DocTree<'_>], doc: &str) -> HashMap<NodeId, usize> {
    let mut out: HashMap<NodeId, usize> = HashMap::new();
    for t in trees {
        for n in t.tree.nodes() {
            for src in texts(&n.props) {
                if !src.contains('#') {
                    continue;
                }
                for l in extract_links(src) {
                    if let Some(u) = parse_item_url(&l.url) {
                        if u.doc == doc && !(t.doc == doc && u.zoom == n.id) && t.tree.is_attached(&n.id) {
                            *out.entry(u.zoom).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    out
}

/// Non-overlapping occurrences of `find` in `s`, left to right, as UTF-16 ranges (JS `/find/g`, `/find/gi`).
fn occurrences(s: &[u16], find: &[u16], match_case: bool) -> Vec<(usize, usize)> {
    let key = |u: u16| if match_case { u } else { canonicalize(u) };
    let needle: Vec<u16> = find.iter().map(|&u| key(u)).collect();
    let mut out = Vec::new();
    let mut k = 0;
    while !needle.is_empty() && k + needle.len() <= s.len() {
        if s[k..k + needle.len()].iter().zip(&needle).all(|(&a, &b)| key(a) == b) {
            out.push((k, k + needle.len()));
            k += needle.len();
        } else {
            k += 1;
        }
    }
    out
}

fn each_in_scope(tree: &Tree, scope: &str, f: &mut dyn FnMut(&str)) {
    fn walk(tree: &Tree, id: &str, f: &mut dyn FnMut(&str)) {
        for c in tree.get(id).map(|n| n.children.as_slice()).unwrap_or_default() {
            f(c);
            walk(tree, c, f);
        }
    }
    if scope != ROOT && tree.contains(scope) {
        f(scope);
    }
    walk(tree, scope, f);
}

/// Occurrences of `find` in the text and notes of the items under `scope` (inclusive unless it is the root).
pub fn count_matches(tree: &Tree, scope: &str, find: &str, match_case: bool) -> usize {
    if find.is_empty() {
        return 0;
    }
    let find: Vec<u16> = find.encode_utf16().collect();
    let mut n = 0;
    each_in_scope(tree, scope, &mut |id| {
        for src in texts(&tree.get(id).expect("in scope").props) {
            n += occurrences(&src.encode_utf16().collect::<Vec<_>>(), &find, match_case).len();
        }
    });
    n
}

/// Replace every occurrence in the scope; returns the number replaced.
pub fn replace_all(tx: &mut Tx<'_>, scope: &str, find: &str, replace: &str, match_case: bool) -> usize {
    if find.is_empty() {
        return 0;
    }
    let find16: Vec<u16> = find.encode_utf16().collect();
    let rep16: Vec<u16> = replace.encode_utf16().collect();
    let mut ids = Vec::new();
    each_in_scope(tx.tree, scope, &mut |id| ids.push(id.to_owned()));
    let mut n = 0;
    for id in ids {
        let mut patch = Props::new();
        for k in ["c", "n"] {
            let Some(v) = tx.tree.get(&id).and_then(|n| n.props.get(k)).and_then(Value::as_str) else { continue };
            let s: Vec<u16> = v.encode_utf16().collect();
            let found = occurrences(&s, &find16, match_case);
            if found.is_empty() {
                continue;
            }
            n += found.len();
            let mut out: Vec<u16> = Vec::with_capacity(s.len());
            let mut last = 0;
            for (a, b) in found {
                out.extend_from_slice(&s[last..a]);
                out.extend_from_slice(&rep16);
                last = b;
            }
            out.extend_from_slice(&s[last..]);
            patch.insert(k.to_owned(), Value::String(String::from_utf16_lossy(&out)));
        }
        if !patch.is_empty() {
            tx.set(&id, patch);
        }
    }
    n
}
