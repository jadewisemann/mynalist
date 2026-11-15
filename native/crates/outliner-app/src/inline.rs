//! Drawing inline source as the web app shows it: the tokens of `outliner_core::inline` made into NSH text spans,
//! with the styles of the `.o-*` classes in `apps/web/src/styles.css` (the web's renderer is `renderInline` in
//! `packages/core/src/inline.ts`).
//!
//! A row's text is one NSH text of nested spans, except code blocks: they are blocks of their own (`display:
//! block` on the web), so the text is cut around them ([`blocks`]). Date chips are inline views (a chip has a
//! border, an icon and padding, which spans do not have); an inline view stands as U+FFFC in its text, which only
//! matters for copying the drawn text, and rows are copied from their source.

use nsh_core::prelude::*;
use nsh_core::LineHeight;
use outliner_core::dates::{self, DateFormatOptions, DateSpec};
use outliner_core::inline::{self as core, Kind, Token};
use outliner_core::text16;

use crate::icons;
use crate::style::{self, color, num};

/// How a row's text is drawn: its type size and the display options of the web app's renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// The text's size (the row's, a heading's or a note's): `em` units of the styles are of it.
    pub size: f32,
    /// The current time, for date labels and overdue dates.
    pub now: i64,
    /// 12h clock in date labels (the web's default).
    pub hour12: bool,
    /// Past dates get the overdue look (`highlight_overdue`, off by default; never on checked items).
    pub mark_overdue: bool,
    /// `![]()` shows the picture instead of a link (`display_images_inline`, off by default).
    pub inline_images: bool,
    /// The date display formats (`custom_date_display_format`, `.._no_time`): "" or `relative` for the relative
    /// labels, else a pattern.
    pub format: String,
    pub format_no_time: String,
    /// The text's color and strike (a checked item's), for what does not inherit them: a chip's label is a text of
    /// its own.
    pub color: Rgba,
    pub struck: bool,
    /// A search's words to mark in the text (`.o-match`), and the source the tokens came from (a link whose URL
    /// holds a match shows its source, as the web's renderer reveals it).
    pub marks: Vec<String>,
    pub src: String,
    /// Links and tags take presses (the outline's rows; not the results' rows, which open their item).
    pub pressable: bool,
    /// The item and field (its note with `true`) the text is: a press on a date chip opens the date picker on it.
    pub dates_of: Option<(String, bool)>,
}

impl Look {
    /// The look of text of `size` at time `now`, with the display settings in use.
    pub fn new(size: f32, now: i64) -> Self {
        let p = crate::settings::get();
        Look {
            size,
            now,
            hour12: p.pick("time_format") == "12h",
            mark_overdue: p.flag("highlight_overdue"),
            inline_images: p.flag("display_images_inline"),
            format: p.text("custom_date_display_format").to_owned(),
            format_no_time: p.text("custom_date_display_format_no_time").to_owned(),
            color: color::fg(),
            struck: false,
            marks: Vec::new(),
            src: String::new(),
            pressable: false,
            dates_of: None,
        }
    }

    /// The same look marking `terms` in text parsed from `src`.
    pub fn marking(self, terms: &[String], src: &str) -> Self {
        Look { marks: terms.iter().filter(|t| !t.is_empty()).cloned().collect(), src: src.to_owned(), ..self }
    }

    /// The same look whose date chips open the date picker on item `id`'s content (or note).
    pub fn dates_of(self, id: &str, note: bool) -> Self {
        Look { dates_of: Some((id.to_owned(), note)), ..self }
    }

    /// The same look with links and tags that take presses.
    pub fn pressable(self) -> Self {
        Look { pressable: true, ..self }
    }

    /// The same look for text in `color`, struck through or not.
    pub fn colored(self, color: Rgba, struck: bool) -> Self {
        Look { color, struck, ..self }
    }
}

/// A piece of a row's text: inline tokens (one NSH text), or a code block.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Inline(Vec<Token>),
    Code { lang: String, text: String },
}

/// Cut a row's tokens around its code blocks.
pub fn blocks(src: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut run: Vec<Token> = Vec::new();
    for t in core::parse_inline(src) {
        match t.kind {
            Kind::Codeblock { lang, text } => {
                // the line break before a block ends the line above (a `<br>` before a block shows no empty line)
                if let Some(Token { kind: Kind::Text { text }, end, inner_end, .. }) = run.last_mut() {
                    if text.ends_with('\n') {
                        text.pop();
                        *end -= 1;
                        *inner_end -= 1;
                    }
                }
                run.retain(|t| !matches!(&t.kind, Kind::Text { text } if text.is_empty()));
                if !run.is_empty() {
                    out.push(Block::Inline(std::mem::take(&mut run)));
                }
                out.push(Block::Code { lang, text });
            }
            _ => run.push(t),
        }
    }
    if !run.is_empty() || out.is_empty() {
        out.push(Block::Inline(run));
    }
    out
}

/// The spans of tokens, for a text that has the row's font, size and color.
pub fn spans(tokens: &[Token], look: &Look) -> Vec<AnyElement> {
    tokens.iter().map(|t| span(t, look)).collect()
}

fn em(look: &Look, k: f32) -> Length {
    px(look.size * k).into()
}

/// `s` with the look's search words marked (`<mark class="o-match">`: the match color behind them).
fn marked(s: &str, look: &Look) -> Text {
    if look.marks.is_empty() {
        return text(s.to_owned());
    }
    let terms: Vec<&str> = look.marks.iter().map(String::as_str).collect();
    let ranges = core::match_ranges(s, &terms);
    if ranges.is_empty() {
        return text(s.to_owned());
    }
    let mut parts: Vec<AnyElement> = Vec::new();
    let mut last = 0;
    for (a, b) in ranges {
        if a > last {
            parts.push(text(text16::slice(s, last, a).to_owned()).into_element());
        }
        parts.push(
            text(text16::slice(s, a, b).to_owned())
                .bg(color::matched())
                .text_color(color::matched_fg())
                .rounded(px(num("radius.mark")))
                .into_element(),
        );
        last = b;
    }
    if last < text16::len(s) {
        parts.push(text(text16::slice_from(s, last).to_owned()).into_element());
    }
    text("").children(parts)
}

/// A link's or an image's source when a search match in it falls outside its shown text (in the URL or the
/// brackets): the link then shows its markup, so the match can be seen (the renderer's `revealed`).
fn revealed(t: &Token, look: &Look) -> Option<String> {
    if look.marks.is_empty() || look.src.is_empty() {
        return None;
    }
    let raw = text16::slice(&look.src, t.start, t.end);
    let (a, b) = (t.inner_start - t.start, t.inner_end - t.start);
    let terms: Vec<&str> = look.marks.iter().map(String::as_str).collect();
    core::match_ranges(raw, &terms).iter().any(|&(s, e)| s < a || e > b).then(|| raw.to_owned())
}

/// A link's span: pressing it opens it (an item of the app, or the browser) when the look takes presses.
fn link(span: Text, url: &str, look: &Look) -> AnyElement {
    match core::safe_url(url, false) {
        Some(href) if look.pressable => span.on_press(move || crate::links::open(&href)).into_element(),
        _ => span.into_element(),
    }
}

fn span(t: &Token, look: &Look) -> AnyElement {
    match &t.kind {
        Kind::Text { text: s } => marked(s, look).into_element(),
        Kind::Escape { text: s } => text(s.clone()).into_element(),
        Kind::Bold { children } => text("").font_weight(style::weight("type.bold_weight")).children(spans(children, look)).into_element(),
        Kind::Italic { children } => text("").italic().children(spans(children, look)).into_element(),
        Kind::Strike { children } => text("").line_through().children(spans(children, look)).into_element(),
        // `.o-highlight`: a translucent yellow behind the text
        Kind::Highlight { children } => text("")
            .bg(color::highlight())
            .rounded(px(num("radius.mark")))
            .children(spans(children, look))
            .into_element(),
        // `.o-code`: Fira Mono at 8/9 of the text, red on a gray box. The web pads the box 5px each side; a span
        // here has no padding (narrow spaces would change the text that copies and screen readers get)
        Kind::Code { text: s, .. } => marked(s, look)
            .font_family(style::mono())
            .text_size(em(look, num("type.code")))
            .text_color(color::code_fg())
            .bg(color::code_bg())
            .rounded(px(num("radius.code")))
            .into_element(),
        // `.o-latex`: no math typesetting here (the web uses KaTeX): the source, in the math font's place a mono one
        Kind::Latex { text: s } => text(s.clone()).font_family(style::mono()).text_size(em(look, num("type.code"))).into_element(),
        // `.o-link` keeps the text's color, underlined
        Kind::Link { text: s, url, children, .. } => {
            let shown = match revealed(t, look) {
                Some(raw) => marked(&raw, look),
                None => marked(&core::link_label(s, url, children, None), look),
            };
            link(shown.underline(), url, look)
        }
        Kind::Autolink { url } => match core::safe_url(url, false) {
            Some(_) => link(marked(url, look).underline(), url, look),
            None => marked(url, look).into_element(),
        },
        Kind::Image { text: alt, url, .. } => image_span(t, alt, url, look),
        // `.o-tag`: gray, underlined; a press adds it to the search or takes it out (`toggleSearchTerm`)
        Kind::Tag { tag } => {
            let span = marked(tag, look).text_color(color::tag()).underline();
            if look.pressable {
                let tag = tag.clone();
                span.on_press(move || crate::find::toggle_term(&tag)).into_element()
            } else {
                span.into_element()
            }
        }
        Kind::Date { date, .. } => {
            let chip = date_chip(date, look);
            match look.dates_of.clone() {
                // a press opens the date picker on that date, under the chip (`openDateChip`)
                Some((id, note)) => {
                    let (s, e) = (t.start as u32, t.end as u32);
                    // the chip's height: its label's line and its border
                    let h = look.size * num("type.date") * num("type.line") + 2.0 * num("space.border");
                    chip.test_id("date-chip")
                        .on_press(move |p: &PressEvent| {
                            let x = p.page_position.x.0 - p.position.x.0;
                            let top = p.page_position.y.0 - p.position.y.0;
                            crate::datepicker::open_chip(&id, note, s, e, (x, top, top + h));
                        })
                        .into_element()
                }
                None => chip.into_element(),
            }
        }
        // not inline: `blocks` takes code blocks out first; shown as their text if one gets here
        Kind::Codeblock { text: s, .. } => text(s.clone()).font_family(style::mono()).into_element(),
    }
}

/// `![alt](url)`: a link in the accent color, underlined as links are (`a.o-image`), or the picture with
/// `inline_images`.
fn image_span(t: &Token, alt: &str, url: &str, look: &Look) -> AnyElement {
    match core::safe_url(url, true) {
        Some(src) if look.inline_images => {
            // `.o-img`: at most the column's width, at its natural size otherwise
            image(src).max_w(px(480.0)).max_h(px(400.0)).align_text_bottom().into_element()
        }
        _ => {
            let shown = match revealed(t, look) {
                Some(raw) => marked(&raw, look),
                None if alt.is_empty() => text("Image"),
                None => marked(alt, look),
            };
            link(shown.text_color(color::accent()).underline(), url, look)
        }
    }
}

/// The label a date chip shows, and whether it is overdue.
pub fn date_label(spec: &DateSpec, look: &Look) -> (String, bool) {
    let mut o = DateFormatOptions::new(look.now);
    o.hour12 = look.hour12;
    o.format = Some(look.format.clone()).filter(|f| !f.is_empty());
    o.format_no_time = Some(look.format_no_time.clone()).filter(|f| !f.is_empty());
    // a checked item is done: its dates are not overdue
    (dates::format_date_label(spec, &o), look.mark_overdue && !look.struck && dates::is_overdue(spec, look.now))
}

/// `.o-date`: the label at 8/9 of the text, after a calendar icon, in a gray box with a border; reddish when
/// overdue. An inline view: its bottom at the bottom of the text's font, as the web's inline box sits.
fn date_chip(spec: &DateSpec, look: &Look) -> View {
    let (label, overdue) = date_label(spec, look);
    let size = look.size * num("type.date");
    let (bg, border) =
        if overdue { (color::overdue_bg(), color::overdue_border()) } else { (color::code_bg(), color::date_border()) };
    view()
        .flex_row()
        .items_center()
        .align_text_bottom()
        .bg(bg)
        .border(px(num("space.border")))
        .border_color(border)
        .rounded(px(num("radius.code")))
        .px(px(num("space.date_pad")))
        .child(
            // `::before`: a .8em calendar, 2px in and 4px before the label
            view().ml(px(2.0)).mr(px(4.0)).text_color(color::muted()).child(icons::calendar(size * 0.8)),
        )
        .child({
            // the label inherits the text's color and strike, as the web's inline box does
            let t = text(label)
                .font_family(style::editor_font())
                .text_size(px(size))
                .line_height(LineHeight::Points(px(size * num("type.line"))))
                .text_color(look.color);
            if look.struck { t.line_through() } else { t }
        })
}

/// A code block (`.o-codeblock`): mono at .85em in a bordered gray box. The web keeps its lines whole and scrolls
/// them sideways; here long lines wrap (NSH has no `white-space: pre` from Rust).
pub fn code_block(source: &str, look: &Look) -> impl IntoElement {
    let size = look.size * num("type.codeblock");
    view()
        .self_stretch()
        .my(px(num("space.codeblock_margin")))
        .px(px(num("space.codeblock_x")))
        .py(px(num("space.codeblock_y")))
        .bg(color::chip())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.block")))
        .child({
            // the text's color, as the block inherits it (`.o-codeblock` sets none), struck on a checked item
            let t = marked(source, look)
                .font_family(style::mono())
                .text_size(px(size))
                .line_height(LineHeight::Points(px(size * num("type.line"))))
                .text_color(look.color);
            if look.struck { t.line_through() } else { t }
        })
}
