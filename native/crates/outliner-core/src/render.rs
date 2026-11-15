//! Inline source → HTML: the renderer of `packages/core/src/inline.ts` (`renderInline`, `escapeHtml`). The web
//! app draws rows with it; the native app draws tokens itself and uses it for HTML export and the clipboard
//! (`clean`).
//!
//! All user text is escaped; URLs go through `safe_url`. Offsets in `data-s`/`data-e` are UTF-16 units.

use crate::dates::{describe_recurrence, format_date_label, is_overdue, DateFormatOptions};
use crate::inline::{match_ranges, LinkTitle, parse_inline, safe_url, Kind, Token};
use crate::text16;

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out += "&amp;",
            '<' => out += "&lt;",
            '>' => out += "&gt;",
            '"' => out += "&quot;",
            '\'' => out += "&#39;",
            _ => out.push(c),
        }
    }
    out
}

#[derive(Default)]
pub struct RenderOptions<'a> {
    /// current time for relative date labels / overdue (default now)
    pub now: Option<i64>,
    /// 12h clock (default true)
    pub hour12: Option<bool>,
    /// custom date formats (see `dates::format_date_label`)
    pub date_format: Option<String>,
    pub date_format_no_time: Option<String>,
    /// add `o-overdue` to past dates
    pub mark_overdue: bool,
    /// render `![]()` as `<img>` instead of a chip
    pub inline_images: bool,
    /// case-insensitive substrings to wrap in `<mark class="o-match">` (search; see `match_ranges`)
    pub highlight: Vec<String>,
    /// true for app-internal URLs (rendered with `o-link-internal`, no target)
    pub is_internal: Option<&'a dyn Fn(&str) -> bool>,
    /// live title for links whose text is empty or equals the URL
    pub link_title: Option<LinkTitle<'a>>,
    /// clean HTML for export/clipboard: plain tags, no classes or data attributes; tags and dates as literal text
    pub clean: bool,
}

/// Render inline source to an HTML string.
pub fn render_inline(src: &str, opts: &RenderOptions<'_>) -> String {
    render_tokens(&parse_inline(src), opts, Some(src))
}

/// `src` (what `tokens` were parsed from) is needed only to reveal search matches hidden in link URLs.
pub fn render_tokens(tokens: &[Token], opts: &RenderOptions<'_>, src: Option<&str>) -> String {
    let terms: Vec<&str> = opts.highlight.iter().map(String::as_str).filter(|t| !t.is_empty()).collect();
    Renderer { o: opts, terms, src }.tokens(tokens)
}

struct Renderer<'a, 'o> {
    o: &'a RenderOptions<'o>,
    terms: Vec<&'a str>,
    src: Option<&'a str>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl Renderer<'_, '_> {
    /// Source of a link/image when a search match in it falls outside its visible text (in the URL or the
    /// brackets): the link then shows its markup so the match can be seen.
    fn revealed(&self, t: &Token) -> Option<String> {
        let src = self.src?;
        if self.terms.is_empty() || self.o.clean {
            return None;
        }
        let raw = text16::slice(src, t.start, t.end);
        let a = t.inner_start - t.start;
        let b = t.inner_end - t.start;
        match_ranges(raw, &self.terms).iter().any(|&(s, e)| s < a || e > b).then(|| raw.to_owned())
    }

    fn attrs(&self, t: &Token, extra: &str) -> String {
        if self.o.clean { String::new() } else { format!(" data-s=\"{}\" data-e=\"{}\"{extra}", t.start, t.end) }
    }

    fn cls(&self, c: &str) -> String {
        if self.o.clean { String::new() } else { format!(" class=\"{c}\"") }
    }

    fn tokens(&self, list: &[Token]) -> String {
        list.iter().map(|t| self.token(t)).collect()
    }

    /// Plain text run starting at source offset `s` (1:1 mapping), split at newlines.
    fn text(&self, text: &str, s: usize) -> String {
        let mut out = String::new();
        let mut off = 0;
        for line in text.split('\n') {
            if off > 0 {
                out += "<br>";
            }
            let len = text16::len(line);
            if !line.is_empty() {
                let body = self.mark(line);
                if self.o.clean {
                    out += &body;
                } else {
                    out += &format!("<span class=\"o-t\" data-s=\"{}\" data-e=\"{}\">{body}</span>", s + off, s + off + len);
                }
            }
            off += len + 1;
        }
        out
    }

    fn mark(&self, s: &str) -> String {
        if self.terms.is_empty() {
            return escape_html(s);
        }
        let mut out = String::new();
        let mut last = 0;
        for (a, b) in match_ranges(s, &self.terms) {
            out += &escape_html(text16::slice(s, last, a));
            out += &format!("<mark class=\"o-match\">{}</mark>", escape_html(text16::slice(s, a, b)));
            last = b;
        }
        out + &escape_html(text16::slice_from(s, last))
    }

    fn token(&self, t: &Token) -> String {
        let o = self.o;
        match &t.kind {
            Kind::Text { text } => self.text(text, t.start),
            Kind::Escape { text } => {
                if o.clean { escape_html(text) } else { format!("<span class=\"o-esc\"{}>{}</span>", self.attrs(t, ""), escape_html(text)) }
            }
            Kind::Bold { children } | Kind::Italic { children } | Kind::Strike { children } | Kind::Highlight { children } => {
                let (tag, c) = match &t.kind {
                    Kind::Bold { .. } => ("strong", "o-bold"),
                    Kind::Italic { .. } => ("em", "o-italic"),
                    Kind::Strike { .. } => ("del", "o-strike"),
                    _ => ("mark", "o-highlight"),
                };
                format!("<{tag}{}{}>{}</{tag}>", self.cls(c), self.attrs(t, ""), self.tokens(children))
            }
            Kind::Code { children, .. } => {
                let mut inner = String::new();
                for c in children {
                    match &c.kind {
                        Kind::Escape { .. } => {
                            inner += &if o.clean { "`".to_owned() } else { format!("<span class=\"o-esc\"{}>`</span>", self.attrs(c, "")) }
                        }
                        Kind::Text { text } => inner += &self.text(text, c.start),
                        _ => {}
                    }
                }
                format!("<code{}{}>{inner}</code>", self.cls("o-code"), self.attrs(t, ""))
            }
            Kind::Codeblock { lang, text } => {
                let lang_attr = if lang.is_empty() { String::new() } else { format!(" data-lang=\"{}\"", escape_html(lang)) };
                let body = if o.clean {
                    escape_html(text)
                } else if text.is_empty() {
                    String::new()
                } else {
                    format!("<span class=\"o-t\" data-s=\"{}\" data-e=\"{}\">{}</span>", t.inner_start, t.inner_end, self.mark(text))
                };
                let lang_cls = if o.clean && !lang.is_empty() { format!(" class=\"language-{}\"", escape_html(lang)) } else { String::new() };
                format!("<pre{}{lang_attr}{}><code{lang_cls}>{body}</code></pre>", self.cls("o-codeblock"), self.attrs(t, ""))
            }
            Kind::Latex { text } => {
                if o.clean {
                    return escape_html(&format!("$${text}$$"));
                }
                format!(
                    "<span class=\"o-latex\" data-tex=\"{}\"{}>{}</span>",
                    escape_html(text),
                    self.attrs(t, " data-atomic data-select-whole"),
                    escape_html(text)
                )
            }
            Kind::Link { text, url, children, .. } => {
                let href = safe_url(url, false);
                let internal = href.as_deref().is_some_and(|h| o.is_internal.is_some_and(|f| f(h)));
                let title = if text.is_empty() || text == url { o.link_title.and_then(|f| f(url)) } else { None };
                let body = match (self.revealed(t), title) {
                    (Some(raw), _) => self.mark(&raw),
                    (None, Some(title)) => escape_html(&title),
                    (None, None) if !children.is_empty() => self.tokens(children),
                    _ => "Link".to_owned(),
                };
                let href_attr = href.as_deref().map_or(String::new(), |h| format!(" href=\"{}\"", escape_html(h)));
                if o.clean {
                    return format!("<a{href_attr}>{body}</a>");
                }
                let target = if internal || href.is_none() { "" } else { " target=\"_blank\" rel=\"noopener noreferrer\"" };
                let cls = if internal { "o-link o-link-internal" } else { "o-link" };
                format!("<a class=\"{cls}\"{href_attr}{target}{}>{body}</a>", self.attrs(t, " data-atomic"))
            }
            Kind::Image { text, url, .. } => {
                let src = safe_url(url, true);
                let alt = escape_html(text);
                if o.clean {
                    return match src {
                        Some(s) => format!("<img src=\"{}\" alt=\"{alt}\">", escape_html(&s)),
                        None => alt,
                    };
                }
                if let (true, Some(s)) = (o.inline_images, &src) {
                    return format!("<img class=\"o-img\" src=\"{}\" alt=\"{alt}\"{}>", escape_html(s), self.attrs(t, " data-atomic"));
                }
                let href_attr = src.as_deref().map_or(String::new(), |s| {
                    let s = escape_html(s);
                    format!(" href=\"{s}\" target=\"_blank\" rel=\"noopener noreferrer\" data-src=\"{s}\"")
                });
                let body = match self.revealed(t) {
                    Some(raw) => self.mark(&raw),
                    None if !text.is_empty() => self.mark(text),
                    None => "Image".to_owned(),
                };
                format!("<a class=\"o-image\"{href_attr}{}>{body}</a>", self.attrs(t, " data-atomic"))
            }
            Kind::Autolink { url } => {
                let Some(href) = safe_url(url, false) else { return self.text(url, t.start) };
                if o.clean {
                    return format!("<a href=\"{}\">{}</a>", escape_html(&href), escape_html(url));
                }
                let internal = o.is_internal.is_some_and(|f| f(&href));
                let title = o.link_title.and_then(|f| f(url));
                let cls = if internal { "o-link o-autolink o-link-internal" } else { "o-link o-autolink" };
                let target = if internal { "" } else { " target=\"_blank\" rel=\"noopener noreferrer\"" };
                format!(
                    "<a class=\"{cls}\" href=\"{}\"{target}{}>{}</a>",
                    escape_html(&href),
                    self.attrs(t, " data-atomic"),
                    self.mark(title.as_deref().unwrap_or(url))
                )
            }
            Kind::Tag { tag } => {
                if o.clean {
                    return escape_html(tag);
                }
                format!("<span class=\"o-tag\" data-tag=\"{}\"{}>{}</span>", escape_html(tag), self.attrs(t, " data-atomic"), self.mark(tag))
            }
            Kind::Date { text, date } => {
                if o.clean {
                    return escape_html(&format!("!({text})"));
                }
                let now = o.now.unwrap_or_else(now_ms);
                let fo = DateFormatOptions {
                    now,
                    hour12: o.hour12 != Some(false),
                    format: o.date_format.clone(),
                    format_no_time: o.date_format_no_time.clone(),
                };
                let mut cls = "o-date".to_owned();
                if o.mark_overdue && is_overdue(date, now) {
                    cls += " o-overdue";
                }
                let mut title = String::new();
                if let Some(r) = &date.recur {
                    cls += " o-recurring";
                    title = format!(" title=\"{}\"", escape_html(&describe_recurrence(r)));
                }
                format!(
                    "<span class=\"{cls}\" data-date=\"{}\"{title}{}>{}</span>",
                    escape_html(text),
                    self.attrs(t, " data-atomic"),
                    escape_html(&format_date_label(date, &fo))
                )
            }
        }
    }
}
