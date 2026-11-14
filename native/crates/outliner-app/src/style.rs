//! The look, as the web app's: every value comes from the design tokens (`design/*.toml`, generated into
//! `crate::design`), resolved for the current modes, the user's appearance and the device by `crate::appearance`.
//! Nothing here holds a color or a size of its own; what is here is how the native views use the tokens.
//! Reading a value tracks the look: a view drawn from it is drawn again when the appearance changes.

use std::rc::Rc;

use nsh_core::platform::Os;
use nsh_core::prelude::*;

use crate::appearance::{self, Look, Looks};

/// The look of this device (tracked).
pub struct Current(Rc<Looks>, bool);

impl std::ops::Deref for Current {
    type Target = Look;
    fn deref(&self) -> &Look {
        if self.1 { &self.0.phone } else { &self.0.desktop }
    }
}

pub fn look() -> Current {
    Current(appearance::current(), phone())
}

/// A length, ratio or weight entry of the current look (`space.pad`, `type.h1`).
pub fn num(id: &str) -> f32 {
    look().number(id)
}

/// The tokens' colors (`[color]`, `[label]`).
pub mod color {
    use nsh_core::prelude::*;

    fn of(id: &str) -> Rgba {
        rgba(super::look().color(id))
    }

    pub fn bg() -> Rgba {
        of("color.bg")
    }
    pub fn fg() -> Rgba {
        of("color.fg")
    }
    pub fn muted() -> Rgba {
        of("color.muted")
    }
    pub fn faint() -> Rgba {
        of("color.faint")
    }
    pub fn line() -> Rgba {
        of("color.line")
    }
    pub fn guide() -> Rgba {
        of("color.guide")
    }
    pub fn check() -> Rgba {
        of("color.check")
    }
    pub fn tag() -> Rgba {
        of("color.tag")
    }
    pub fn code_fg() -> Rgba {
        of("color.code_fg")
    }
    pub fn code_bg() -> Rgba {
        of("color.code_bg")
    }
    pub fn accent() -> Rgba {
        of("color.accent")
    }
    /// The bullet thread and the bullets on it.
    pub fn thread() -> Rgba {
        of("color.thread")
    }
    /// Selected text's background: the accent color, `color.selection_mix` of it over the page
    /// (`.o-editor ::selection`).
    pub fn selection() -> Rgba {
        let mut c = accent();
        c.a *= super::num("color.selection_mix");
        c
    }
    pub fn hover() -> Rgba {
        of("color.hover")
    }
    /// Selected items and the active file.
    pub fn sel() -> Rgba {
        of("color.sel")
    }
    pub fn side_bg() -> Rgba {
        of("color.side_bg")
    }
    /// Behind sheets and the phone sidebar.
    pub fn scrim() -> Rgba {
        of("color.scrim")
    }
    /// The phone sidebar's shadow.
    pub fn shadow_strong() -> Rgba {
        of("color.shadow_strong")
    }
    pub fn note() -> Rgba {
        of("color.note")
    }
    pub fn chip() -> Rgba {
        of("color.chip")
    }
    pub fn chip_border() -> Rgba {
        of("color.chip_border")
    }
    pub fn date_border() -> Rgba {
        of("color.date_border")
    }
    pub fn highlight() -> Rgba {
        of("color.highlight")
    }
    pub fn overdue_bg() -> Rgba {
        of("color.overdue_bg")
    }
    pub fn overdue_border() -> Rgba {
        of("color.overdue_border")
    }
    pub fn topbar_border() -> Rgba {
        of("color.topbar_border")
    }
    pub fn topbar_fg() -> Rgba {
        of("color.topbar_fg")
    }
    /// The top bar's background: the page's where the bar has none of its own (the web's bar is transparent
    /// over the page).
    pub fn topbar_bg() -> Rgba {
        let look = super::look();
        let c = look.color("color.topbar_bg");
        rgba(if c & 0xff == 0 { look.color("color.bg") } else { c })
    }
    /// Color labels 1..6 (`[label]`).
    pub fn label(n: i64) -> Option<Rgba> {
        (1..=6).contains(&n).then(|| of(&format!("label.{n}")))
    }
}

/// The app's font (`font.ui`), the editor's (`font.editor`, East Asian font included) and the mono one, as CSS
/// font-family lists (NSH reads them as the web does).
pub fn font() -> String {
    look().text("font.ui").to_owned()
}
pub fn editor_font() -> String {
    look().text("font.editor").to_owned()
}
pub fn mono() -> String {
    look().text("font.mono").to_owned()
}

/// Editor type: the text size, and its line.
pub fn efs() -> f32 {
    num("type.size")
}
pub fn line() -> f32 {
    efs() * num("type.line")
}
/// Space above and below a row's text (density).
pub fn pad() -> f32 {
    num("space.pad")
}
/// The bullet's column, left of the text (`.o-bullet`), and the gap after it (`.o-body`).
pub fn bullet_w() -> f32 {
    num("space.bullet")
}
pub fn body_gap() -> f32 {
    num("space.body_gap")
}
pub fn dot() -> f32 {
    num("space.dot")
}

/// Whether this runs as a phone (no hardware keys assumed; the web's `max-width: 760px` layout, the `phone`
/// device mode). `OUTLINER_PHONE=1` lays a desktop build out as a phone, to look at it without one.
pub fn phone() -> bool {
    matches!(os(), Os::Ios | Os::Android) || std::env::var_os("OUTLINER_PHONE").is_some()
}

/// One level of indentation: the children block's indent less the guide's offset, plus the guide line, which is
/// its left border (`.o-kids`).
pub fn indent() -> f32 {
    num("space.indent") - num("space.guide") + num("space.guide_w")
}

/// The document's left padding (where depth-0 rows start, bullet included) and right padding.
pub fn doc_pad() -> (f32, f32) {
    (num("space.doc_left"), num("space.doc_right"))
}

/// The top bar's height, its bottom border included (the web's bar is that tall above its border).
pub fn topbar_h() -> f32 {
    num("space.topbar") + num("space.border")
}

/// Where a row's text starts, for `depth` (0 under the zoom root): the bullet column is just left of it.
pub fn text_left(depth: u16) -> f32 {
    doc_pad().0 + indent() * f32::from(depth) + body_gap()
}

/// The x of the bullet's center for `depth`.
pub fn bullet_center(depth: u16) -> f32 {
    doc_pad().0 + indent() * f32::from(depth) - bullet_w() / 2.0
}

/// The x of the guide line of the children of a row at `depth` (`.o-kids`' left border, half a bullet left of the
/// children's bullet column).
pub fn guide_x(depth: u16) -> f32 {
    doc_pad().0 + indent() * f32::from(depth) - num("space.guide")
}

/// The x of the center of the bullet thread on the guide of the children of a row at `depth`.
pub fn thread_x(depth: u16) -> f32 {
    guide_x(depth) + num("space.guide_w") / 2.0
}

/// Where the thread's turn into the bullet of a row at `depth` ends (`--elbow-end`, from the left of the row's
/// node box, which starts its gutter left of the bullet column's right edge); `numbered`: the row shows a number.
pub fn elbow_end(depth: u16, numbered: bool) -> f32 {
    let end = num(if numbered { "space.elbow_end_numbered" } else { "space.elbow_end" });
    doc_pad().0 + indent() * f32::from(depth) - num("space.gutter") + end
}

/// The app's text size (bars, menus, the sidebar) and its line height.
pub fn ui() -> (f32, f32) {
    ui_sized("type.ui")
}

/// A size entry of the app's text (`type.ui_small`) and its line height (the app's line height ratio).
pub fn ui_sized(id: &str) -> (f32, f32) {
    let size = num(id);
    (size, size * num("type.ui_line"))
}

/// Text size and line box of a heading level (`.o-h1` .. `.o-h3`; 0 for plain text).
pub fn heading(level: i64) -> (f32, f32) {
    match level {
        1..=3 => (efs() * num(&format!("type.h{level}")), efs() * num(&format!("type.h{level}_box"))),
        _ => (efs(), line()),
    }
}

/// A font weight entry (`type.heading_weight`).
pub fn weight(id: &str) -> FontWeight {
    FontWeight(num(id) as u16)
}
