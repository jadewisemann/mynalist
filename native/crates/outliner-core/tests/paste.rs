//! Checks the clipboard / import parsers against what the TS core does: indented text, entities, the markup
//! tokenizer, HTML → text, OPML and the clipboard priority.
//!
//!     node native/crates/outliner-core/fixtures/gen-paste.ts

mod common;

use common::*;
use outliner_core::paste::*;

#[test]
fn paste_matches_ts() {
    let fx = fixture("paste", "gen-paste.ts");
    let mut c = Check::default();
    for p in arr(&fx["plain"]) {
        c.eq("parsePlainText", p, json(&parse_plain_text(s(&p[0]))), &p[1]);
        c.eq("normalizeNewlines", p, json(&normalize_newlines(s(&p[0]))), &p[2]);
    }
    for e in arr(&fx["entities"]) {
        c.eq("decodeEntities", e, json(&decode_entities(s(&e[0]))), &e[1]);
    }
    for m in arr(&fx["markup"]) {
        c.eq("tokenizeMarkup", m, json(&tokenize_markup(s(&m["s"]))), &m["tokens"]);
        c.eq("htmlToText", m, json(&html_to_text(s(&m["s"]))), &m["text"]);
    }
    for o in arr(&fx["opmls"]) {
        c.eq("isOpml", o, json(&is_opml(s(&o["s"]))), &o["isOpml"]);
        c.eq("parseOpml", o, json(&parse_opml(s(&o["s"]))), &o["doc"]);
    }
    for p in arr(&fx["opmlProbes"]) {
        c.eq("isOpml probe", p, json(&is_opml(s(&p[0]))), &p[1]);
    }
    for k in arr(&fx["clipboard"]) {
        let d = &k["d"];
        let get = |key: &str| d[key].as_str().map(str::to_owned);
        let data = ClipboardData { plain: get("text/plain"), html: get("text/html"), opml: get("text/opml") };
        c.eq("parseClipboard", k, json(&parse_clipboard(&data)), &k["r"]);
    }
    c.done("paste");
}
