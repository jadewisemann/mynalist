//! The item menu (the web's `ui/itemmenu.ts`): a right click on a bullet (or the title), or a long press on
//! phones, opens it on that item, acting on the selected items when it is one of them. Its entries, their order
//! and labels are the web's (Dynalist's), the keys of the same commands at their right; the color labels and the
//! heading levels are swatches at its end (`swatches`).
//!
//! The hover toolbar (`ui/hoverbar.ts`, "Show item toolbar on hover", `item_toolbar_hover`): on desktops, a bar of
//! buttons above the right end of the row under the pointer.

use nsh_core::prelude::*;
use outliner_core::commands::SORT_LABELS;
use outliner_core::view::visible_children;
use outliner_core::{item, NodeId, ROOT};

use crate::dialog::{self, Button};
use crate::menu::{self, Item};
use crate::sidebar::ui_text;
use crate::store;
use crate::style::{self, color, num};
use crate::{commands as cmd, keymap, toast, workspace};

/// The items a command of the menu (or the hover bar) opened on `id` acts on: the selected items when `id` is one
/// of them, else `id`.
fn ids_for(id: &str) -> Vec<NodeId> {
    store::peek(|s| match &s.block {
        Some(b) if b.ids.iter().any(|x| x == id) => b.ids.clone(),
        _ => vec![id.to_owned()],
    })
}

/// The first key of command `id`, as the menu shows it.
fn key_of(id: &str) -> Option<String> {
    keymap::current().keys_for(id).first().map(|k| keymap::pretty(k, apple_keys()))
}

/// The item menu of `id` at (`x`, `y`) in the window (`itemMenu`).
pub fn open(id: &str, x: f32, y: f32) {
    let ids = ids_for(id);
    let Some((zoom, kids, shut, has_note, first, parent, all_cb, kids_cb, all_ck, numbered)) = store::peek(|s| {
        let t = s.tree();
        let n = t.get(id)?;
        let zoom = s.view.zoom.clone();
        let parent = n.parent.clone();
        let first = parent.as_ref().and_then(|p| visible_children(t, p, &s.view).first().cloned());
        let prop = |i: &str, k: &str| t.get(i).is_some_and(|n| n.props.get(k).is_some_and(|v| !v.is_null() && *v != false));
        Some((
            zoom,
            !n.children.is_empty(),
            n.collapsed && !n.children.is_empty(),
            !item::note(Some(n)).is_empty(),
            first,
            parent,
            ids.iter().all(|i| prop(i, "cb")),
            !n.children.is_empty() && n.children.iter().all(|c| prop(c, "cb")),
            ids.iter().all(|i| prop(i, "ck")),
            prop(id, "num"),
        ))
    }) else {
        return;
    };
    let multi = ids.len() > 1;
    let root = id == zoom;
    let on = |label: &str, c: &'static str| -> Item {
        let ids = ids.clone();
        menu::item(label, move || {
            cmd::exec_on(&ids, c);
        })
        .key(key_of(c))
    };
    let mut items: Vec<Item> = Vec::new();
    if kids && !root {
        items.push(on(if shut { "Expand" } else { "Collapse" }, "collapse"));
    }
    if kids {
        items.push(on(if shut { "Expand all" } else { "Collapse all" }, "collapse-all"));
    }
    items.push(expand_levels(ids.clone()));
    if kids && !root {
        items.push(on(if shut { "Expand all siblings" } else { "Collapse all siblings" }, "toggle-siblings"));
    }
    if !multi && !root {
        let id = id.to_owned();
        items.push(menu::item("Zoom in", move || cmd::zoom_into(&id)).key(key_of("zoom-in")));
    }
    if !multi {
        items.push(on(if has_note { "Edit note" } else { "Add note" }, "goto-note-of").key(key_of("goto-note")));
    }
    if !root {
        items.push(on("Delete", "delete-left"));
    }
    items.push(on("Delete checked items", "delete-checked"));
    if !multi {
        let id = id.to_owned();
        items.push(menu::item("Add to bookmarks", move || bookmark(&id)));
    }
    items.push(menu::sub(
        "Sort",
        SORT_LABELS
            .iter()
            .map(|(mode, label)| {
                let ids = ids.clone();
                let name = serde_json::to_value(mode).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
                menu::item(*label, move || {
                    cmd::exec_on(&ids, &format!("sort-{name}"));
                })
            })
            .collect(),
    ));
    items.push(menu::item("Search and replace…", crate::find::replace_dialog).key(key_of("search-replace")));
    items.push(menu::sep());
    if !root {
        if first.as_deref() != ids.first().map(String::as_str) {
            items.push(on("Indent", "indent"));
        }
        if parent.as_deref() != Some(zoom.as_str()) {
            items.push(on("Unindent", "unindent"));
        }
        let (ids2, doc) = (ids.clone(), store::peek(|s| s.doc.clone()));
        items.push(menu::item("Move to…", move || crate::finder::move_items_to(ids2.clone(), doc.clone())).key(key_of("move-to")));
        let first_id = ids[0].clone();
        items.push(
            menu::item("Insert template…", move || {
                store::edit(|s| {
                    let at = cmd::utf16_len(item::content(s.tree().get(&first_id)));
                    s.block = None;
                    s.editing = Some(store::Caret::text(&first_id, at));
                    s.caret_rev += 1;
                });
                crate::finder::insert_template();
            })
            .key(key_of("insert-template")),
        );
        items.push(on("Duplicate", "duplicate"));
    }
    items.push(menu::sep());
    items.push(on(if all_cb { "Remove checkbox" } else { "Add checkbox" }, "toggle-checkbox"));
    if kids {
        items.push(on(if kids_cb { "Remove checkbox from children" } else { "Add checkbox to children" }, "toggle-children-checkbox"));
    }
    if !root {
        items.push(on(if all_ck { "Uncheck" } else { "Check off" }, "toggle-checked"));
    }
    if kids {
        items.push(on("Uncheck all children", "uncheck-children"));
    }
    items.push(on(if numbered { "Stop numbering children" } else { "Number children" }, "toggle-numbered"));
    items.push(menu::sep());
    if !multi {
        let (a, b) = (id.to_owned(), id.to_owned());
        items.push(menu::item("Get link", move || get_link(&a)));
        items.push(menu::item("Show all references", move || crate::links::references(&b)));
    }
    {
        let ids = ids.clone();
        items.push(menu::item("Export…", move || crate::export::open_items(ids.clone())));
    }
    if !multi {
        let (doc, id) = (store::peek(|s| s.doc.clone()), id.to_owned());
        items.push(menu::item("Set as inbox", move || crate::inbox::set(&doc, &id)));
    }
    items.push(menu::sep());
    items.push(swatch_row("colors", id, &ids, true));
    items.push(swatch_row("headings", id, &ids, false));
    // no separator first, last or twice in a row
    let mut clean: Vec<Item> = Vec::new();
    for it in items {
        let sep = it.label.is_empty();
        if sep && clean.last().is_none_or(|l| l.label.is_empty()) {
            continue;
        }
        clean.push(it);
    }
    if clean.last().is_some_and(|l| l.label.is_empty()) {
        clean.pop();
    }
    menu::show(x, y, clean);
}

/// "Expand to level" with its four buttons (`custom` in the web's menu).
fn expand_levels(ids: Vec<NodeId>) -> Item {
    let (size, line) = style::ui();
    let h = line + 2.0 * num("space.menu_item_y");
    menu::custom("Expand to level…", h, move || {
        let ids = ids.clone();
        view()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(num("space.menu_item_x")))
            .py(px(num("space.menu_item_y")))
            .child(ui_text("Expand to level", (size, line)).text_color(color::fg()))
            .child(view().flex_row().gap(px(num("space.step2"))).children((1..=4).map(|lv| {
                let ids = ids.clone();
                view()
                    .test_id("expand-level")
                    .px(px(num("space.step6")))
                    .border(px(num("space.border")))
                    .border_color(color::chip_border())
                    .rounded(px(num("radius.block")))
                    .on_press(move || {
                        menu::close();
                        cmd::exec_on(&ids, &format!("expand-{lv}"));
                    })
                    .child(ui_text(lv.to_string(), (size, line)).text_color(color::fg()))
            })))
            .into_element()
    })
}

/// The swatches' row of the menu.
fn swatch_row(label: &str, id: &str, ids: &[NodeId], colors: bool) -> Item {
    let h = num("space.swatch_h") + 2.0 * num("space.step5");
    let (id, ids) = (id.to_owned(), ids.to_vec());
    menu::custom(label, h, move || swatches(&id, &ids, colors).into_element())
}

/// Color label (`colors`) or heading swatches for `ids` (`swatches`): the current value of `id` (when all of
/// `ids` have it) is outlined in the accent color.
pub fn swatches(id: &str, ids: &[NodeId], colors: bool) -> impl IntoElement {
    let key = if colors { "cl" } else { "h" };
    let value = |s: &store::State, i: &str| {
        s.tree().get(i).and_then(|n| n.props.get(key)).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as i64
    };
    let cur = store::peek(|s| {
        let v = value(s, id);
        ids.iter().all(|i| value(s, i) == v).then_some(v)
    });
    let (size, line) = style::ui_sized("type.ui_tiny");
    let max = if colors { 6 } else { 3 };
    view()
        .test_id(if colors { "swatches-color" } else { "swatches-heading" })
        .flex_row()
        .gap(px(num("space.step4")))
        .px(px(num("space.step8")))
        .py(px(num("space.step5")))
        .children((0..=max).map(|v: i64| {
            let ids = ids.to_vec();
            let text = match (v, colors) {
                (0, _) => "∅".to_owned(),
                (_, true) => "A".to_owned(),
                (_, false) => format!("H{v}"),
            };
            let c = if colors { format!("color-{v}") } else { format!("heading-{v}") };
            let current = cur == Some(v);
            let bg = if colors { color::label(v) } else { None };
            let mut sw = view()
                .test_id(if current { "swatch-current" } else { "swatch" })
                .w(px(num("space.step11")))
                .h(px(num("space.swatch_h")))
                .items_center()
                .justify_center()
                .rounded(px(num("radius.code")))
                .border(px(if current { 2.0 * num("space.border") } else { num("space.border") }))
                .border_color(if current { color::accent() } else { color::chip_border() })
                .on_press(move || {
                    menu::close();
                    cmd::exec_on(&ids, &c);
                })
                .child(ui_text(text, (size, line)).text_color(color::fg()));
            if let Some(bg) = bg {
                sw = sw.bg(bg);
            }
            sw
        }))
}

/// "Add to bookmarks" of the item menu: item `id`, named after its text (the document's name at the top).
fn bookmark(id: &str) {
    let (doc, title) = store::peek(|s| {
        let plain = outliner_core::inline::inline_to_plain(item::content(s.tree().get(id)), None);
        let title = if id == ROOT { s.title(&s.doc) } else if plain.trim().is_empty() { "Untitled".to_owned() } else { plain };
        (s.doc.clone(), title)
    });
    workspace::add_bookmark(&title, &doc, (id != ROOT).then_some(id));
    toast::show("Bookmarked");
}

/// The link of item `id` in the open document.
fn url_of(id: &str) -> String {
    let doc = store::peek(|s| s.doc.clone());
    crate::links::item_url(&doc, id)
}

/// "Get link" (`getLinkDialog`): the item's link in a field, with Copy.
pub fn get_link(id: &str) {
    let url = url_of(id);
    dialog::show(move || {
        let url = url.clone();
        let field = dialog::input("link-url", "").value(url.clone());
        let body = dialog::field("Item URL", field);
        dialog::modal(
            "get-link",
            "Link to this item",
            body,
            vec![
                Button::new("Copy", false, move || {
                    set_clipboard(url.clone());
                    toast::show("Link copied to your clipboard!");
                    false
                }),
                Button { label: "Done", primary: true, run: None },
            ],
        )
    });
}

/// The keymap's "Copy link to item": the link of the item acted on, to the clipboard.
pub fn copy_link_here() {
    if let Some(id) = cmd::exec_targets().into_iter().next() {
        set_clipboard(url_of(&id));
        toast::show("Link copied to your clipboard!");
    }
}

/// The keymap's "Get link to item": of the item acted on, or the zoom root.
pub fn get_link_here() {
    let id = cmd::exec_targets().into_iter().next().unwrap_or_else(cmd::zoom);
    get_link(&id);
}

/// The keymap's "Show all references": of the item acted on, or the zoom root.
pub fn references_here() {
    let id = cmd::exec_targets().into_iter().next().unwrap_or_else(cmd::zoom);
    crate::links::references(&id);
}

// ---- the hover toolbar --------------------------------------------------------------------------------------

/// The row the pointer is over (`id`), and whether it is over the bar itself.
#[derive(Debug, Clone, PartialEq, Default)]
struct Hover {
    id: Option<NodeId>,
    on_bar: bool,
    /// Bumped by each leave, so a later enter cancels an earlier leave's hiding.
    rev: u64,
}

fn hover() -> Signal<Hover> {
    global::<Hover>()
}

/// How long the bar stays after the pointer leaves the row, so it can travel onto the bar (`HIDE_MS`).
const HIDE: std::time::Duration = std::time::Duration::from_millis(250);

/// Whether the hover bar is on (desktops with `item_toolbar_hover`).
pub fn bar_on() -> bool {
    !style::phone() && crate::settings::get().flag("item_toolbar_hover")
}

/// The pointer came over row `id`.
pub fn entered(id: &str) {
    hover().update(|h| {
        h.rev += 1;
        h.id = Some(id.to_owned());
    });
}

/// The pointer left row `id` (or the bar): the bar goes after a moment, unless it came back.
pub fn left() {
    let rev = hover().with_untracked(|h| h.rev) + 1;
    hover().update(|h| h.rev = rev);
    Owner::detached().run(|| {
        drop(set_timeout(HIDE, move || {
            hover().update(|h| {
                if h.rev == rev && !h.on_bar {
                    h.id = None;
                }
            })
        }));
    });
}

/// Whether the bar shows over row `id` (tracked).
pub fn shows_on(id: &str) -> bool {
    hover().with(|h| h.id.as_deref() == Some(id))
}

/// The bar's buttons: what each does, its glyph and its name (`BUTTONS`; the web's "Upload a file" is left out:
/// the native app has no uploads).
const BUTTONS: [(&str, &str, &str); 13] = [
    ("unindent", "⇤", "Unindent"),
    ("indent", "⇥", "Indent"),
    ("zoom-in", "⊕", "Zoom in"),
    ("delete", "🗑", "Delete"),
    ("check", "✓", "Check off"),
    ("note", "✎", "Edit note"),
    ("date", "◷", "Add date"),
    ("checkbox", "☐", "Toggle checkbox"),
    ("color", "🎨", "Set color label"),
    ("heading", "H", "Set heading level"),
    ("duplicate", "⧉", "Duplicate"),
    ("export", "⇪", "Export item"),
    ("newline", "↵", "Insert new line"),
];

/// The hover bar of row `id`, `row_h` high (`.hover-bar`): above the row's right end, in the row's cell.
pub fn bar(id: &str) -> impl IntoElement {
    let (indent_off, unindent_off, done) = store::peek(|s| {
        let t = s.tree();
        let ids = ids_for(id);
        let n = t.get(id);
        let parent = n.and_then(|n| n.parent.clone());
        let first = parent.as_ref().and_then(|p| visible_children(t, p, &s.view).first().cloned());
        let done = ids.iter().all(|i| item::is_checked(t.get(i)));
        (first.as_deref() == ids.first().map(String::as_str), parent.as_deref() == Some(s.view.zoom.as_str()), done)
    });
    let (size, line) = style::ui_sized("type.ui_small");
    let btn_h = num("space.step11");
    let bar_h = btn_h + 2.0 * num("space.step2") + 2.0 * num("space.border");
    let hover_bg = color::hover();
    view()
        .test_id("hover-bar")
        .absolute()
        .top(px(-bar_h))
        .right(px(style::doc_pad().1))
        .flex_row()
        .gap(px(num("space.step1")))
        .p(px(num("space.step2")))
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.block")))
        .shadow([BoxShadow {
            offset_y: px(num("space.step2")),
            blur: px(num("space.step6")),
            color: rgba(style::look().color("color.shadow_bar")).into(),
            ..Default::default()
        }])
        .on_mouse_enter(|_: &MouseEvent| hover().update(|h| h.on_bar = true))
        .on_mouse_leave(|_: &MouseEvent| {
            hover().update(|h| h.on_bar = false);
            left();
        })
        .children(BUTTONS.iter().map(|&(b, glyph, _name)| {
            let disabled = (b == "indent" && indent_off) || (b == "unindent" && unindent_off);
            let fg = if b == "check" && done { color::accent() } else { color::muted() };
            let id = id.to_owned();
            view()
                .test_id(&format!("hover-{b}"))
                .opacity(if disabled { 0.35 } else { 1.0 })
                .h(px(btn_h))
                .min_w(px(btn_h))
                .px(px(num("space.step3")))
                .items_center()
                .justify_center()
                .rounded(px(num("radius.code")))
                .hover(move |s| s.bg(hover_bg))
                .on_press(move |p: &PressEvent| {
                    if !disabled {
                        press(b, &id, p.page_position.x.0 - p.position.x.0, p.page_position.y.0 - p.position.y.0 + btn_h);
                    }
                })
                .child(ui_text(glyph, (size, line)).text_color(fg))
        }))
}

/// Put the caret at the end of `id`, unless it is being edited (the commands that act on the caret).
fn edit_here(id: &str) {
    store::edit(|s| {
        if s.editing.as_ref().is_some_and(|c| c.id == id) {
            return;
        }
        let at = cmd::utf16_len(item::content(s.tree().get(id)));
        s.block = None;
        s.editing = Some(store::Caret::text(id, at));
        s.caret_rev += 1;
    });
}

/// A press on bar button `b` of row `id`; (`x`, `y`): the button's bottom left in the window.
fn press(b: &str, id: &str, x: f32, y: f32) {
    let ids = ids_for(id);
    match b {
        "unindent" | "indent" | "duplicate" => {
            cmd::exec_on(&ids, b);
        }
        "export" => crate::export::open_items(ids),
        "zoom-in" => cmd::zoom_into(id),
        "delete" => {
            cmd::exec_on(&ids, "delete-left");
        }
        "check" => {
            cmd::exec_on(&ids, "toggle-checked");
        }
        "checkbox" => {
            cmd::exec_on(&ids, "toggle-checkbox");
        }
        "note" => {
            cmd::exec_on(&[id.to_owned()], "goto-note-of");
        }
        "date" => {
            edit_here(id);
            crate::datepicker::add_date();
        }
        "newline" => {
            edit_here(id);
            cmd::exec("newline");
        }
        "color" | "heading" => {
            let colors = b == "color";
            let h = num("space.swatch_h") + 2.0 * num("space.step5");
            let (id, ids) = (id.to_owned(), ids.clone());
            menu::show(x, y + num("space.step4"), vec![menu::custom(b, h, move || swatches(&id, &ids, colors).into_element())]);
        }
        _ => {}
    }
}
