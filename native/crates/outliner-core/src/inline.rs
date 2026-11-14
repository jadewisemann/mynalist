//! Inline markdown-ish syntax: a tokenizer with source offsets, and the helpers built on it. The Rust port of
//! `packages/core/src/inline.ts` (its HTML renderer stays in TS: the app draws tokens itself).
//!
//! Offsets are UTF-16 units, as in TS. Every token carries `start`/`end` (its full raw range) and
//! `inner_start`/`inner_end` (the range of its content, e.g. between delimiters), so a press on drawn text can be
//! mapped back to a caret offset. Tokens serialize to the TS token objects (`t`, `innerStart`, ...).

use serde::Serialize;

use crate::dates::{js_ws, parse_date_body, DateSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EmphasisKind {
    Bold,
    Italic,
    Strike,
    Highlight,
}

impl EmphasisKind {
    /// Delimiter string for an emphasis kind (`Bold` → `**`).
    pub fn delimiter(self) -> &'static str {
        match self {
            EmphasisKind::Bold => "**",
            EmphasisKind::Italic => "__",
            EmphasisKind::Strike => "~~",
            EmphasisKind::Highlight => "==",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    #[serde(flatten)]
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    pub inner_start: usize,
    pub inner_end: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "t", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum Kind {
    /// Literal text; maps 1:1 onto the source range (may contain `\n`).
    Text { text: String },
    /// `\x`: shows as `text` (the escaped char); the inner range is the char.
    Escape { text: String },
    Bold { children: Vec<Token> },
    Italic { children: Vec<Token> },
    Strike { children: Vec<Token> },
    Highlight { children: Vec<Token> },
    /// `` `code` ``: `text` has `` \` `` escapes resolved; `children` are text/escape parts.
    Code { text: String, children: Vec<Token> },
    /// ```` ```lang\n…``` ````: `text` excludes one trailing newline before the fence.
    Codeblock { lang: String, text: String },
    Latex { text: String },
    /// `[text](url)`: the inner range is the text; children = the text, unformatted.
    Link { text: String, url: String, url_start: usize, url_end: usize, children: Vec<Token> },
    /// `![alt](url)`: `text` is the alt text with escapes resolved.
    Image { text: String, url: String, url_start: usize, url_end: usize },
    Autolink { url: String },
    /// `#tag` / `@tag`: `tag` includes the prefix char.
    Tag { tag: String },
    /// `!(…)`: `text` is the raw body between the parens.
    Date { text: String, date: DateSpec },
}

impl Kind {
    pub fn emphasis(&self) -> Option<EmphasisKind> {
        Some(match self {
            Kind::Bold { .. } => EmphasisKind::Bold,
            Kind::Italic { .. } => EmphasisKind::Italic,
            Kind::Strike { .. } => EmphasisKind::Strike,
            Kind::Highlight { .. } => EmphasisKind::Highlight,
            _ => return None,
        })
    }

    pub fn children(&self) -> &[Token] {
        match self {
            Kind::Bold { children }
            | Kind::Italic { children }
            | Kind::Strike { children }
            | Kind::Highlight { children }
            | Kind::Code { children, .. }
            | Kind::Link { children, .. } => children,
            _ => &[],
        }
    }
}

pub const ESCAPABLE: &str = "\\~$=`*_{}[]()#+-.!";
const TAG_STOP: &str = "`',?!;/";

fn is_escapable(c: Option<u16>) -> bool {
    c.is_some_and(|c| c < 128 && ESCAPABLE.as_bytes().contains(&(c as u8)))
}

fn is_ws(c: Option<u16>) -> bool {
    c.is_some_and(|c| js_ws(u32::from(c)))
}

fn is_alpha(c: u16) -> bool {
    c < 128 && (c as u8).is_ascii_alphabetic()
}

fn is_scheme_char(c: u16) -> bool {
    c < 128 && ((c as u8).is_ascii_alphanumeric() || b"+.-".contains(&(c as u8)))
}

const fn pair(s: &[u8; 2]) -> [u16; 2] {
    [s[0] as u16, s[1] as u16]
}

fn emphasis_of(two: [u16; 2]) -> Option<EmphasisKind> {
    Some(match two {
        x if x == pair(b"**") => EmphasisKind::Bold,
        x if x == pair(b"__") => EmphasisKind::Italic,
        x if x == pair(b"~~") => EmphasisKind::Strike,
        x if x == pair(b"==") => EmphasisKind::Highlight,
        _ => return None,
    })
}

struct Ctx<'a> {
    src: &'a [u16],
    /// links, images, autolinks, tags and dates allowed
    rich: bool,
    /// `[` → matching `]`, computed on first use
    brackets: Option<Vec<i64>>,
}

impl Ctx<'_> {
    fn at(&self, i: usize) -> Option<u16> {
        self.src.get(i).copied()
    }

    fn is(&self, i: usize, c: u8) -> bool {
        self.at(i) == Some(u16::from(c))
    }

    /// `src.slice(i, i + 2)` as a pair (0 past the end: no delimiter has a NUL).
    fn two(&self, i: usize) -> [u16; 2] {
        [self.at(i).unwrap_or(0), self.at(i + 1).unwrap_or(0)]
    }

    fn starts_with(&self, i: usize, s: &str) -> bool {
        let p: Vec<u16> = s.encode_utf16().collect();
        self.src.get(i..i + p.len()) == Some(&p[..])
    }

    fn index_of(&self, s: &str, from: usize) -> Option<usize> {
        let p: Vec<u16> = s.encode_utf16().collect();
        (from..=self.src.len().saturating_sub(p.len())).find(|&k| self.src[k..].starts_with(&p))
    }

    fn slice(&self, a: usize, b: usize) -> String {
        String::from_utf16_lossy(&self.src[a..b])
    }

    fn text_tok(&self, s: usize, e: usize) -> Token {
        Token { kind: Kind::Text { text: self.slice(s, e) }, start: s, end: e, inner_start: s, inner_end: e }
    }
}

struct Seq {
    tokens: Vec<Token>,
    pos: usize,
    closed_by: Option<[u16; 2]>,
}

/// Tokenize inline source. Never fails; unclosed delimiters become text.
pub fn parse_inline(src: &str) -> Vec<Token> {
    let u: Vec<u16> = src.encode_utf16().collect();
    parse_inline16(&u)
}

/// [`parse_inline`] on UTF-16 text.
pub fn parse_inline16(src: &[u16]) -> Vec<Token> {
    let mut ctx = Ctx { src, rich: true, brackets: None };
    parse_seq(&mut ctx, 0, src.len(), &[]).tokens
}

fn parse_seq(ctx: &mut Ctx<'_>, from: usize, end: usize, closers: &[[u16; 2]]) -> Seq {
    let mut tokens = Vec::new();
    let mut text_start = from;
    let mut i = from;
    let flush = |ctx: &Ctx<'_>, tokens: &mut Vec<Token>, text_start: usize, up_to: usize| {
        if up_to > text_start {
            tokens.push(ctx.text_tok(text_start, up_to));
        }
    };

    while i < end {
        let c = ctx.src[i];
        let two = ctx.two(i);
        let mut tok: Option<Token> = None;
        if c == u16::from(b'\\') && i + 1 < end && is_escapable(ctx.at(i + 1)) {
            tok = Some(Token {
                kind: Kind::Escape { text: ctx.slice(i + 1, i + 2) },
                start: i,
                end: i + 2,
                inner_start: i + 1,
                inner_end: i + 2,
            });
        } else if closers.contains(&two) {
            // closer of an enclosing span
            flush(ctx, &mut tokens, text_start, i);
            return Seq { tokens, pos: i, closed_by: Some(two) };
        } else if c == u16::from(b'`') {
            tok = if ctx.starts_with(i, "```") { code_block(ctx, i, end) } else { inline_code(ctx, i, end) };
        } else if two == pair(b"$$") {
            tok = latex(ctx, i, end);
        } else if ctx.rich && two == pair(b"![") {
            tok = image(ctx, i, end);
        } else if ctx.rich && c == u16::from(b'[') {
            tok = link(ctx, i, end);
        } else if ctx.rich && two == pair(b"!(") {
            tok = date(ctx, i, end);
        } else if ctx.rich && (c == u16::from(b'#') || c == u16::from(b'@')) && (i == 0 || is_ws(ctx.at(i - 1))) {
            tok = tag(ctx, i, end, closers);
        } else if ctx.rich && is_alpha(c) && (i == 0 || !is_scheme_char(ctx.src[i - 1])) {
            tok = autolink(ctx, i, end, closers);
        }

        if let Some(t) = tok {
            flush(ctx, &mut tokens, text_start, i);
            i = t.end;
            text_start = i;
            tokens.push(t);
            continue;
        }
        if c == u16::from(b'`') || two == pair(b"$$") || (two == pair(b"![") && ctx.rich) {
            // unclosed opener: literal; skip the whole delimiter so it can't re-open
            i += if c == u16::from(b'`') {
                if ctx.starts_with(i, "```") {
                    3
                } else if ctx.is(i + 1, b'`') {
                    2
                } else {
                    1
                }
            } else {
                2
            };
            continue;
        }
        if let Some(kind) = emphasis_of(two) {
            // cheap pre-check: no later delimiter → can't close
            let delim = kind.delimiter();
            let again = ctx.index_of(delim, i + 2);
            let r = match again {
                Some(a) if a + 2 <= end => {
                    let mut inner: Vec<[u16; 2]> = closers.to_vec();
                    inner.push(two);
                    Some(parse_seq(ctx, i + 2, end, &inner))
                }
                _ => None,
            };
            match r {
                Some(r) if r.closed_by == Some(two) => {
                    // an empty pair ("****") is still a span: it shows as nothing, like Dynalist
                    let children = r.tokens;
                    let kind = match kind {
                        EmphasisKind::Bold => Kind::Bold { children },
                        EmphasisKind::Italic => Kind::Italic { children },
                        EmphasisKind::Strike => Kind::Strike { children },
                        EmphasisKind::Highlight => Kind::Highlight { children },
                    };
                    flush(ctx, &mut tokens, text_start, i);
                    tokens.push(Token { kind, start: i, end: r.pos + 2, inner_start: i + 2, inner_end: r.pos });
                    i = r.pos + 2;
                    text_start = i;
                }
                // unclosed: literal delimiter, resume after it
                _ => i += 2,
            }
            continue;
        }
        i += 1;
    }
    flush(ctx, &mut tokens, text_start, i);
    Seq { tokens, pos: i, closed_by: None }
}

fn inline_code(ctx: &Ctx<'_>, i: usize, end: usize) -> Option<Token> {
    let mut children = Vec::new();
    let mut text = String::new();
    let mut seg = i + 1;
    let mut j = i + 1;
    while j < end {
        if ctx.is(j, b'\\') && ctx.is(j + 1, b'`') && j + 1 < end {
            if j > seg {
                children.push(ctx.text_tok(seg, j));
            }
            children.push(Token {
                kind: Kind::Escape { text: "`".into() },
                start: j,
                end: j + 2,
                inner_start: j + 1,
                inner_end: j + 2,
            });
            text += &ctx.slice(seg, j);
            text.push('`');
            j += 2;
            seg = j;
            continue;
        }
        if ctx.is(j, b'`') {
            if j > seg {
                children.push(ctx.text_tok(seg, j));
            }
            text += &ctx.slice(seg, j);
            return Some(Token { kind: Kind::Code { text, children }, start: i, end: j + 1, inner_start: i + 1, inner_end: j });
        }
        j += 1;
    }
    None
}

fn code_block(ctx: &Ctx<'_>, i: usize, end: usize) -> Option<Token> {
    let first_close = ctx.index_of("```", i + 3)?;
    if first_close + 3 > end {
        return None;
    }
    let mut lang = String::new();
    let mut cs = i + 3;
    if let Some(nl) = ctx.index_of("\n", i + 3) {
        if nl < first_close && ctx.src[i + 3..nl].iter().all(|&c| c < 128 && (c as u8).is_ascii_lowercase()) {
            lang = ctx.slice(i + 3, nl);
            cs = nl + 1;
        }
    }
    let close = ctx.index_of("```", cs)?;
    if close + 3 > end {
        return None;
    }
    let mut body = &ctx.src[cs..close];
    if body.last() == Some(&u16::from(b'\n')) {
        body = &body[..body.len() - 1];
    }
    let mut tok_end = close + 3;
    if ctx.is(tok_end, b'\n') && tok_end < end {
        tok_end += 1;
    }
    Some(Token {
        kind: Kind::Codeblock { lang, text: String::from_utf16_lossy(body) },
        start: i,
        end: tok_end,
        inner_start: cs,
        inner_end: cs + body.len(),
    })
}

fn latex(ctx: &Ctx<'_>, i: usize, end: usize) -> Option<Token> {
    let close = ctx.index_of("$$", i + 2)?;
    if close + 2 > end || close == i + 2 {
        return None;
    }
    Some(Token { kind: Kind::Latex { text: ctx.slice(i + 2, close) }, start: i, end: close + 2, inner_start: i + 2, inner_end: close })
}

/// The `]` matching the `[` at `open`, honoring escapes and nesting.
fn match_bracket(ctx: &mut Ctx<'_>, open: usize, end: usize) -> Option<usize> {
    let map = ctx.brackets.get_or_insert_with(|| {
        // one stack pass: equivalent to depth-counting forward from each `[`
        let src = ctx.src;
        let mut map = vec![-1i64; src.len()];
        let mut stack = Vec::new();
        let mut j = 0;
        while j < src.len() {
            let c = src[j];
            if c == u16::from(b'\\') && is_escapable(src.get(j + 1).copied()) {
                j += 2;
                continue;
            }
            if c == u16::from(b'[') {
                stack.push(j);
            } else if c == u16::from(b']') {
                if let Some(o) = stack.pop() {
                    map[o] = j as i64;
                }
            }
            j += 1;
        }
        map
    });
    let m = map[open];
    (m >= 0 && (m as usize) < end).then_some(m as usize)
}

/// Given `(` at `open`, the balanced `)`; no newlines allowed.
fn match_paren(ctx: &Ctx<'_>, open: usize, end: usize) -> Option<usize> {
    let mut depth = 0;
    for (j, &c) in ctx.src.iter().enumerate().take(end).skip(open) {
        match c {
            0x0a => return None,
            0x28 => depth += 1,
            0x29 => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

fn link_parts(ctx: &mut Ctx<'_>, lb: usize, end: usize) -> Option<(usize, usize)> {
    let rb = match_bracket(ctx, lb, end)?;
    if !ctx.is(rb + 1, b'(') {
        return None;
    }
    let rp = match_paren(ctx, rb + 1, end)?;
    Some((rb, rp))
}

fn link(ctx: &mut Ctx<'_>, i: usize, end: usize) -> Option<Token> {
    let (rb, rp) = link_parts(ctx, i, end)?;
    // link text is shown as written: Dynalist doesn't format inside it
    let children = if rb > i + 1 { vec![ctx.text_tok(i + 1, rb)] } else { vec![] };
    Some(Token {
        kind: Kind::Link {
            text: ctx.slice(i + 1, rb),
            url: js_trim16(&ctx.src[rb + 2..rp]),
            url_start: rb + 2,
            url_end: rp,
            children,
        },
        start: i,
        end: rp + 1,
        inner_start: i + 1,
        inner_end: rb,
    })
}

fn image(ctx: &mut Ctx<'_>, i: usize, end: usize) -> Option<Token> {
    let (rb, rp) = link_parts(ctx, i + 1, end)?;
    Some(Token {
        kind: Kind::Image {
            text: unescape(&ctx.slice(i + 2, rb)),
            url: js_trim16(&ctx.src[rb + 2..rp]),
            url_start: rb + 2,
            url_end: rp,
        },
        start: i,
        end: rp + 1,
        inner_start: i + 2,
        inner_end: rb,
    })
}

fn js_trim16(s: &[u16]) -> String {
    crate::dates::js_trim(&String::from_utf16_lossy(s)).to_owned()
}

fn date(ctx: &Ctx<'_>, i: usize, end: usize) -> Option<Token> {
    let mut j = i + 2;
    while j < end && !ctx.is(j, b')') && !ctx.is(j, b'(') && !ctx.is(j, b'\n') {
        j += 1;
    }
    if j >= end || !ctx.is(j, b')') {
        return None;
    }
    let body = ctx.slice(i + 2, j);
    let spec = parse_date_body(&body)?;
    Some(Token { kind: Kind::Date { text: body, date: spec }, start: i, end: j + 1, inner_start: i + 2, inner_end: j })
}

/// Length of a tag body starting at `from` (after `#`/`@`).
fn scan_tag(ctx: &Ctx<'_>, from: usize, end: usize, closers: &[[u16; 2]]) -> usize {
    let mut depth = 0;
    let mut j = from;
    while j < end {
        let c = ctx.src[j];
        if is_ws(Some(c)) || (c < 128 && TAG_STOP.as_bytes().contains(&(c as u8))) {
            break;
        }
        let two = ctx.two(j);
        if two == pair(b"**") || two == pair(b"__") || closers.contains(&two) {
            break;
        }
        if c == u16::from(b'(') {
            depth += 1;
        } else if c == u16::from(b')') {
            if depth == 0 {
                break;
            }
            depth -= 1;
        } else if (c == u16::from(b':') || c == u16::from(b'.')) && (j + 1 >= end || is_ws(ctx.at(j + 1))) {
            break;
        }
        j += 1;
    }
    j - from
}

fn tag(ctx: &Ctx<'_>, i: usize, end: usize, closers: &[[u16; 2]]) -> Option<Token> {
    let len = scan_tag(ctx, i + 1, end, closers);
    if len == 0 {
        return None;
    }
    let body = &ctx.src[i + 1..i + 1 + len];
    if body.iter().all(|&c| c < 128 && (c as u8).is_ascii_digit()) {
        return None;
    }
    Some(Token {
        kind: Kind::Tag { tag: ctx.slice(i, i + 1 + len) },
        start: i,
        end: i + 1 + len,
        inner_start: i,
        inner_end: i + 1 + len,
    })
}

fn autolink(ctx: &Ctx<'_>, i: usize, end: usize, closers: &[[u16; 2]]) -> Option<Token> {
    // /[a-zA-Z][a-zA-Z0-9+.-]*:\/\//y
    let mut j = i + 1;
    while j < ctx.src.len() && is_scheme_char(ctx.src[j]) {
        j += 1;
    }
    if !ctx.starts_with(j, "://") {
        return None;
    }
    j += 3;
    let body_start = j;
    let mut depth = 0;
    while j < end {
        let c = ctx.src[j];
        if is_ws(Some(c)) || closers.contains(&ctx.two(j)) {
            break;
        }
        if c == u16::from(b'(') {
            depth += 1;
        } else if c == u16::from(b')') {
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        j += 1;
    }
    if j == body_start {
        return None;
    }
    Some(Token { kind: Kind::Autolink { url: ctx.slice(i, j) }, start: i, end: j, inner_start: i, inner_end: j })
}

/// Resolve `\x` escapes (escapable chars only).
pub fn unescape(s: &str) -> String {
    // s.replace(/\\(.)/g, …): `.` is one UTF-16 unit other than a line break
    let u: Vec<u16> = s.encode_utf16().collect();
    let mut out = Vec::with_capacity(u.len());
    let mut i = 0;
    while i < u.len() {
        if u[i] == u16::from(b'\\') && i + 1 < u.len() && !matches!(u[i + 1], 0x0a | 0x0d | 0x2028 | 0x2029) {
            if !is_escapable(Some(u[i + 1])) {
                out.push(u[i]);
            }
            out.push(u[i + 1]);
            i += 2;
            continue;
        }
        out.push(u[i]);
        i += 1;
    }
    String::from_utf16_lossy(&out)
}

/// Backslash-escape inline syntax characters so `s` shows literally.
pub fn escape_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\`*_~=$[]()#!".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// ---- traversal helpers --------------------------------------------------------------

/// Depth-first walk over tokens (including children of spans, code and links).
pub fn walk_tokens<'a>(tokens: &'a [Token], f: &mut dyn FnMut(&'a Token)) {
    for t in tokens {
        f(t);
        walk_tokens(t.kind.children(), f);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundTag {
    pub tag: String,
    pub start: usize,
    pub end: usize,
}

pub fn find_tags(src: &str) -> Vec<FoundTag> {
    let tokens = parse_inline(src);
    let mut out = Vec::new();
    walk_tokens(&tokens, &mut |t| {
        if let Kind::Tag { tag } = &t.kind {
            out.push(FoundTag { tag: tag.clone(), start: t.start, end: t.end });
        }
    });
    out
}

/// All tags (with `#`/`@` prefix, as written) in source order, duplicates kept.
pub fn extract_tags(src: &str) -> Vec<String> {
    find_tags(src).into_iter().map(|t| t.tag).collect()
}

/// Rename (or delete, with `to = None`) every occurrence of a tag, case-insensitively. Deleting also removes one
/// adjacent space.
pub fn replace_tag(src: &str, from: &str, to: Option<&str>) -> String {
    let want = from.to_lowercase();
    let mut out: Vec<u16> = src.encode_utf16().collect();
    let to16: Vec<u16> = to.unwrap_or("").encode_utf16().collect();
    let space = u16::from(b' ');
    for f in find_tags(src).into_iter().rev() {
        if f.tag.to_lowercase() != want {
            continue;
        }
        let (mut s, mut e) = (f.start, f.end);
        if to.is_none() {
            if out.get(e) == Some(&space) {
                e += 1;
            } else if s > 0 && out[s - 1] == space {
                s -= 1;
            }
        }
        out.splice(s..e, to16.iter().copied());
    }
    String::from_utf16_lossy(&out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Link,
    Image,
    Autolink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundLink {
    pub kind: LinkKind,
    /// link text / image alt (raw source for links); the URL itself for autolinks
    pub text: String,
    pub url: String,
    pub start: usize,
    pub end: usize,
}

pub fn extract_links(src: &str) -> Vec<FoundLink> {
    let tokens = parse_inline(src);
    let mut out = Vec::new();
    walk_tokens(&tokens, &mut |t| {
        let (kind, text, url) = match &t.kind {
            Kind::Link { text, url, .. } => (LinkKind::Link, text, url),
            Kind::Image { text, url, .. } => (LinkKind::Image, text, url),
            Kind::Autolink { url } => (LinkKind::Autolink, url, url),
            _ => return,
        };
        out.push(FoundLink { kind, text: text.clone(), url: url.clone(), start: t.start, end: t.end });
    });
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundInlineDate {
    pub spec: DateSpec,
    pub body: String,
    pub start: usize,
    pub end: usize,
}

/// Dates in source order (syntax-aware: dates inside code are ignored).
pub fn extract_dates(src: &str) -> Vec<FoundInlineDate> {
    let tokens = parse_inline(src);
    let mut out = Vec::new();
    walk_tokens(&tokens, &mut |t| {
        if let Kind::Date { text, date } = &t.kind {
            out.push(FoundInlineDate { spec: date.clone(), body: text.clone(), start: t.start, end: t.end });
        }
    });
    out
}

/// A live title for a link's URL (a document or item it points at), when known.
pub type LinkTitle<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Text as displayed, without formatting (breadcrumbs, window titles, finders).
pub fn inline_to_plain(src: &str, link_title: Option<LinkTitle<'_>>) -> String {
    plain_of(&parse_inline(src), link_title)
}

/// What a link shows: a live title when its text is empty or is the URL (and a title is known), else its text,
/// else "Link".
pub fn link_label(text: &str, url: &str, children: &[Token], link_title: Option<LinkTitle<'_>>) -> String {
    let title = if text.is_empty() || text == url { link_title.and_then(|f| f(url)) } else { None };
    title.unwrap_or_else(|| {
        let own = plain_of(children, None);
        if own.is_empty() { "Link".to_owned() } else { own }
    })
}

fn plain_of(tokens: &[Token], link_title: Option<LinkTitle<'_>>) -> String {
    let mut s = String::new();
    for t in tokens {
        match &t.kind {
            Kind::Text { text }
            | Kind::Escape { text }
            | Kind::Code { text, .. }
            | Kind::Codeblock { text, .. }
            | Kind::Latex { text } => s += text,
            Kind::Bold { children } | Kind::Italic { children } | Kind::Strike { children } | Kind::Highlight { children } => {
                s += &plain_of(children, link_title)
            }
            Kind::Link { text, url, children, .. } => s += &link_label(text, url, children, link_title),
            Kind::Image { text, .. } => s += if text.is_empty() { "Image" } else { text },
            Kind::Autolink { url } => s += url,
            Kind::Tag { tag } => s += tag,
            Kind::Date { text, .. } => s += &format!("!({text})"),
        }
    }
    s
}

// ---- URLs ---------------------------------------------------------------------------

fn has_scheme_prefix(url: &str) -> bool {
    // /^[a-zA-Z][a-zA-Z0-9+.-]*:/
    let b = url.as_bytes();
    if !b.first().is_some_and(u8::is_ascii_alphabetic) {
        return false;
    }
    let n = b[1..].iter().take_while(|c| c.is_ascii_alphanumeric() || b"+.-".contains(c)).count();
    b.get(1 + n) == Some(&b':')
}

/// Spec normalization: keep anything with a `scheme:`; `C:\…` or `/abs/path` → `file://…`; otherwise prefix
/// `http://`.
pub fn normalize_url(raw: &str) -> String {
    let url = crate::dates::js_trim(raw);
    if url.is_empty() {
        return String::new();
    }
    let b = url.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/') {
        return format!("file:///{}", url.replace('\\', "/"));
    }
    if url.starts_with('/') {
        return format!("file://{url}");
    }
    if has_scheme_prefix(url) || url.starts_with('#') {
        return url.to_owned();
    }
    format!("http://{url}")
}

const BAD_SCHEMES: [&str; 5] = ["javascript", "vbscript", "data", "livescript", "mocha"];

/// Normalized URL, or `None` if it uses a dangerous scheme (javascript:, data:, …).
pub fn safe_url(raw: &str, allow_data_image: bool) -> Option<String> {
    let url = normalize_url(&raw.replace(['\t', '\n', '\r'], ""));
    if url.is_empty() {
        return None;
    }
    // browsers ignore control chars/whitespace inside the scheme
    let probe: String = url
        .chars()
        .filter(|&c| {
            let c = c as u32;
            !(c <= 0x20
                || (0x7f..=0xa0).contains(&c)
                || c == 0x1680
                || c == 0x180e
                || (0x2000..=0x2029).contains(&c)
                || c == 0x205f
                || c == 0x3000
                || c == 0xfeff)
        })
        .collect::<String>()
        .to_lowercase();
    if has_scheme_prefix(&probe) {
        let scheme = &probe[..probe.find(':').expect("scheme")];
        if BAD_SCHEMES.contains(&scheme) {
            let data_image = ["png", "gif", "jpg", "jpeg", "webp", "bmp"]
                .iter()
                .any(|k| probe.starts_with(&format!("data:image/{k};")));
            return (allow_data_image && data_image).then_some(url);
        }
    }
    Some(url)
}

/// Map a character offset inside a drawn element back to a source offset. `s`/`e` are the element's source
/// range, `display_len` the length of what it shows.
pub fn map_display_offset(s: usize, e: usize, display_len: usize, offset: usize) -> usize {
    if e - s == display_len {
        return s + offset.min(display_len);
    }
    if offset == 0 { s } else { e }
}

/// Innermost token containing source `offset` (start <= offset < end), with its ancestors.
pub fn token_path_at(tokens: &[Token], offset: usize) -> Vec<&Token> {
    let mut path = Vec::new();
    let mut list = tokens;
    while let Some(hit) = list.iter().find(|t| t.start <= offset && offset < t.end) {
        path.push(hit);
        list = hit.kind.children();
    }
    path
}

/// JS's case-insensitive comparison key for one UTF-16 unit (`Canonicalize` without the `u` flag).
fn canonicalize(u: u16) -> u16 {
    let Some(c) = char::from_u32(u32::from(u)) else { return u };
    let mut up = c.to_uppercase();
    match (up.next(), up.next()) {
        (Some(x), None) if (x as u32) < 0x10000 && !(u >= 128 && (x as u32) < 128) => x as u16,
        _ => u,
    }
}

/// Where search terms occur in `s` (case-insensitive), as UTF-16 ranges: every occurrence of every term, with
/// overlapping and touching ranges merged into one, sorted.
pub fn match_ranges(s: &str, terms: &[&str]) -> Vec<(usize, usize)> {
    let hay: Vec<u16> = s.encode_utf16().map(canonicalize).collect();
    let mut found = Vec::new();
    for t in terms {
        let needle: Vec<u16> = t.encode_utf16().map(canonicalize).collect();
        if needle.is_empty() || needle.len() > hay.len() {
            continue;
        }
        for k in 0..=hay.len() - needle.len() {
            if hay[k..k + needle.len()] == needle[..] {
                found.push((k, k + needle.len()));
            }
        }
    }
    found.sort_unstable();
    let mut out: Vec<(usize, usize)> = Vec::new();
    for r in found {
        match out.last_mut() {
            Some(last) if r.0 <= last.1 => last.1 = last.1.max(r.1),
            _ => out.push(r),
        }
    }
    out
}
