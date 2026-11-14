//! The bars around the document, as the web app's: the top bar (`.topbar`), and on phones the editing toolbar
//! above the keyboard (`.mobile-bar`, Dynalist's default buttons). Buttons whose feature the native app does not
//! have yet are drawn but do nothing.

use nsh_base::button;
use nsh_core::prelude::*;

use crate::commands as cmd;
use crate::icons::icon;
use crate::store;
use crate::style::{self, color, num};

/// An icon button of the top bar (`.icon-btn`): 30×30 with a rounded hover on desktops, 44×46 edge to edge on
/// phones.
fn top_button(name: &'static str, label: &'static str, run: impl Fn() + 'static) -> impl IntoElement {
    let hover = color::hover();
    button()
        .h(px(num("space.topbar_tool")))
        .min_w(px(num("space.topbar_tool_min")))
        .px(px(num("space.topbar_tool_pad")))
        .rounded(px(num("radius.topbar_tool")))
        .items_center()
        .justify_center()
        .text_color(color::topbar_fg())
        .accessibility_label(Some(label.into()))
        .active(move |s| s.bg(hover))
        .on_press(run)
        .child(icon(name, num("space.topbar_icon")))
}

fn none() {}

/// The sync status's padding left and right (`.sync-status`, a web-only metric there).
const SYNC_PAD: f32 = 8.0;

pub fn topbar() -> impl IntoElement {
    let phone = style::phone();
    let bar = view()
        .test_id("topbar")
        .flex_row()
        .items_center()
        .h(px(style::topbar_h()))
        .px(px(num("space.topbar_pad")))
        .gap(px(num("space.topbar_gap")))
        .border_b(px(num("space.border")))
        .border_color(color::topbar_border())
        .bg(color::topbar_bg())
        .child(top_button("side", "Toggle sidebar", || crate::sidebar::toggle(None)));
    if phone {
        bar.child(view().flex_1())
            .child(top_button("undo", "Undo", || cmd::undo(false)))
            .child(top_button("select", "Select multiple items", none))
            .child(top_button("search", "Search", none))
            .child(top_button("lock", "Read mode", none))
            .child(top_button("menu", "Menu", none))
    } else {
        bar.child(top_button("back", "Back", cmd::zoom_out))
            .child(top_button("fwd", "Forward", none))
            .child(view().flex_1())
            .child(
                text("Local only")
                    .font_family(style::font())
                    .text_size(px(num("type.ui_small")))
                    .text_color(color::topbar_fg())
                    .px(px(SYNC_PAD)),
            )
            .child(top_button("search", "Search", none))
            .child(top_button("gear", "Settings", none))
            .child(top_button("menu", "Menu", none))
    }
}

/// A button of the phone toolbar: 40×41, a 20pt icon in the muted color. It acts on the item being edited.
fn tool(name: &'static str, _label: &'static str, run: impl Fn(&str, u32) + 'static) -> impl IntoElement {
    button()
        .w(px(num("space.toolbar_button")))
        .h(px(num("space.toolbar")))
        .items_center()
        .justify_center()
        .text_color(color::muted())
        .active(|s| s.opacity(0.5))
        .on_press(move || {
            if let Some(c) = store::peek(|s| s.editing.clone()) {
                run(&c.id, c.at);
            }
        })
        .child(icon(name, num("space.toolbar_icon")))
}

/// The editing toolbar of phones, shown while an item is being edited, right above the keyboard.
pub fn mobile_bar() -> impl IntoElement {
    when(
        || style::phone() && store::read(|s| s.editing.is_some()),
        || {
            scroll()
                .test_id("toolbar")
                .horizontal()
                .h(px(num("space.toolbar")))
                .border_t(px(num("space.border")))
                .border_color(color::line())
                .bg(color::bg())
                .child(
                    view()
                        .flex_row()
                        .child(tool("unindent", "Unindent", cmd::outdent))
                        .child(tool("indent", "Indent", cmd::indent))
                        .child(tool("zoom", "Zoom in", |id, _| cmd::zoom_in(id)))
                        .child(tool("trash", "Delete item", cmd::delete_item))
                        .child(tool("check", "Check off", |id, at| cmd::toggle_checked(id, at, true)))
                        .child(tool("note", "Edit note", |_, _| {}))
                        .child(tool("date", "Add date", |_, _| {}))
                        .child(tool("up", "Move up", cmd::move_up))
                        .child(tool("down", "Move down", cmd::move_down))
                        .child(tool("send", "Move item to", |_, _| {}))
                        .child(tool("undo", "Undo", |_, _| cmd::undo(false)))
                        .child(tool("redo", "Redo", |_, _| cmd::undo(true))),
                )
        },
    )
}
