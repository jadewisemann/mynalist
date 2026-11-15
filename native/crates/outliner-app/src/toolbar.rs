//! The bars around the document, as the web app's: the top bar (`.topbar`), and on phones the editing toolbar
//! above the keyboard (`.mobile-bar`, Dynalist's default buttons). Buttons whose feature the native app does not
//! have yet are drawn but do nothing.

use nsh_base::button;
use nsh_core::prelude::*;

use outliner_core::commands::Dir;

use crate::commands as cmd;
use crate::icons::icon;
use crate::store;
use crate::style::{self, color, num};

/// An icon button of the top bar (`.icon-btn`): 30×30 with a rounded hover on desktops, 44×46 edge to edge on
/// phones.
fn top_button(name: &'static str, label: &'static str, run: impl Fn() + 'static) -> impl IntoElement {
    top_toggle(name, label, run, || false)
}

/// [`top_button`] in the "on" color while `on` (`.icon-btn[aria-pressed='true']`).
fn top_toggle(
    name: &'static str,
    label: &'static str,
    run: impl Fn() + 'static,
    on: impl Fn() -> bool + 'static,
) -> impl IntoElement {
    let hover = color::hover();
    button()
        .h(px(num("space.topbar_tool")))
        .min_w(px(num("space.topbar_tool_min")))
        .px(px(num("space.topbar_tool_pad")))
        .rounded(px(num("radius.topbar_tool")))
        .items_center()
        .justify_center()
        .text_color(move || if on() { color::topbar_fg_on() } else { color::topbar_fg() })
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
            .child(
                // pressed while select mode is on (`[aria-pressed='true']`)
                top_toggle(
                    "select",
                    "Select multiple items",
                    || cmd::set_select_mode(!store::peek(|s| s.select_mode)),
                    || store::read(|s| s.select_mode),
                ),
            )
            .child(top_button("search", "Search", || crate::find::open(None)))
            .child(top_button("lock", "Read mode", none))
            .child(top_button("menu", "Menu", crate::settings::toggle_menu))
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
            .child(top_button("search", "Search", || crate::find::open(None)))
            .child(top_button("gear", "Settings", || crate::settings::open(crate::settings::Tab::Prefs)))
            .child(top_button("menu", "Menu", crate::settings::toggle_menu))
    }
}

/// A button of the phone toolbar: 40×41, a 20pt icon in the muted color. It acts on the item being edited or the
/// selected items; `single` ones only on one item (hidden while several are selected, as on the web).
fn tool(name: &'static str, label: &'static str, single: bool, run: impl Fn() + 'static) -> impl IntoElement {
    let run = std::rc::Rc::new(run);
    let make = move || {
        let run = run.clone();
        button()
            .w(px(num("space.toolbar_button")))
            .h(px(num("space.toolbar")))
            .items_center()
            .justify_center()
            .text_color(color::muted())
            .accessibility_label(Some(label.into()))
            .active(|s| s.opacity(0.5))
            .on_press(move || run())
            .child(icon(name, num("space.toolbar_icon")))
    };
    when(move || !single || store::read(|s| s.block.as_ref().is_none_or(|b| b.ids.len() == 1)), make)
}

/// The editing toolbar of phones, right above the keyboard: shown while an item's content is being edited (not
/// its note) or items are selected.
pub fn mobile_bar() -> impl IntoElement {
    when(
        || style::phone() && store::read(|s| s.block.is_some() || s.editing.as_ref().is_some_and(|c| !c.note)),
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
                        .child(tool("unindent", "Unindent", false, cmd::outdent))
                        .child(tool("indent", "Indent", false, cmd::indent))
                        .child(tool("zoom", "Zoom in", true, cmd::zoom_in))
                        .child(tool("trash", "Delete item", false, || cmd::delete_items(Dir::Left)))
                        .child(tool("check", "Check off", false, || cmd::toggle_checked(true)))
                        .child(tool("note", "Edit note", true, cmd::goto_note))
                        .child(tool("date", "Add date", true, crate::datepicker::add_date))
                        .child(tool("up", "Move up", false, cmd::move_up))
                        .child(tool("down", "Move down", false, cmd::move_down))
                        .child(tool("send", "Move item to", false, || {}))
                        .child(tool("undo", "Undo", false, || cmd::undo(false)))
                        .child(tool("redo", "Redo", false, || cmd::undo(true))),
                )
        },
    )
}
