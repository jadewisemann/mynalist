//! Export: plain text, HTML and OPML 2.0. Port of `packages/core/src/export.ts`.
//!
//! Pipeline: tree → portable items (applying "visible only" filters) → serializer. The serializers also take
//! portable items directly, which is what the clipboard uses.

use std::collections::HashSet;

use serde_json::Value;

use crate::commands::Portable;
use crate::paste::{parse_opml, OpmlDocument};
use crate::render::{escape_html, render_inline, RenderOptions};
use crate::tree::{is_falsy, NodeId, Tree};

pub const APP_NAME: &str = "Outliner";
/// `<flavor>` written to (and recognized in) OPML heads
pub const OPML_FLAVOR: &str = "outliner";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotesMode {
    #[default]
    Show,
    First,
    Hide,
}

#[derive(Default)]
pub struct ExportScope<'a> {
    /// skip children of collapsed items, hidden checked items and hidden notes
    pub visible_only: bool,
    /// with `visible_only`: false hides checked items and their subtrees (default true)
    pub show_checked: Option<bool>,
    /// with `visible_only`: note display mode (default show)
    pub notes: NotesMode,
    /// with `visible_only`: extra filter (e.g. active search); false skips the item and its subtree
    pub include: Option<&'a dyn Fn(&str) -> bool>,
}

fn truthy(p: &crate::tree::Props, k: &str) -> bool {
    p.get(k).is_some_and(|v| !is_falsy(v))
}

fn str_of<'a>(p: &'a crate::tree::Props, k: &str) -> &'a str {
    p.get(k).and_then(Value::as_str).unwrap_or("")
}

/// A number prop as JS prints it in a template string, when it is in `lo..=hi` (headings, color labels).
fn level(p: &crate::tree::Props, k: &str, lo: f64, hi: f64) -> Option<String> {
    let f = p.get(k)?.as_f64()?;
    (f != 0.0 && f >= lo && f <= hi).then(|| if f.fract() == 0.0 { format!("{}", f as i64) } else { format!("{f}") })
}

/// Copy subtrees into portable items, applying the visible-only rules.
pub fn portable_from_tree(tree: &Tree, ids: &[NodeId], opts: &ExportScope<'_>) -> Vec<Portable> {
    let vis = opts.visible_only;
    let notes = if vis { opts.notes } else { NotesMode::Show };
    fn conv(tree: &Tree, id: &str, opts: &ExportScope<'_>, vis: bool, notes: NotesMode) -> Option<Portable> {
        let n = tree.get(id)?;
        if vis && opts.show_checked == Some(false) && truthy(&n.props, "ck") {
            return None;
        }
        if vis && opts.include.is_some_and(|f| !f(id)) {
            return None;
        }
        let mut props = n.props.clone();
        if let Some(note) = props.get("n").and_then(Value::as_str).map(str::to_owned) {
            match notes {
                NotesMode::Hide => {
                    props.remove("n");
                }
                NotesMode::First => {
                    props.insert("n".into(), Value::String(note.split('\n').next().unwrap_or("").to_owned()));
                }
                NotesMode::Show => {}
            }
            if props.get("n").and_then(Value::as_str) == Some("") {
                props.remove("n");
            }
        }
        let kids: &[NodeId] = if vis && n.collapsed { &[] } else { &n.children };
        let children = kids.iter().filter_map(|c| conv(tree, c, opts, vis, notes)).collect();
        Some(Portable { props, collapsed: Some(n.collapsed && !n.children.is_empty()), children })
    }
    ids.iter().filter_map(|id| conv(tree, id, opts, vis, notes)).collect()
}

/// True if the items share a parent whose children are numbered.
fn parent_numbered(tree: &Tree, ids: &[NodeId]) -> bool {
    let parents: HashSet<Option<Option<&NodeId>>> = ids.iter().map(|id| tree.get(id).map(|n| n.parent.as_ref())).collect();
    if parents.len() != 1 {
        return false;
    }
    match parents.into_iter().next() {
        Some(Some(Some(p))) => tree.get(p).is_some_and(|n| truthy(&n.props, "num")),
        _ => false,
    }
}

// ---- plain text -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndentStyle {
    /// 4 spaces per level
    #[default]
    Spaces,
    /// spaces and a "* " prefix
    Asterisks,
    /// spaces and a "- " prefix
    Dashes,
    None,
}

#[derive(Debug, Clone, Default)]
pub struct TextExportOptions {
    pub indent: IndentStyle,
    /// include notes (default true)
    pub include_notes: Option<bool>,
    /// the top-level items are children of a numbered parent (default: from the tree)
    pub top_numbered: Option<bool>,
}

/// One line per item; numbered children get "N. " prefixes; note lines (and extra lines of multi-line content)
/// follow, aligned with the item text.
pub fn portable_to_text(items: &[Portable], opts: &TextExportOptions) -> String {
    fn walk(list: &[Portable], depth: usize, numbered: bool, opts: &TextExportOptions, out: &mut Vec<String>) {
        for (i, it) in list.iter().enumerate() {
            let ind = if opts.indent == IndentStyle::None { String::new() } else { "    ".repeat(depth) };
            let prefix = if numbered {
                format!("{}. ", i + 1)
            } else {
                match opts.indent {
                    IndentStyle::Asterisks => "* ".into(),
                    IndentStyle::Dashes => "- ".into(),
                    _ => String::new(),
                }
            };
            let pad = format!("{ind}{}", " ".repeat(prefix.len()));
            let mut lines = str_of(&it.props, "c").split('\n');
            out.push(format!("{ind}{prefix}{}", lines.next().unwrap_or("")));
            for l in lines {
                out.push(format!("{pad}{l}"));
            }
            let note = str_of(&it.props, "n");
            if !note.is_empty() && opts.include_notes != Some(false) {
                for l in note.split('\n') {
                    out.push(format!("{pad}{l}"));
                }
            }
            walk(&it.children, depth + 1, truthy(&it.props, "num"), opts, out);
        }
    }
    let mut out = Vec::new();
    walk(items, 0, opts.top_numbered == Some(true), opts, &mut out);
    out.join("\n")
}

// ---- HTML ----------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct HtmlExportOptions {
    /// include notes (default true)
    pub include_notes: Option<bool>,
    pub top_numbered: Option<bool>,
    /// wrap in a complete HTML document
    pub document: bool,
    pub title: Option<String>,
}

/// Nested `<ul>`/`<ol>` (ol when the parent is numbered); headings as `<hN>`, checked items wrapped in `<del>`,
/// notes after `<br>`. Inline formatting rendered as clean HTML.
pub fn portable_to_html(items: &[Portable], opts: &HtmlExportOptions) -> String {
    let clean = RenderOptions { clean: true, ..Default::default() };
    fn list(items: &[Portable], numbered: bool, opts: &HtmlExportOptions, clean: &RenderOptions<'_>) -> String {
        if items.is_empty() {
            return String::new();
        }
        let tag = if numbered { "ol" } else { "ul" };
        let mut s = format!("<{tag}>");
        for it in items {
            let ck = truthy(&it.props, "ck");
            let mut body = render_inline(str_of(&it.props, "c"), clean);
            if ck {
                body = format!("<del>{body}</del>");
            }
            if let Some(h) = level(&it.props, "h", 1.0, 3.0) {
                body = format!("<h{h}>{body}</h{h}>");
            }
            let note = str_of(&it.props, "n");
            if !note.is_empty() && opts.include_notes != Some(false) {
                let mut nb = render_inline(note, clean);
                if ck {
                    nb = format!("<del>{nb}</del>");
                }
                body += &format!("<br>{nb}");
            }
            s += &format!("<li>{body}{}</li>", list(&it.children, truthy(&it.props, "num"), opts, clean));
        }
        s + &format!("</{tag}>")
    }
    let html = list(items, opts.top_numbered == Some(true), opts, &clean);
    if !opts.document {
        return html;
    }
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\n{html}\n</body></html>\n",
        escape_html(opts.title.as_deref().unwrap_or(""))
    )
}

// ---- OPML ----------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct OpmlExportOptions {
    pub title: Option<String>,
    /// source URL of the document
    pub source: Option<String>,
    pub owner_name: Option<String>,
    pub owner_email: Option<String>,
}

/// XML attribute escaping (newlines/tabs as character references).
pub fn escape_xml_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out += "&amp;",
            '<' => out += "&lt;",
            '>' => out += "&gt;",
            '"' => out += "&quot;",
            '\n' => out += "&#10;",
            '\r' => out += "&#13;",
            '\t' => out += "&#9;",
            _ => out.push(c),
        }
    }
    out
}

fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// OPML 2.0 with our attributes (text, _note, checkbox, complete, heading, colorLabel, listStyle, collapsed).
pub fn portable_to_opml(items: &[Portable], opts: &OpmlExportOptions) -> String {
    let mut lines: Vec<String> = vec![
        r#"<?xml version="1.0" encoding="utf-8"?>"#.into(),
        r#"<opml version="2.0">"#.into(),
        "  <head>".into(),
        format!("    <title>{}</title>", xml_text(opts.title.as_deref().unwrap_or(""))),
        format!("    <flavor>{OPML_FLAVOR}</flavor>"),
    ];
    let mut head = |tag: &str, v: &Option<String>| {
        if let Some(v) = v.as_deref().filter(|v| !v.is_empty()) {
            lines.push(format!("    <{tag}>{}</{tag}>", xml_text(v)));
        }
    };
    head("source", &opts.source);
    head("ownerName", &opts.owner_name);
    head("ownerEmail", &opts.owner_email);
    lines.push("  </head>".into());
    lines.push("  <body>".into());
    fn walk(list: &[Portable], depth: usize, lines: &mut Vec<String>) {
        let ind = "  ".repeat(depth + 2);
        for it in list {
            let p = &it.props;
            let mut a = format!(" text=\"{}\"", escape_xml_attr(str_of(p, "c")));
            if !str_of(p, "n").is_empty() {
                a += &format!(" _note=\"{}\"", escape_xml_attr(str_of(p, "n")));
            }
            if truthy(p, "cb") {
                a += " checkbox=\"true\"";
            }
            if truthy(p, "ck") {
                a += " complete=\"true\"";
            }
            if let Some(h) = level(p, "h", 1.0, 3.0) {
                a += &format!(" heading=\"{h}\"");
            }
            if let Some(cl) = level(p, "cl", 1.0, 6.0) {
                a += &format!(" colorLabel=\"{cl}\"");
            }
            if truthy(p, "num") {
                a += " listStyle=\"arabic\"";
            }
            if it.collapsed == Some(true) && !it.children.is_empty() {
                a += " collapsed=\"true\"";
            }
            if it.children.is_empty() {
                lines.push(format!("{ind}<outline{a} />"));
            } else {
                lines.push(format!("{ind}<outline{a}>"));
                walk(&it.children, depth + 1, lines);
                lines.push(format!("{ind}</outline>"));
            }
        }
    }
    walk(items, 0, &mut lines);
    lines.push("  </body>".into());
    lines.push("</opml>".into());
    lines.join("\n") + "\n"
}

/// Import: OPML (any flavor) → title + portable items; `None` if not OPML.
pub fn opml_to_portable(xml: &str) -> Option<OpmlDocument> {
    parse_opml(xml)
}

// ---- tree convenience wrappers ---------------------------------------------------------------

pub fn export_text(tree: &Tree, ids: &[NodeId], scope: &ExportScope<'_>, opts: &TextExportOptions) -> String {
    let items = portable_from_tree(tree, ids, scope);
    let top = opts.top_numbered.unwrap_or_else(|| parent_numbered(tree, ids));
    portable_to_text(&items, &TextExportOptions { top_numbered: Some(top), ..opts.clone() })
}

pub fn export_html(tree: &Tree, ids: &[NodeId], scope: &ExportScope<'_>, opts: &HtmlExportOptions) -> String {
    let items = portable_from_tree(tree, ids, scope);
    let top = opts.top_numbered.unwrap_or_else(|| parent_numbered(tree, ids));
    portable_to_html(&items, &HtmlExportOptions { top_numbered: Some(top), ..opts.clone() })
}

pub fn export_opml(tree: &Tree, ids: &[NodeId], scope: &ExportScope<'_>, opts: &OpmlExportOptions) -> String {
    portable_to_opml(&portable_from_tree(tree, ids, scope), opts)
}
