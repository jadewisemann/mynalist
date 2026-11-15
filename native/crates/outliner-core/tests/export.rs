//! Checks the exporters (text, HTML, OPML, portable copies) and the inline HTML renderer against what the TS
//! core writes. Render cases are recorded in several time zones (date labels).
//!
//!     node native/crates/outliner-core/fixtures/gen-export.ts
//!
//! A single test: the zone cases set `TZ` for the process.

mod common;

use common::*;
use outliner_core::export::*;
use outliner_core::render::{render_inline, RenderOptions};
use outliner_core::NodeId;
use serde_json::Value;

fn include(id: &str) -> bool {
    id[1..].parse::<i64>().map_or(true, |n| n % 3 != 0)
}

fn link_title(url: &str) -> Option<String> {
    (url.encode_utf16().count() % 2 == 1).then(|| format!("Title of {url}"))
}

fn is_internal(url: &str) -> bool {
    url.starts_with('#')
}

fn opt_bool(v: &Value) -> Option<bool> {
    v.as_bool()
}

fn opt_string(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn render_cases(c: &mut Check, cases: &[Value]) {
    for case in cases {
        let o = &case["o"];
        let cb = o["callbacks"].as_bool().unwrap();
        let opts = RenderOptions {
            now: o["now"].as_i64(),
            hour12: opt_bool(&o["hour12"]),
            date_format: opt_string(&o["dateFormat"]),
            date_format_no_time: opt_string(&o["dateFormatNoTime"]),
            mark_overdue: o["markOverdue"].as_bool().unwrap(),
            inline_images: o["inlineImages"].as_bool().unwrap(),
            highlight: arr(&o["highlight"]).iter().map(|v| s(v).to_owned()).collect(),
            is_internal: if cb { Some(&is_internal) } else { None },
            link_title: if cb { Some(&link_title) } else { None },
            clean: o["clean"].as_bool().unwrap(),
        };
        c.eq("renderInline", case, json(&render_inline(s(&case["src"]), &opts)), &case["html"]);
    }
}

#[test]
fn export_matches_ts() {
    let fx = fixture("export", "gen-export.ts");
    let mut c = Check::default();
    for a in arr(&fx["attrs"]) {
        c.eq("escapeXmlAttr", a, json(&escape_xml_attr(s(&a[0]))), &a[1]);
    }
    for case in arr(&fx["trees"]) {
        let t = tree(&case["tree"]);
        let sel: Vec<NodeId> = arr(&case["sel"]).iter().map(|v| s(v).to_owned()).collect();
        let sc = &case["scope"];
        let scope = ExportScope {
            visible_only: sc["visibleOnly"].as_bool().unwrap_or(false),
            show_checked: opt_bool(&sc["showChecked"]),
            notes: match sc["notes"].as_str() {
                Some("first") => NotesMode::First,
                Some("hide") => NotesMode::Hide,
                _ => NotesMode::Show,
            },
            include: if sc["include"].as_bool() == Some(true) { Some(&include) } else { None },
        };
        let to = &case["text"];
        let text = TextExportOptions {
            indent: match to["indent"].as_str() {
                Some("asterisks") => IndentStyle::Asterisks,
                Some("dashes") => IndentStyle::Dashes,
                Some("none") => IndentStyle::None,
                _ => IndentStyle::Spaces,
            },
            include_notes: opt_bool(&to["includeNotes"]),
            top_numbered: opt_bool(&to["topNumbered"]),
        };
        let ho = &case["html"];
        let html = HtmlExportOptions {
            include_notes: opt_bool(&ho["includeNotes"]),
            top_numbered: opt_bool(&ho["topNumbered"]),
            document: ho["document"].as_bool().unwrap(),
            title: opt_string(&ho["title"]),
        };
        let oo = &case["opml"];
        let opml = OpmlExportOptions {
            title: opt_string(&oo["title"]),
            source: opt_string(&oo["source"]),
            owner_name: opt_string(&oo["ownerName"]),
            owner_email: opt_string(&oo["ownerEmail"]),
        };
        let items = portable_from_tree(&t, &sel, &scope);
        c.eq("portableFromTree", case, json(&items), &case["portable"]);
        c.eq("exportText", case, json(&export_text(&t, &sel, &scope, &text)), &case["exportText"]);
        c.eq("exportHtml", case, json(&export_html(&t, &sel, &scope, &html)), &case["exportHtml"]);
        let xml = export_opml(&t, &sel, &scope, &opml);
        c.eq("exportOpml", case, json(&xml), &case["exportOpml"]);
        c.eq("portableToText", case, json(&portable_to_text(&items, &text)), &case["toText"]);
        c.eq("portableToHtml", case, json(&portable_to_html(&items, &html)), &case["toHtml"]);
        c.eq("portableToOpml", case, json(&portable_to_opml(&items, &opml)), &case["toOpml"]);
        c.eq("opmlToPortable", case, json(&opml_to_portable(&xml)), &case["roundTrip"]);
    }
    zones(&mut c, &fx["zones"], render_cases);
    c.done("export");
}
