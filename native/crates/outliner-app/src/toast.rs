//! A short message at the bottom of the screen for a moment (`toast` in `apps/web/src/ui/dom.ts`, `.toast`):
//! "Replaced 3 occurrences", "Copied!".

use std::cell::RefCell;
use std::time::Duration;

use nsh_core::prelude::*;

use crate::sidebar::ui_text;
use crate::style::{self, num};

/// How long a message shows (the web's default).
const SHOWS: Duration = Duration::from_millis(2200);

#[derive(Clone, Default, PartialEq)]
struct Toast {
    text: Option<String>,
    serial: u64,
}

fn toast() -> Signal<Toast> {
    global::<Toast>()
}

thread_local! {
    /// The timer that takes the message away (kept so that it runs).
    static TIMER: RefCell<Option<Spawned<()>>> = const { RefCell::new(None) };
}

/// Show `text` for a moment; a message showing gives way to it.
pub fn show(text: impl Into<String>) {
    let serial = toast().with_untracked(|t| t.serial) + 1;
    toast().set(Toast { text: Some(text.into()), serial });
    let timer = set_timeout(SHOWS, move || {
        if toast().with_untracked(|t| t.serial) == serial {
            toast().update(|t| t.text = None);
        }
    });
    TIMER.with(|t| *t.borrow_mut() = Some(timer));
}

/// The message while one shows, centered above the bottom.
pub fn view_of() -> impl IntoElement {
    when(
        || toast().with(|t| t.text.is_some()),
        || {
            let size = style::efs();
            view()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .bottom(px(num("space.step11")))
                .items_center()
                .child(
                    ui_text(|| SharedString::from(toast().with(|t| t.text.clone().unwrap_or_default())), (size, size * num("type.ui_line")))
                        .test_id("toast")
                        .text_color(rgba(style::look().color("color.toast_fg")))
                        .bg(rgba(style::look().color("color.toast_bg")))
                        .py(px(num("space.step6")))
                        .px(px(num("space.step9")))
                        .rounded(px(num("radius.menu"))),
                )
        },
    )
}
