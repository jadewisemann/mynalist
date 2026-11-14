//! The look, from the web app's `apps/web/src/styles.css` (its values measured against Dynalist, `qa/layout.mjs`):
//! colors, type and the outline's metrics, for desktops and phones.

use nsh_core::platform::Os;
use nsh_core::prelude::*;

/// The light theme's colors (`:root` in styles.css).
pub mod color {
    use nsh_core::prelude::*;

    pub fn bg() -> Rgba {
        rgb(0xffffff)
    }
    pub fn fg() -> Rgba {
        rgb(0x222222)
    }
    pub fn muted() -> Rgba {
        rgb(0x8b8f97)
    }
    pub fn faint() -> Rgba {
        rgb(0xc9ccd1)
    }
    pub fn line() -> Rgba {
        rgb(0xeceef1)
    }
    pub fn guide() -> Rgba {
        rgb(0xe3e5e8)
    }
    pub fn check() -> Rgba {
        rgb(0xa7a7a7)
    }
    pub fn tag() -> Rgba {
        rgb(0x7a7a7a)
    }
    pub fn code_fg() -> Rgba {
        rgb(0xc7254e)
    }
    pub fn code_bg() -> Rgba {
        rgb(0xf5f5f5)
    }
    pub fn accent() -> Rgba {
        rgb(0x2f8cf0)
    }
    pub fn note() -> Rgba {
        rgb(0x4e4e4e)
    }
    pub fn chip() -> Rgba {
        rgb(0xf1f2f4)
    }
    pub fn chip_border() -> Rgba {
        rgb(0xdfe1e5)
    }
    pub fn date_border() -> Rgba {
        rgb(0xcccccc)
    }
    /// Color labels 1..6 (`--cl1` .. `--cl6`).
    pub fn label(n: i64) -> Option<Rgba> {
        Some(rgb(match n {
            1 => 0xf9d0c4,
            2 => 0xfed8b1,
            3 => 0xfef2c0,
            4 => 0xc8e3c0,
            5 => 0xc5def5,
            6 => 0xd4c5f9,
            _ => return None,
        }))
    }
}

/// Fira Sans, as the web app (bundled in `assets/fonts`, SIL OFL); the platform's font covers what it lacks
/// (Hangul among others).
pub const FONT: &str = "Fira Sans, system-ui, sans-serif";
pub const MONO: &str = "Fira Mono, ui-monospace, monospace";

/// Editor type: 18px text on a 22px line, rows 4px apart above and below (a 30px pitch).
pub const EFS: f32 = 18.0;
pub const LINE: f32 = 22.0;
pub const PAD: f32 = 4.0;
/// The bullet's column, left of the text (`.o-bullet`), and the gap after it (`.o-body`).
pub const BULLET_W: f32 = 18.0;
pub const BODY_GAP: f32 = 6.0;
pub const DOT: f32 = 6.0;

/// Whether this runs as a phone (no hardware keys assumed; the web's `max-width: 760px` layout).
/// `OUTLINER_PHONE=1` lays a desktop build out as a phone, to look at it without one.
pub fn phone() -> bool {
    matches!(os(), Os::Ios | Os::Android) || std::env::var_os("OUTLINER_PHONE").is_some()
}

/// One level of indentation: the children block's `padding-left` minus its `margin-left`.
pub fn indent() -> f32 {
    if phone() { 29.0 - 9.0 } else { 38.0 - 9.0 }
}

/// The document's left padding (where depth-0 rows start, bullet included) and right padding.
pub fn doc_pad() -> (f32, f32) {
    if phone() { (30.0, 14.0) } else { (70.0, 40.0) }
}

/// The top bar's height.
pub fn topbar_h() -> f32 {
    if phone() { 46.0 } else { 44.0 }
}

/// Where a row's text starts, for `depth` (0 under the zoom root): the bullet column is just left of it.
pub fn text_left(depth: u16) -> f32 {
    doc_pad().0 + indent() * f32::from(depth) + BODY_GAP
}

/// The x of the bullet's center for `depth`, where the guide line of its children runs.
pub fn bullet_center(depth: u16) -> f32 {
    doc_pad().0 + indent() * f32::from(depth) - BULLET_W / 2.0
}
