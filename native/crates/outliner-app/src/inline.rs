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

use crate::icons;
use crate::style::{color, FONT, MONO};

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
    /// The text's color and strike (a checked item's), for what does not inherit them: a chip's label is a text of
    /// its own.
    pub color: Rgba,
    pub struck: bool,
}

impl Look {
    pub fn new(size: f32, now: i64) -> Self {
        Look { size, now, hour12: true, mark_overdue: false, inline_images: false, color: color::fg(), struck: false }
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

fn span(t: &Token, look: &Look) -> AnyElement {
    match &t.kind {
        Kind::Text { text: s } | Kind::Escape { text: s } => text(s.clone()).into_element(),
        Kind::Bold { children } => text("").font_weight(FontWeight::BOLD).children(spans(children, look)).into_element(),
        Kind::Italic { children } => text("").italic().children(spans(children, look)).into_element(),
        Kind::Strike { children } => text("").line_through().children(spans(children, look)).into_element(),
        // `.o-highlight`: a translucent yellow behind the text
        Kind::Highlight { children } => text("")
            .bg(rgba(0xffdd0061))
            .rounded(px(2.0))
            .children(spans(children, look))
            .into_element(),
        // `.o-code`: Fira Mono at 8/9 of the text, red on a gray box. The web pads the box 5px each side; a span
        // here has no padding (narrow spaces would change the text that copies and screen readers get)
        Kind::Code { text: s, .. } => text(s.clone())
            .font_family(MONO)
            .text_size(em(look, 0.8889))
            .text_color(color::code_fg())
            .bg(color::code_bg())
            .rounded(px(4.0))
            .into_element(),
        // `.o-latex`: no math typesetting here (the web uses KaTeX): the source, in the math font's place a mono one
        Kind::Latex { text: s } => text(s.clone()).font_family(MONO).text_size(em(look, 0.8889)).into_element(),
        // `.o-link` keeps the text's color, underlined
        Kind::Link { text: s, url, children, .. } => text(core::link_label(s, url, children, None)).underline().into_element(),
        Kind::Autolink { url } => match core::safe_url(url, false) {
            Some(_) => text(url.clone()).underline().into_element(),
            None => text(url.clone()).into_element(),
        },
        Kind::Image { text: alt, url, .. } => image_span(alt, url, look),
        // `.o-tag`: gray, underlined
        Kind::Tag { tag } => text(tag.clone()).text_color(color::tag()).underline().into_element(),
        Kind::Date { date, .. } => date_chip(date, look).into_element(),
        // not inline: `blocks` takes code blocks out first; shown as their text if one gets here
        Kind::Codeblock { text: s, .. } => text(s.clone()).font_family(MONO).into_element(),
    }
}

/// `![alt](url)`: a link in the accent color, underlined as links are (`a.o-image`), or the picture with
/// `inline_images`.
fn image_span(alt: &str, url: &str, look: &Look) -> AnyElement {
    match core::safe_url(url, true) {
        Some(src) if look.inline_images => {
            // `.o-img`: at most the column's width, at its natural size otherwise
            image(src).max_w(px(480.0)).max_h(px(400.0)).align_text_bottom().into_element()
        }
        _ => text(if alt.is_empty() { "Image".to_owned() } else { alt.to_owned() })
            .text_color(color::accent())
            .underline()
            .into_element(),
    }
}

/// The label a date chip shows, and whether it is overdue.
pub fn date_label(spec: &DateSpec, look: &Look) -> (String, bool) {
    let mut o = DateFormatOptions::new(look.now);
    o.hour12 = look.hour12;
    // a checked item is done: its dates are not overdue
    (dates::format_date_label(spec, &o), look.mark_overdue && !look.struck && dates::is_overdue(spec, look.now))
}

/// `.o-date`: the label at 8/9 of the text, after a calendar icon, in a gray box with a border; reddish when
/// overdue. An inline view: its bottom at the bottom of the text's font, as the web's inline box sits.
fn date_chip(spec: &DateSpec, look: &Look) -> impl IntoElement {
    let (label, overdue) = date_label(spec, look);
    let size = look.size * 0.8889;
    let (bg, border) = if overdue { (rgba(0xd9534f1f), rgba(0xd9534f80)) } else { (color::code_bg(), color::date_border()) };
    view()
        .flex_row()
        .items_center()
        .align_text_bottom()
        .bg(bg)
        .border(px(1.0))
        .border_color(border)
        .rounded(px(4.0))
        .px(px(2.0))
        .child(
            // `::before`: a .8em calendar, 2px in and 4px before the label
            view().ml(px(2.0)).mr(px(4.0)).text_color(color::muted()).child(icons::calendar(size * 0.8)),
        )
        .child({
            // the label inherits the text's color and strike, as the web's inline box does
            let t = text(label)
                .font_family(FONT)
                .text_size(px(size))
                .line_height(LineHeight::Points(px(size * 1.2223)))
                .text_color(look.color);
            if look.struck { t.line_through() } else { t }
        })
}

/// A code block (`.o-codeblock`): mono at .85em in a bordered gray box. The web keeps its lines whole and scrolls
/// them sideways; here long lines wrap (NSH has no `white-space: pre` from Rust).
pub fn code_block(source: &str, look: &Look) -> impl IntoElement {
    let size = look.size * 0.85;
    view()
        .self_stretch()
        .my(px(2.0))
        .px(px(10.0))
        .py(px(8.0))
        .bg(color::chip())
        .border(px(1.0))
        .border_color(color::chip_border())
        .rounded(px(6.0))
        .child({
            // the text's color, as the block inherits it (`.o-codeblock` sets none), struck on a checked item
            let t = text(source.to_owned())
                .font_family(MONO)
                .text_size(px(size))
                .line_height(LineHeight::Points(px(size * 1.2223)))
                .text_color(look.color);
            if look.struck { t.line_through() } else { t }
        })
}
