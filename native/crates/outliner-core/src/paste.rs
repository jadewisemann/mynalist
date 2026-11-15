//! Clipboard / import parsing: indented plain text, HTML → indented markdown text, and OPML → portable item
//! trees. Port of `packages/core/src/paste.ts`: HTML and OPML go through a small tolerant tokenizer.
//!
//! The TS code indexes strings in UTF-16 units; here every cut falls on an ASCII character the scanners look
//! for, so byte offsets give the same strings. Known difference: TS looks entity names up in a plain object, so
//! names of `Object.prototype` members (`&constructor;`, `&toString;`) decode to their function's source text
//! there; here they stay as written.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::commands::Portable;
use crate::dates::{js_trim, js_ws};
use crate::tree::Props;

fn ws(c: char) -> bool {
    js_ws(c as u32)
}

/// The character at byte `i` (`None` at the end).
fn ch(s: &str, i: usize) -> Option<char> {
    s.get(i..).and_then(|r| r.chars().next())
}

/// Byte offset after the white space starting at `i`.
fn skip_ws(s: &str, mut i: usize) -> usize {
    while let Some(c) = ch(s, i).filter(|&c| ws(c)) {
        i += c.len_utf8();
    }
    i
}

fn trim_end_ws(s: &str) -> &str {
    s.trim_end_matches(ws)
}

fn lead_ws(s: &str) -> &str {
    &s[..s.len() - s.trim_start_matches(ws).len()]
}

fn trail_ws(s: &str) -> &str {
    &s[s.trim_end_matches(ws).len()..]
}

fn starts_with_ci(s: &str, at: usize, p: &str) -> bool {
    s.as_bytes().get(at..at + p.len()).is_some_and(|b| b.eq_ignore_ascii_case(p.as_bytes()))
}

fn find_ci(s: &str, from: usize, p: &str) -> Option<usize> {
    (from..=s.len().saturating_sub(p.len())).find(|&k| starts_with_ci(s, k, p))
}

/// JS `parseInt(s, 10)` for the integers that fit (saturating beyond).
fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start_matches(ws);
    let (neg, t) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits: &str = &t[..t.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return None;
    }
    let v = digits.bytes().fold(0i64, |a, d| a.saturating_mul(10).saturating_add(i64::from(d - b'0')));
    Some(if neg { -v } else { v })
}

// ---- plain text ------------------------------------------------------------------

/// CRLF/CR → LF.
pub fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn leading_spaces(l: &str) -> usize {
    l.bytes().take_while(|&b| b == b' ').count()
}

fn item(c: &str) -> Portable {
    let mut props = Props::new();
    if !c.is_empty() {
        props.insert("c".into(), Value::String(c.to_owned()));
    }
    Portable { props, collapsed: Some(false), children: Vec::new() }
}

/// Indented plain text → items: tabs become 4 spaces, the common indent is stripped, deeper lines nest under the
/// previous line, shallower lines pop to a matching level. Like Dynalist, list markers stay part of the text and
/// blank lines become empty items; only indentation carries structure. The first returned item is line 1 (meant
/// to be spliced into the item at the caret); a trailing newline yields a final empty item.
pub fn parse_plain_text(text: &str) -> Vec<Portable> {
    let norm = normalize_newlines(text).replace('\t', "    ");
    let lines: Vec<&str> = norm.split('\n').collect();
    let blank = |l: &str| js_trim(l).is_empty();
    let min = lines.iter().filter(|l| !blank(l)).map(|l| leading_spaces(l)).min().unwrap_or(0);
    let clean = |l: &str| if blank(l) { String::new() } else { l.trim_start_matches(' ').to_owned() };

    // A path of child indexes from the roots to each stack entry, with its indent; `list` = the entry's parent.
    struct Entry {
        path: Vec<usize>,
        indent: i64,
    }
    fn list_at<'a>(roots: &'a mut Vec<Portable>, path: &[usize]) -> &'a mut Vec<Portable> {
        let mut list = roots;
        for &k in path {
            list = &mut list[k].children;
        }
        list
    }
    let mut roots = vec![item(&clean(lines[0]))];
    let indent0 = if blank(lines[0]) { 0 } else { leading_spaces(lines[0]) as i64 - min as i64 };
    let mut stack = vec![Entry { path: vec![0], indent: indent0 }];
    for line in &lines[1..] {
        let top = stack.last().expect("stack");
        let indent = if blank(line) { top.indent } else { leading_spaces(line) as i64 - min as i64 };
        let it = item(&clean(line));
        if indent > top.indent {
            let path = top.path.clone();
            let kids = &mut list_at(&mut roots, &path[..path.len() - 1])[path[path.len() - 1]].children;
            kids.push(it);
            let mut p = path;
            p.push(kids.len() - 1);
            stack.push(Entry { path: p, indent });
            continue;
        }
        while stack.last().is_some_and(|e| e.indent > indent) {
            stack.pop();
        }
        let Some(sib) = stack.last_mut() else {
            roots.push(it);
            stack = vec![Entry { path: vec![roots.len() - 1], indent }];
            continue;
        };
        let parent = &sib.path[..sib.path.len() - 1];
        let list = list_at(&mut roots, parent);
        list.push(it);
        let mut p = parent.to_vec();
        p.push(list.len() - 1);
        *sib = Entry { path: p, indent };
    }
    roots
}

// ---- markup tokenizer ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "k", rename_all = "lowercase")]
pub enum MarkupToken {
    Open {
        name: String,
        /// attribute names lowercased, values decoded; the first of a repeated name wins
        attrs: BTreeMap<String, String>,
        #[serde(rename = "selfClose")]
        self_close: bool,
    },
    Close {
        name: String,
    },
    Text {
        v: String,
    },
}

const ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"), ("lt", "<"), ("gt", ">"), ("quot", "\""), ("apos", "'"), ("nbsp", "\u{a0}"), ("copy", "©"),
    ("reg", "®"), ("trade", "™"), ("hellip", "…"), ("mdash", "—"), ("ndash", "–"), ("lsquo", "‘"), ("rsquo", "’"),
    ("ldquo", "“"), ("rdquo", "”"), ("bull", "•"), ("middot", "·"), ("euro", "€"), ("pound", "£"), ("yen", "¥"),
    ("cent", "¢"), ("sect", "§"), ("deg", "°"), ("times", "×"), ("divide", "÷"), ("laquo", "«"), ("raquo", "»"),
    ("shy", ""), ("zwj", "\u{200d}"), ("zwnj", "\u{200c}"), ("ensp", "\u{2002}"), ("emsp", "\u{2003}"),
    ("thinsp", "\u{2009}"),
];

fn entity(name: &str) -> Option<&'static str> {
    ENTITIES.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}

/// Decode character references (`&#10;`, `&#x41;`) and the named entities above (a missing `;` is fine for
/// lowercase names).
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'&' {
            i += 1;
            continue;
        }
        // &(#x[0-9a-fA-F]+|#\d+|[a-zA-Z][a-zA-Z0-9]*);?
        let run = |from: usize, f: fn(&u8) -> bool| b[from..].iter().take_while(|c| f(c)).count();
        let (body_end, rep): (usize, Option<String>) = if b.get(i + 1) == Some(&b'#') && b.get(i + 2) == Some(&b'x') && run(i + 3, u8::is_ascii_hexdigit) > 0 {
            let e = i + 3 + run(i + 3, u8::is_ascii_hexdigit);
            (e, Some(char_ref(&s[i + 3..e], 16)))
        } else if b.get(i + 1) == Some(&b'#') && run(i + 2, u8::is_ascii_digit) > 0 {
            let e = i + 2 + run(i + 2, u8::is_ascii_digit);
            (e, Some(char_ref(&s[i + 2..e], 10)))
        } else if b.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
            (i + 1 + run(i + 1, u8::is_ascii_alphanumeric), None)
        } else {
            i += 1;
            continue;
        };
        let semi = b.get(body_end) == Some(&b';');
        let end = body_end + usize::from(semi);
        let rep = rep.or_else(|| {
            let e = &s[i + 1..body_end];
            let v = entity(e).or_else(|| entity(&e.to_lowercase()))?;
            (semi || entity(e).is_some()).then(|| v.to_owned())
        });
        if let Some(r) = rep {
            out += &s[last..i];
            out += &r;
            last = end;
        }
        i = end;
    }
    out + &s[last..]
}

fn char_ref(digits: &str, radix: u32) -> String {
    let cp = digits.chars().fold(0u64, |a, d| a.saturating_mul(u64::from(radix)).saturating_add(u64::from(d.to_digit(radix).unwrap_or(0))));
    match u32::try_from(cp).ok().and_then(char::from_u32) {
        Some(c) if cp != 0 => c.to_string(),
        _ => "\u{fffd}".to_owned(),
    }
}

const RAW_TEXT: &[&str] = &["script", "style", "textarea", "xmp"];

/// `</name\s*>` (any case) at or after `from`: (start, end).
fn find_raw_close(src: &str, from: usize, name: &str) -> Option<(usize, usize)> {
    let mut k = from;
    while let Some(at) = src[k..].find("</").map(|x| x + k) {
        if starts_with_ci(src, at + 2, name) {
            let e = skip_ws(src, at + 2 + name.len());
            if src.as_bytes().get(e) == Some(&b'>') {
                return Some((at, e + 1));
            }
        }
        k = at + 2;
    }
    None
}

/// Tolerant HTML/XML tokenizer. Tag and attribute names are lowercased; values decoded.
pub fn tokenize_markup(src: &str) -> Vec<MarkupToken> {
    let mut out = Vec::new();
    let b = src.as_bytes();
    let n = b.len();
    let mut i = 0;
    let mut text_start = 0;
    let flush = |out: &mut Vec<MarkupToken>, text_start: usize, to: usize| {
        if to > text_start {
            out.push(MarkupToken::Text { v: decode_entities(&src[text_start..to]) });
        }
    };
    while i < n {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if src[i..].starts_with("<!--") {
            flush(&mut out, text_start, i);
            i = src[i + 4..].find("-->").map_or(n, |e| i + 4 + e + 3);
            text_start = i;
            continue;
        }
        if src[i..].starts_with("<![CDATA[") {
            flush(&mut out, text_start, i);
            let e = src[i + 9..].find("]]>").map(|e| i + 9 + e);
            out.push(MarkupToken::Text { v: src[i + 9..e.unwrap_or(n)].to_owned() });
            i = e.map_or(n, |e| e + 3);
            text_start = i;
            continue;
        }
        let next = b.get(i + 1).copied();
        if next == Some(b'!') || next == Some(b'?') {
            flush(&mut out, text_start, i);
            i = src.get(i + 2..).and_then(|r| r.find('>')).map_or(n, |e| i + 2 + e + 1);
            text_start = i;
            continue;
        }
        // <(\/?)([a-zA-Z][a-zA-Z0-9:_-]*) within the next 64 units (all ASCII here)
        let closing = next == Some(b'/');
        let ns = i + 1 + usize::from(closing);
        if !b.get(ns).is_some_and(u8::is_ascii_alphabetic) {
            i += 1;
            continue;
        }
        let name_len = b[ns..].iter().take_while(|c| c.is_ascii_alphanumeric() || b":_-".contains(c)).count().min(i + 64 - ns);
        flush(&mut out, text_start, i);
        let name = src[ns..ns + name_len].to_ascii_lowercase();
        let mut j = ns + name_len;
        let mut attrs = BTreeMap::new();
        while j < n {
            j = skip_ws(src, j);
            if j >= n || b[j] == b'>' || src[j..].starts_with("/>") {
                break;
            }
            // [^\s=>/]+ within the next 256 units
            let mut e = j;
            let mut units = 0;
            while let Some(c) = ch(src, e) {
                if ws(c) || matches!(c, '=' | '>' | '/') || units + c.len_utf16() > 256 {
                    break;
                }
                units += c.len_utf16();
                e += c.len_utf8();
            }
            if e == j {
                j += 1;
                continue;
            }
            let an = src[j..e].to_lowercase();
            j = skip_ws(src, e);
            let mut val = "";
            if b.get(j) == Some(&b'=') {
                j = skip_ws(src, j + 1);
                match b.get(j) {
                    Some(&q @ (b'"' | b'\'')) => {
                        let e = src[j + 1..].find(q as char).map(|e| j + 1 + e);
                        val = &src[j + 1..e.unwrap_or(n)];
                        j = e.map_or(n, |e| e + 1);
                    }
                    _ => {
                        let mut e = j;
                        while let Some(c) = ch(src, e).filter(|&c| !ws(c) && c != '>') {
                            e += c.len_utf8();
                        }
                        val = &src[j..e];
                        j = e;
                    }
                }
            }
            attrs.entry(an).or_insert_with(|| decode_entities(val));
        }
        let self_close = src.get(j..).is_some_and(|r| r.starts_with("/>"));
        i = if self_close { j + 2 } else { n.min(j + 1) };
        text_start = i;
        if closing {
            out.push(MarkupToken::Close { name });
            continue;
        }
        let raw = RAW_TEXT.contains(&name.as_str()) && !self_close;
        out.push(MarkupToken::Open { name: name.clone(), attrs, self_close });
        if raw {
            let em = find_raw_close(src, i, &name);
            let end = em.map_or(n, |m| m.0);
            if end > i {
                let v = &src[i..end];
                out.push(MarkupToken::Text { v: if name == "textarea" { decode_entities(v) } else { v.to_owned() } });
            }
            out.push(MarkupToken::Close { name });
            i = em.map_or(n, |m| m.1);
            text_start = i;
        }
    }
    flush(&mut out, text_start, n);
    out
}

// ---- HTML → indented text ------------------------------------------------------------

const BLOCKS: &[&str] = &[
    "p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "li", "ul", "ol", "blockquote", "pre", "table", "thead", "tbody",
    "tfoot", "tr", "section", "article", "header", "footer", "nav", "aside", "dl", "dt", "dd", "hr", "figure",
    "figcaption", "address", "main", "form", "fieldset", "details", "summary", "center", "caption", "body", "html",
];
const SKIP: &[&str] = &[
    "script", "style", "head", "title", "template", "noscript", "xml", "iframe", "object", "svg", "math", "textarea",
    "select", "button",
];
const VOID: &[&str] = &["br", "img", "hr", "meta", "link", "input", "col", "area", "base", "wbr", "source", "embed", "param", "track"];

fn is_block(n: &str) -> bool {
    BLOCKS.contains(&n)
}

struct Marker {
    m: &'static str,
    /// byte position of the opening marker in the current line; `None` = not emitted on this line yet
    pos: Option<usize>,
}

struct Link {
    href: String,
    pos: Option<usize>,
    line: usize,
}

struct Frame {
    name: String,
    markers: Vec<Marker>,
    link: Option<Link>,
    pre: bool,
    list: bool,
}

/// The first match of `key:(alt1|alt2|…)` in `s` (alternatives tried in order at each place).
fn first_value<'a>(s: &str, key: &str, alts: &[&'a str]) -> Option<&'a str> {
    let mut from = 0;
    while let Some(at) = s[from..].find(key).map(|x| x + from) {
        let rest = &s[at + key.len()..];
        if let Some(a) = alts.iter().find(|a| rest.starts_with(**a)) {
            return Some(a);
        }
        from = at + 1;
    }
    None
}

const WEIGHTS: &[&str] = &[
    "bold", "bolder", "600", "700", "800", "900", "normal", "lighter", "100", "200", "300", "400", "500",
];

fn style_markers(name: &str, style: &str) -> Vec<&'static str> {
    let s: String = style.to_lowercase().chars().filter(|&c| !ws(c)).collect();
    let weight = first_value(&s, "font-weight:", WEIGHTS);
    let italic = first_value(&s, "font-style:", &["italic", "oblique", "normal"]);
    let bold_tag = name == "b" || name == "strong";
    let italic_tag = name == "i" || name == "em";
    let is_bold = match weight {
        Some(w) => !matches!(w, "normal" | "lighter" | "100" | "200" | "300" | "400" | "500"),
        None => bold_tag,
    };
    let is_italic = match italic {
        Some(v) => v != "normal",
        None => italic_tag,
    };
    let mut out = Vec::new();
    if is_bold {
        out.push("**");
    }
    if is_italic {
        out.push("__");
    }
    out
}

/// `mso-list:[^;"']*level(\d+)` (any case): the level number.
fn mso_level(style: &str) -> Option<i64> {
    let b = style.as_bytes();
    let mut from = 0;
    while let Some(at) = find_ci(style, from, "mso-list:") {
        let run_start = at + 9;
        let run_end = run_start + b[run_start..].iter().take_while(|c| !b";\"'".contains(c)).count();
        // greedy: the last `level<digits>` in the run
        for k in (run_start..run_end).rev() {
            if starts_with_ci(style, k, "level") && b.get(k + 5).is_some_and(u8::is_ascii_digit) {
                return parse_int(&style[k + 5..]);
            }
        }
        from = at + 1;
    }
    None
}

fn mso_ignore(style: &str) -> bool {
    let mut from = 0;
    while let Some(at) = find_ci(style, from, "mso-list:") {
        if starts_with_ci(style, skip_ws(style, at + 9), "ignore") {
            return true;
        }
        from = at + 1;
    }
    false
}

fn is_word_html(html: &str) -> bool {
    find_ci(html, 0, "urn:schemas-microsoft-com:office").is_some()
        || find_ci(html, 0, "mso-").is_some()
        || (0..html.len()).any(|k| {
            starts_with_ci(html, k, "class=") && {
                let q = k + 6 + usize::from(matches!(html.as_bytes().get(k + 6), Some(b'"' | b'\'')));
                html.get(q..q + 3) == Some("Mso") || starts_with_ci(html, q, "mso")
            }
        })
}

struct HtmlText {
    lines: Vec<String>,
    cur: String,
    cur_depth: Option<i64>,
    line_no: usize,
    list_depth: i64,
    pre_depth: i64,
    pending_depth: Option<i64>,
    stack: Vec<Frame>,
}

/// `s[..at]` and `s[at..]` with `at` moved back to a character boundary.
fn cut(s: &str, at: usize) -> (&str, &str) {
    let mut at = at.min(s.len());
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    s.split_at(at)
}

impl HtmlText {
    fn open_markers(&self) -> impl Iterator<Item = &Marker> {
        self.stack.iter().flat_map(|f| f.markers.iter())
    }

    fn close_marker(cur: &mut String, mk: &mut Marker) {
        let Some(pos) = mk.pos else { return };
        let (head, rest) = cut(cur, pos);
        let inner = cut(rest, mk.m.len()).1;
        *cur = if js_trim(inner).is_empty() {
            format!("{head}{inner}")
        } else {
            format!("{head}{}{}{}{}{}", lead_ws(inner), mk.m, js_trim(inner), mk.m, trail_ws(inner))
        };
        mk.pos = None;
    }

    fn flush(&mut self) {
        for f in self.stack.iter_mut().rev() {
            for mk in f.markers.iter_mut().rev() {
                Self::close_marker(&mut self.cur, mk);
            }
        }
        let text = if self.pre_depth != 0 { trim_end_ws(&self.cur) } else { js_trim(&self.cur) };
        if !text.is_empty() {
            self.lines.push("\t".repeat(self.cur_depth.unwrap_or(0).max(0) as usize) + text);
        }
        self.cur.clear();
        self.cur_depth = None;
        self.line_no += 1;
    }

    fn append(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        let mut s = s.to_owned();
        if self.pre_depth == 0 {
            // /[\s ]+/g → ' '
            let mut c = String::with_capacity(s.len());
            let mut in_ws = false;
            for ch in s.chars() {
                if ws(ch) {
                    if !in_ws {
                        c.push(' ');
                    }
                    in_ws = true;
                } else {
                    c.push(ch);
                    in_ws = false;
                }
            }
            s = c;
            if self.cur.is_empty() || self.cur.chars().next_back().is_some_and(ws) {
                if let Some(r) = s.strip_prefix(' ') {
                    s = r.to_owned();
                }
            }
            if s.is_empty() {
                return;
            }
        }
        if self.cur_depth.is_none() {
            if js_trim(&s).is_empty() && self.pre_depth == 0 {
                return;
            }
            self.cur_depth = Some(self.pending_depth.unwrap_or(self.list_depth));
        }
        if !js_trim(&s).is_empty() {
            // emit pending markers (outermost first) and pin link starts after outer markers
            for f in &mut self.stack {
                for mk in &mut f.markers {
                    if mk.pos.is_none() {
                        mk.pos = Some(self.cur.len());
                        self.cur += mk.m;
                    }
                }
                if let Some(l) = f.link.as_mut().filter(|l| l.pos.is_none()) {
                    l.pos = Some(self.cur.len());
                    l.line = self.line_no;
                }
            }
        }
        self.cur += &s;
    }
}

/// Convert clipboard HTML to indented markdown text: one tab per list nesting level, block elements and `<br>`
/// → new lines, b/strong → `**`, i/em → `__`, links → `[text](href)` (bare URL if text == href), images →
/// `![alt](src)`, scripts/styles dropped, whitespace collapsed except in `<pre>`. MS Word list paragraphs use
/// `mso-list … levelN` as depth.
pub fn html_to_text(html: &str) -> String {
    let tokens = tokenize_markup(html);
    let is_word = is_word_html(html);
    let mut h = HtmlText {
        lines: Vec::new(),
        cur: String::new(),
        cur_depth: None,
        line_no: 0,
        list_depth: 0,
        pre_depth: 0,
        pending_depth: None,
        stack: Vec::new(),
    };
    let mut skip: Option<(String, usize)> = None;
    for tok in &tokens {
        if let Some((sname, depth)) = &mut skip {
            match tok {
                MarkupToken::Open { name, self_close: false, .. } if name == sname => *depth += 1,
                MarkupToken::Close { name } if name == sname => {
                    *depth -= 1;
                    if *depth == 0 {
                        skip = None;
                    }
                }
                _ => {}
            }
            continue;
        }
        match tok {
            MarkupToken::Text { v } => {
                if h.pre_depth != 0 {
                    let v = normalize_newlines(v);
                    for (k, p) in v.split('\n').enumerate() {
                        if k > 0 {
                            h.flush();
                        }
                        h.append(&p.replace('\u{a0}', " "));
                    }
                } else {
                    h.append(v);
                }
            }
            MarkupToken::Open { name, attrs, self_close } => {
                let style = attrs.get("style").map_or("", String::as_str);
                if SKIP.contains(&name.as_str()) || (name == "span" && mso_ignore(style)) {
                    if !self_close && !VOID.contains(&name.as_str()) {
                        skip = Some((name.clone(), 1));
                    }
                    continue;
                }
                if name == "br" {
                    h.flush();
                    continue;
                }
                if name == "img" {
                    let src = attrs.get("src").map_or("", String::as_str);
                    if !src.is_empty() {
                        h.append(&format!("![{}]({src})", attrs.get("alt").map_or("", String::as_str)));
                    }
                    continue;
                }
                if (name == "td" || name == "th") && !js_trim(&h.cur).is_empty() {
                    h.append(" ");
                }
                if is_block(name) {
                    h.flush();
                    h.pending_depth = None;
                    if is_word {
                        if let Some(l) = mso_level(style) {
                            h.pending_depth = Some(l);
                        } else if let [b'h', d @ b'1'..=b'6'] = name.as_bytes() {
                            h.pending_depth = Some(i64::from(d - b'1'));
                        }
                    }
                }
                if VOID.contains(&name.as_str()) || *self_close {
                    continue;
                }
                let mut frame = Frame { name: name.clone(), markers: Vec::new(), link: None, pre: false, list: false };
                if name == "ul" || name == "ol" {
                    h.list_depth += 1;
                    frame.list = true;
                }
                if name == "pre" {
                    h.pre_depth += 1;
                    frame.pre = true;
                }
                if name == "a" {
                    let href = js_trim(attrs.get("href").map_or("", String::as_str));
                    let lower = href.to_ascii_lowercase();
                    let bad = ["javascript:", "vbscript:", "data:"].iter().any(|p| lower.starts_with(p));
                    frame.link = Some(Link { href: if bad { String::new() } else { href.to_owned() }, pos: None, line: h.line_no });
                }
                if ["b", "strong", "i", "em", "span", "font"].contains(&name.as_str()) {
                    for m in style_markers(name, style) {
                        if !h.open_markers().any(|x| x.m == m) {
                            frame.markers.push(Marker { m, pos: None });
                        }
                    }
                }
                h.stack.push(frame);
            }
            MarkupToken::Close { name } => {
                let Some(idx) = h.stack.iter().rposition(|f| &f.name == name) else {
                    if is_block(name) {
                        h.flush();
                    }
                    continue;
                };
                while h.stack.len() > idx {
                    let mut f = h.stack.pop().expect("frame");
                    for mk in f.markers.iter_mut().rev() {
                        HtmlText::close_marker(&mut h.cur, mk);
                    }
                    if let Some(Link { href, pos: Some(pos), line }) = &f.link {
                        if *line == h.line_no && !href.is_empty() {
                            let (head, text) = cut(&h.cur, *pos);
                            let t = js_trim(text);
                            if !t.is_empty() {
                                let md = if t == href { href.clone() } else { format!("[{t}]({href})") };
                                h.cur = format!("{head}{}{md}{}", lead_ws(text), trail_ws(text));
                            }
                        }
                    }
                    // the frame is off the stack while its block flushes (TS pops after): its markers are closed
                    // already, so the flush is the same
                    if is_block(&f.name) {
                        h.flush();
                        h.pending_depth = None;
                    }
                    if f.list {
                        h.list_depth -= 1;
                    }
                    if f.pre {
                        h.pre_depth -= 1;
                    }
                }
            }
        }
    }
    h.flush();
    h.lines.join("\n")
}

// ---- OPML ------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpmlDocument {
    pub title: String,
    /// `<flavor>` from the head, if any
    pub flavor: String,
    pub items: Vec<Portable>,
}

/// Flavors whose `text` attributes are raw inline markdown (no HTML conversion).
pub const NATIVE_FLAVORS: &[&str] = &["outliner", "dynalist"];

/// True if the string looks like an OPML document:
/// `^(<\?xml[^>]*\?>\s*)?(<!--[\s\S]*?-->\s*)*<opml[\s>]` (any case) on its first 512 units.
pub fn is_opml(text: &str) -> bool {
    let t = text.trim_start_matches(ws);
    let t = crate::text16::slice(t, 0, 512);
    // comments, then <opml, from `at`; each comment may end at any later `-->` (the lazy group backtracks)
    fn rest(t: &str, at: usize, memo: &mut Vec<Option<bool>>) -> bool {
        if let Some(v) = memo[at] {
            return v;
        }
        let v = if starts_with_ci(t, at, "<opml") {
            ch(t, at + 5).is_some_and(|c| ws(c) || c == '>')
        } else {
            false
        } || (t[at..].starts_with("<!--") && {
            let mut k = at + 4;
            let mut ok = false;
            while let Some(q) = t[k..].find("-->").map(|x| x + k) {
                if rest(t, skip_ws(t, q + 3), memo) {
                    ok = true;
                    break;
                }
                k = q + 1;
            }
            ok
        });
        memo[at] = Some(v);
        v
    }
    let mut memo = vec![None; t.len() + 1];
    if starts_with_ci(t, 0, "<?xml") {
        if let Some(gt) = t[5..].find('>').map(|x| x + 5) {
            if gt > 5 && t.as_bytes()[gt - 1] == b'?' && rest(t, skip_ws(t, gt + 1), &mut memo) {
                return true;
            }
        }
    }
    rest(t, 0, &mut memo)
}

/// Inline HTML (b, i, a…) from foreign OPML → markdown, newlines kept.
fn inline_html_to_markdown(s: &str) -> String {
    let b = s.as_bytes();
    let tagish = b.windows(2).any(|w| w[0] == b'<' && (w[1].is_ascii_alphabetic() || w[1] == b'/'));
    if !tagish {
        return s.to_owned();
    }
    html_to_text(&s.replace("\r\n", "<br>").replace('\n', "<br>"))
}

fn truthy_attr(v: Option<&String>) -> bool {
    v.is_some_and(|v| matches!(js_trim(v).to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
}

/// Parse OPML (any flavor) into portable items. `None` if it isn't OPML.
pub fn parse_opml(xml: &str) -> Option<OpmlDocument> {
    let tokens = tokenize_markup(xml);
    if !tokens.iter().any(|t| matches!(t, MarkupToken::Open { name, .. } if name == "opml")) {
        return None;
    }
    struct Raw<'a> {
        attrs: &'a BTreeMap<String, String>,
        children: Vec<Raw<'a>>,
    }
    let mut title = String::new();
    let mut flavor = String::new();
    let mut field: Option<&str> = None;
    let mut in_head = false;
    let mut roots: Vec<Raw<'_>> = Vec::new();
    // open outlines: a path of child indexes from the roots
    let mut stack: Vec<usize> = Vec::new();
    fn at<'r, 'a>(roots: &'r mut Vec<Raw<'a>>, path: &[usize]) -> &'r mut Vec<Raw<'a>> {
        let mut list = roots;
        for &k in path {
            list = &mut list[k].children;
        }
        list
    }
    for t in &tokens {
        match t {
            MarkupToken::Open { name, attrs, self_close } => {
                if name == "head" {
                    in_head = true;
                } else if in_head && (name == "title" || name == "flavor") {
                    field = if *self_close { None } else { Some(name.as_str()) };
                } else if name == "outline" {
                    let list = at(&mut roots, &stack);
                    list.push(Raw { attrs, children: Vec::new() });
                    if !self_close {
                        let k = list.len() - 1;
                        stack.push(k);
                    }
                }
            }
            MarkupToken::Close { name } => {
                if name == "head" {
                    in_head = false;
                } else if Some(name.as_str()) == field {
                    field = None;
                } else if name == "outline" {
                    stack.pop();
                }
            }
            MarkupToken::Text { v } => match field {
                Some("title") => title += v,
                Some("flavor") => flavor += v,
                _ => {}
            },
        }
    }
    let flavor = js_trim(&flavor).to_owned();
    let native = NATIVE_FLAVORS.contains(&flavor.to_lowercase().as_str());
    fn conv(raw: &Raw<'_>, native: bool) -> Portable {
        let a = raw.attrs;
        let get = |k: &str| a.get(k);
        let mut props = Props::new();
        let mut c = get("text").or(get("title")).cloned().unwrap_or_default();
        let mut n = get("_note").or(get("note")).cloned().unwrap_or_default();
        if !native {
            c = inline_html_to_markdown(&c).replace('\n', " ");
            n = inline_html_to_markdown(&n);
        }
        if !c.is_empty() {
            props.insert("c".into(), Value::String(c));
        }
        if !n.is_empty() {
            props.insert("n".into(), Value::String(n));
        }
        if truthy_attr(get("checkbox")) || truthy_attr(get("checklist")) {
            props.insert("cb".into(), Value::Bool(true));
        }
        if truthy_attr(get("complete")) || truthy_attr(get("_complete")) {
            props.insert("ck".into(), Value::Bool(true));
        }
        if let Some(h) = get("heading").and_then(|v| parse_int(v)).filter(|h| (1..=3).contains(h)) {
            props.insert("h".into(), Value::from(h));
        }
        if let Some(cl) = get("colorlabel").or(get("_colorlabel")).and_then(|v| parse_int(v)).filter(|c| (1..=6).contains(c)) {
            props.insert("cl".into(), Value::from(cl));
        }
        if get("liststyle").is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "arabic" | "numbered" | "decimal")) {
            props.insert("num".into(), Value::Bool(true));
        }
        Portable {
            props,
            collapsed: Some(truthy_attr(get("collapsed")) || truthy_attr(get("_collapsed"))),
            children: raw.children.iter().map(|r| conv(r, native)).collect(),
        }
    }
    Some(OpmlDocument { title: js_trim(&title).to_owned(), flavor, items: roots.iter().map(|r| conv(r, native)).collect() })
}

// ---- clipboard ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ClipboardData {
    pub plain: Option<String>,
    pub html: Option<String>,
    pub opml: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedPaste {
    pub items: Vec<Portable>,
    /// came from OPML: lossless, restore collapsed flags; else derived from text
    pub structured: bool,
}

/// Clipboard → items, by priority: OPML flavor or plain text that is OPML; then HTML (converted to indented
/// text); then plain text. `None` when there is no usable text. Literal-text contexts (notes, zoom title) should
/// use `normalize_newlines(plain)` instead.
pub fn parse_clipboard(data: &ClipboardData) -> Option<ParsedPaste> {
    let plain = data.plain.as_deref().filter(|p| !p.is_empty());
    for src in [data.opml.as_deref(), plain.filter(|p| is_opml(p))].into_iter().flatten() {
        if src.is_empty() {
            continue;
        }
        if let Some(doc) = parse_opml(src).filter(|d| !d.items.is_empty()) {
            return Some(ParsedPaste { items: doc.items, structured: true });
        }
    }
    if let Some(html) = data.html.as_deref().filter(|h| !js_trim(h).is_empty()) {
        let text = html_to_text(html);
        if !js_trim(&text).is_empty() {
            return Some(ParsedPaste { items: parse_plain_text(&text), structured: false });
        }
    }
    plain.map(|p| ParsedPaste { items: parse_plain_text(p), structured: false })
}
