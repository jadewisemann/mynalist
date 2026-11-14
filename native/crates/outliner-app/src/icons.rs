//! The web app's icons (`PATHS` in `apps/web/src/ui/dom.ts`): 24×24 strokes in the text color.

use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

fn path(name: &str) -> &'static str {
    match name {
        "menu" => "M3 6h18M3 12h18M3 18h18",
        "side" => "M3 4h18v16H3zM9 4v16",
        "search" => "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM21 21l-5-5",
        "gear" => "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-1.8-.3 1.6 1.6 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.6 1.6 0 0 0-1-1.5 1.6 1.6 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0 .3-1.8 1.6 1.6 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.6 1.6 0 0 0 1.5-1 1.6 1.6 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.6 1.6 0 0 0 1.8.3H9a1.6 1.6 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.6 1.6 0 0 0 1 1.5 1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0-.3 1.8V9a1.6 1.6 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1z",
        "back" => "M15 18l-6-6 6-6",
        "fwd" => "M9 18l6-6-6-6",
        "undo" => "M9 14L4 9l5-5M4 9h11a5 5 0 0 1 0 10h-4",
        "redo" => "M15 14l5-5-5-5M20 9H9a5 5 0 0 0 0 10h4",
        "select" => "M4 4h10v10H4zM18 8v12H8M6.5 9l2 2 3.5-4",
        "lock" => "M6 11h12v10H6zM8 11V7a4 4 0 0 1 8 0v4M12 15v2",
        "unindent" => "M11 6h10M11 12h10M11 18h10M7 9l-3 3 3 3",
        "indent" => "M11 6h10M11 12h10M11 18h10M3 9l3 3-3 3",
        "zoom" => "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM21 21l-5-5M11 8v6M8 11h6",
        "trash" => "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6",
        "check" => "M5 12l5 5 9-10",
        "note" => "M5 4h14v16H5zM8 9h8M8 13h8M8 17h5",
        "date" => "M4 6h16v14H4zM4 10h16M8 3v4M16 3v4",
        "up" => "M12 19V5M6 11l6-6 6 6",
        "down" => "M12 5v14M6 13l6 6 6-6",
        "send" => "M21 3L3 11l7 3 3 7zM10 14L21 3",
        _ => "",
    }
}

/// The icon `name`, `size` points square, stroked in the text color around it.
pub fn icon(name: &str, size: f32) -> impl IntoElement {
    let markup = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="{}"/></svg>"#,
        path(name)
    );
    svg(ImageSrc::svg_markup(&markup)).w(px(size)).h(px(size))
}

/// The date chip's calendar (`.o-date::before`, a 16×16 shape filled in the text color), `size` points square.
pub fn calendar(size: f32) -> impl IntoElement {
    let markup = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path d="M4 0h2v2h4V0h2v2h3v14H1V2h3zM3 6v8h10V6z"/></svg>"#;
    svg(ImageSrc::svg_markup(markup)).w(px(size)).h(px(size))
}

/// A shape filled in the text color around it: the path `d` in a `w`×`h` box, drawn at that size.
pub fn shape(d: &str, w: f32, h: f32) -> impl IntoElement {
    let markup = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" fill="currentColor"><path d="{d}"/></svg>"#
    );
    svg(ImageSrc::svg_markup(&markup)).w(px(w)).h(px(h))
}
