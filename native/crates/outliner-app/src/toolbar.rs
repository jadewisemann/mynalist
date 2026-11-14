//! The editing toolbar for touch screens (no Tab or arrow keys there): shown while a row is being edited.

use nsh_base::{button, theme};
use nsh_core::prelude::*;

use crate::commands as cmd;
use crate::store;

fn tool(label: &'static str, run: impl Fn(&str) + 'static) -> impl IntoElement {
    button()
        .px(px(14.0))
        .py(px(10.0))
        .active(|s| s.opacity(0.5))
        .on_press(move || {
            if let Some(c) = store::peek(|s| s.editing.clone()) {
                run(&c.id);
            }
        })
        .child(text(label).text_color(move || theme().foreground))
}

pub fn toolbar() -> impl IntoElement {
    when(
        || store::read(|s| s.editing.is_some()),
        || {
            let caret = || store::peek(|s| s.editing.as_ref().map_or(0, |c| c.at));
            view()
                .flex_row()
                .justify_around()
                .border_t(px(1.0))
                .border_color(move || theme().border)
                .bg(move || theme().background)
                .pb_safe_area()
                .child(tool("⇤", move |id| cmd::outdent(id, caret())))
                .child(tool("⇥", move |id| cmd::indent(id, caret())))
                .child(tool("↑", |id| cmd::step(id, false)))
                .child(tool("↓", |id| cmd::step(id, true)))
                .child(tool("☑", cmd::toggle_checked))
                .child(tool("↶", |_| cmd::undo(false)))
                .child(tool("↷", |_| cmd::undo(true)))
        },
    )
}
