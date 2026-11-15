//! The mind map (`apps/web/src/ui/mindmap.ts`, `.mindmap`): a read-only view of the zoomed item over the whole
//! screen, shown while the document's layout is the mind map (`docview`). Each item is a box (its text with the
//! inline formatting, its note, its heading, label and checked looks) with its children in a column to its right,
//! joined by lines; a box's button folds or unfolds it here only (the document's folds are where it starts), a
//! double press zooms in on it, the path at the top zooms out. "Back to document" (or Escape) goes back to the list.

use std::collections::HashMap;

use nsh_core::prelude::*;
use nsh_core::LineHeight;
use outliner_core::{item, NodeId, Tree, ROOT};

use crate::dialog;
use crate::docview;
use crate::icons::icon;
use crate::inline::{self, Block};
use crate::sidebar::ui_text;
use crate::store;
use crate::style::{self, color, num};

/// The map's own state: its scale and the boxes folded or unfolded here, for one document.
#[derive(Clone, PartialEq)]
struct Map {
    doc: String,
    scale: f32,
    open: HashMap<NodeId, bool>,
}

impl Default for Map {
    fn default() -> Self {
        Map { doc: String::new(), scale: 1.0, open: HashMap::new() }
    }
}

fn map() -> Signal<Map> {
    global::<Map>()
}

/// Whether the mind map shows (the open document's layout).
pub fn showing() -> bool {
    docview::layout() == docview::MINDMAP
}

/// Zooms the map by `d` (`zoomBy`): between 0.3 and 2, in tenths.
fn zoom_by(d: f32) {
    map().update(|m| m.scale = (((m.scale + d) * 10.0).round() / 10.0).clamp(0.3, 2.0));
}

/// Opens item `id` in the map (`onZoom`: the route's zoom, so Back comes here again).
fn zoom_to(id: &str) {
    let id = id.to_owned();
    store::edit(|s| {
        if s.tree().is_attached(&id) {
            s.view.zoom = id;
            s.editing = None;
            s.block = None;
            s.caret_rev += 1;
            s.reflow();
        }
    });
}

fn back() {
    docview::set_layout("list");
}

/// The map over the whole screen while it shows.
pub fn layer() -> impl IntoElement {
    when(showing, || {
        on_key("escape", |_| back());
        view()
            .test_id("mindmap")
            .absolute()
            .top(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .pt_safe_area()
            .bg(color::bg())
            .child(dynamic(bar))
            .child(scroll().flex_1().horizontal().child(scroll().child(dynamic(canvas))))
    })
}

/// The bar (`.mm-bar`): back to the document, the path to the zoomed item, the zoom buttons.
fn bar() -> AnyElement {
    let crumbs = store::read(|s| {
        let t = s.tree();
        let crumbs: Vec<(NodeId, String)> = t
            .ancestors(&s.view.zoom)
            .into_iter()
            .map(|a| {
                let name = if a == ROOT { s.title(&s.doc) } else { item::content(t.get(&a)).chars().take(30).collect() };
                (a, name)
            })
            .collect::<Vec<(NodeId, String)>>();
        crumbs
    });
    let small = style::ui_sized("type.ui_small");
    let hover = color::hover();
    let tool = |label: &'static str, run: fn(), child: AnyElement| {
        nsh_base::button()
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
            .child(child)
    };
    let path = view().flex_row().gap(px(num("space.step5"))).flex_shrink(1.0).overflow_hidden().children(
        crumbs.into_iter().enumerate().map(move |(i, (id, name))| {
            let shown = if i == 0 { name } else { format!("› {name}") };
            nsh_base::button()
                .accessibility_label(Some(shown.clone().into()))
                .on_press(move || zoom_to(&id))
                .child(ui_text(shown, small).text_color(color::muted()).truncate())
                .into_element()
        }),
    );
    view()
        .test_id("mm-bar")
        .flex_row()
        .items_center()
        .gap(px(num("space.step7")))
        .py(px(num("space.step6")))
        .px(px(num("space.step8")))
        .border_b(px(num("space.border")))
        .border_color(color::line())
        .child(dialog::btn("← Back to document", false, back))
        .child(path)
        .child(view().flex_1())
        .child(tool("Zoom out", || zoom_by(-0.1), ui_text("−", style::ui()).into_element()))
        .child(tool("Zoom in", || zoom_by(0.1), icon("plus", num("space.topbar_icon")).into_element()))
        .into_element()
}

/// What the boxes are drawn from.
struct Ctx<'a> {
    t: &'a Tree,
    zoom: &'a str,
    title: String,
    show_checked: bool,
    open: &'a HashMap<NodeId, bool>,
    scale: f32,
}

/// The map from the zoomed item (`.mm-canvas`), at its scale.
fn canvas() -> AnyElement {
    let doc = store::read(|s| s.doc.clone());
    // another document's folds start over (`show` with another tree)
    if map().with_untracked(|m| m.doc != doc) {
        map().update(|m| {
            m.doc = doc.clone();
            m.open.clear();
        });
    }
    let m = map().get();
    store::read(|s| {
        let cx = Ctx {
            t: s.tree(),
            zoom: &s.view.zoom,
            title: s.title(&s.doc),
            show_checked: s.show_checked,
            open: &m.open,
            scale: m.scale,
        };
        view().p(px(num("space.step12") * m.scale)).items_start().child(node(&cx, s.view.zoom.clone())).into_element()
    })
}

/// A box and, unfolded, its children to its right (`.mm-node`).
fn node(cx: &Ctx<'_>, id: NodeId) -> AnyElement {
    let k = cx.scale;
    let n = cx.t.get(&id);
    let kids: Vec<NodeId> = n
        .map(|n| n.children.iter().filter(|c| cx.show_checked || !item::is_checked(cx.t.get(c))).cloned().collect())
        .unwrap_or_default();
    let checked = item::is_checked(n);
    let level = item::heading(n);
    let label = item::color(n);
    let size = k * match level {
        1 => num("type.mm_h1"),
        2 => num("type.mm_h2"),
        3 => num("type.mm_h3"),
        _ => num("type.mm"),
    };
    let line = size * num("type.ui_line");
    let fg = if checked { color::check() } else { color::fg() };
    let content = item::content(n).to_owned();
    let shown = inline::Look::new(size, store::now()).colored(fg, checked);
    let paragraph = |tokens: &[outliner_core::inline::Token]| {
        let mut t = text("")
            .font_family(style::font())
            .text_size(px(size))
            .line_height(LineHeight::Points(px(line)))
            .text_color(fg)
            .children(inline::spans(tokens, &shown));
        if level > 0 {
            t = t.font_weight(style::weight("type.heading_weight"));
        }
        if checked {
            t = t.line_through();
        }
        t.into_element()
    };
    let text_part: AnyElement = if id == ROOT && content.is_empty() {
        ui_text(cx.title.clone(), (size, line)).text_color(fg).into_element()
    } else if content.is_empty() {
        // `&nbsp;`: an empty box keeps a line's height
        view().h(px(line)).into_element()
    } else {
        view()
            .children(inline::blocks(&content).into_iter().map(|b| match b {
                Block::Inline(tokens) => paragraph(&tokens),
                Block::Code { text, .. } => inline::code_block(&text, &shown).into_element(),
            }))
            .into_element()
    };
    let note = item::note(n);
    let note_part = (!note.is_empty()).then(|| {
        let (tiny, tiny_line) = style::ui_sized("type.ui_tiny");
        ui_text(note.to_owned(), (tiny * k, tiny_line * k)).text_color(color::note())
    });
    let border = if label > 0 {
        let mut c = color::fg();
        c.a *= 0.25;
        c
    } else {
        color::chip_border()
    };
    let opened = cx.open.get(&id).copied().unwrap_or_else(|| id == cx.zoom || !n.is_some_and(|n| n.collapsed));
    let mut boxed = view()
        .test_id("mm-box")
        .max_w(px(num("space.mm_box_max") * k))
        .my(px(num("space.mm_box_gap") * k))
        .py(px(num("space.step5") * k))
        .px(px(num("space.step7") * k))
        .border(px(num("space.border")))
        .border_color(border)
        .rounded(px(num("radius.menu") * k))
        .bg(color::label(label).unwrap_or_else(color::bg))
        .child(text_part)
        .children(note_part);
    if id != cx.zoom {
        let at = id.clone();
        boxed = boxed.on_press(move |p: &PressEvent| {
            if p.clicks >= 2 {
                zoom_to(&at);
            }
        });
    }
    if !kids.is_empty() {
        let at = id.clone();
        let mark = if opened { "−".to_owned() } else { kids.len().to_string() };
        let (micro, micro_line) = style::ui_sized("type.ui_micro");
        let d = num("space.step10") * k;
        let toggle = nsh_base::button()
            .accessibility_label(Some(if opened { "Collapse" } else { "Expand" }.into()))
            .min_w(px(d))
            .h(px(d))
            .px(px(num("space.step4") * k))
            .rounded(px(num("radius.modal") * k))
            .border(px(num("space.border")))
            .border_color(color::chip_border())
            .bg(color::bg())
            .items_center()
            .justify_center()
            .on_press(move || map().update(|m| {
                m.open.insert(at.clone(), !opened);
            }))
            .child(ui_text(mark, (micro * k, micro_line * k)).text_color(color::muted()));
        // on the box's right edge, in its middle
        boxed = boxed.child(
            view()
                .absolute()
                .top(px(0.0))
                .bottom(px(0.0))
                .right(px(-num("space.mm_toggle_x") * k))
                .justify_center()
                .child(toggle),
        );
    }
    let mut el = view().flex_row().items_center().child(boxed);
    if opened && !kids.is_empty() {
        let count = kids.len();
        let gap = num("space.step12") * k;
        let stub = num("space.step9") * k;
        let rule = || view().border_t(px(num("space.border"))).border_color(color::faint());
        // the line from the box to its children's column (`.mm-kids::before`)
        el = el.child(view().w(px(gap)).child(rule()));
        let column = view().flex_col().children(kids.into_iter().enumerate().map(|(i, c)| {
            // each child's line in from the column's line (`.mm-kids > .mm-node::before` / `::after`)
            let side = |on: bool| {
                let v = view().flex_1().w(px(stub));
                if on && count > 1 {
                    v.border_l(px(num("space.border"))).border_color(color::faint())
                } else {
                    v
                }
            };
            let joint = view().self_stretch().flex_col().child(side(i > 0)).child(rule().w(px(stub))).child(side(i + 1 < count));
            view().flex_row().items_center().child(joint).child(node(cx, c)).into_element()
        }));
        el = el.child(column);
    }
    el.into_element()
}
