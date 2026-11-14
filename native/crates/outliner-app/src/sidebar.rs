//! The sidebar, as the web app's (`apps/web/src/ui/sidebar.ts`, `.sidebar` in styles.css): the Files, Bookmarks
//! and Tags tabs over their pane. On desktops it is a panel left of the document, shown or hidden by the top
//! bar's sidebar button and as wide as its drag handle leaves it; on phones a drawer over the document, behind it
//! a dimmed scrim that closes it. Until sync exists the files are the local documents of the store; the web's
//! menus (new folder, import, copy, delete, ...), search, the inbox and bookmarks are not here yet: their buttons
//! are drawn and do nothing.

use nsh_core::keymap::When;
use nsh_core::prelude::*;
use nsh_core::LineHeight;
use nsh_core::{TouchEvent, TouchPhase};

use crate::icons::icon;
use crate::store;
use crate::style::{self, color, num};

/// The sidebar's tabs (`PaneName`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Files,
    Bookmarks,
    Tags,
}

/// What the sidebar shows, apart from the documents (the store's).
#[derive(Debug, Clone, PartialEq, Default)]
struct Side {
    /// Shown or not, once toggled; until then shown on desktops and hidden on phones, as on the web.
    open: Option<bool>,
    tab: Tab,
    /// The width the drag handle left (desktops), until then `space.sidebar`.
    width: Option<f32>,
    /// The document whose name is being edited in its row.
    renaming: Option<String>,
    /// The tag sections folded: this document's, all documents'.
    folded: (bool, bool),
    /// Tags in name order instead of most used first (the web's `tag_sort` pref).
    alpha: bool,
}

fn side() -> Signal<Side> {
    global::<Side>()
}

/// Whether the sidebar shows (tracked).
pub fn is_open() -> bool {
    side().with(|s| s.open).unwrap_or(!style::phone())
}

/// Show or hide the sidebar (the top bar's button: `toggleSidebar`); `None` toggles.
pub fn toggle(open: Option<bool>) {
    let now = side().with_untracked(|s| s.open).unwrap_or(!style::phone());
    side().update(|s| s.open = Some(open.unwrap_or(!now)));
}

/// Show the sidebar on `tab` (the look checks' fixtures).
pub fn show(tab: Tab) {
    side().update(|s| {
        s.open = Some(true);
        s.tab = tab;
    });
}

/// The desktop panel's width (tracked).
fn width() -> f32 {
    side().with(|s| s.width).unwrap_or_else(|| num("space.sidebar"))
}

/// Open document `id` from the sidebar (`openDoc`); on phones the drawer closes, as the web's does when the
/// document changes.
fn open_doc(id: &str) {
    let changed = store::peek(|s| s.doc != id);
    store::edit(|s| s.switch(id));
    if changed && style::phone() {
        toggle(Some(false));
    }
}

/// A new document, opened, its name being edited (`newFile('doc')`).
fn new_doc() {
    let id = store::edit(|s| {
        // as the web, a new document is not edited yet: its name is
        s.editing = None;
        s.create()
    });
    side().update(|s| s.renaming = Some(id.clone()));
    if style::phone() {
        toggle(Some(false));
    }
}

fn start_rename(id: &str) {
    // the row being edited lets go: its keys would act while the name is typed
    store::edit(|s| s.editing = None);
    side().update(|s| s.renaming = Some(id.to_owned()));
}

/// The desktop panel and its drag handle, while the sidebar shows (`.sidebar`, `.splitter`).
pub fn panel() -> impl IntoElement {
    when(
        || !style::phone() && is_open(),
        || {
            view()
                .flex_row()
                .child(
                    sidebar()
                        .w(move || px(width()))
                        .border_r(px(num("space.border")))
                        .border_color(color::line()),
                )
                .child(splitter())
        },
    )
}

/// The handle between the panel and the document (`.splitter`): half over the panel, half over the document;
/// dragging it sets the panel's width to where the pointer is, within `space.sidebar_min` ..
/// `space.sidebar_drag_max`.
fn splitter() -> impl IntoElement {
    let w = num("space.splitter");
    view()
        .test_id("splitter")
        .w(px(w))
        .ml(px(-w / 2.0))
        .self_stretch()
        .z_index(2)
        .on_touch(move |t: &TouchEvent| {
            if !matches!(t.phase, TouchPhase::Move | TouchPhase::End) {
                return;
            }
            if let Some(p) = t.points.iter().find(|p| p.changed) {
                let x = p.page.x.0.clamp(num("space.sidebar_min"), num("space.sidebar_drag_max"));
                side().update(|s| s.width = Some(x));
            }
        })
}

/// The phone drawer over the screen, while the sidebar shows: the scrim (a press closes it) and the sidebar,
/// `space.drawer` of the screen wide up to `space.sidebar_max`, with its shadow (`.app.side-open`).
pub fn drawer() -> impl IntoElement {
    when(
        || style::phone() && is_open(),
        || {
            view()
                .absolute()
                .top(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .child(
                    view()
                        .test_id("scrim")
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(0.0))
                        .right(px(0.0))
                        .bg(color::scrim())
                        .on_press(|| toggle(Some(false))),
                )
                .child(
                    sidebar()
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(0.0))
                        .w(relative(num("space.drawer")))
                        .max_w(px(num("space.sidebar_max")))
                        .pt_safe_area()
                        .shadow([BoxShadow {
                            blur: px(num("space.drawer_shadow")),
                            color: color::shadow_strong().into(),
                            ..Default::default()
                        }]),
                )
        },
    )
}

/// The sidebar's box: the tabs over the pane.
fn sidebar() -> View {
    view()
        .test_id("sidebar")
        .flex_col()
        .bg(color::side_bg())
        .child(tabs())
        .child(
            scroll()
                .test_id("pane")
                .flex_1()
                .child(
                    view()
                        .pt(px(num("space.pane_top")))
                        .pb(px(num("space.pane_bottom")))
                        .child(dynamic(|| match side().with(|s| s.tab) {
                            Tab::Files => files().into_element(),
                            Tab::Bookmarks => bookmarks().into_element(),
                            Tab::Tags => tags().into_element(),
                        })),
                ),
        )
}

/// The tabs (`.tabs`): as wide as each other, the active one in the text color over an accent underline.
fn tabs() -> impl IntoElement {
    let tab = |which: Tab, label: &'static str| {
        let on = move || side().with(|s| s.tab) == which;
        let (size, line) = style::ui_sized("type.ui_small");
        view()
            .test_id("tab")
            .flex_1()
            .items_center()
            .py(px(num("space.side_tab_y")))
            .border_b(px(num("space.side_tab_bar")))
            .border_color(move || if on() { color::accent().into() } else { AdaptiveColor::TRANSPARENT })
            .on_press(move || side().update(|s| s.tab = which))
            .child(
                ui_text(label, (size, line))
                    .text_color(move || if on() { color::fg() } else { color::muted() }),
            )
    };
    view()
        .test_id("tabs")
        .flex_row()
        .border_b(px(num("space.border")))
        .border_color(color::line())
        .child(tab(Tab::Files, "Files"))
        .child(tab(Tab::Bookmarks, "Bookmarks"))
        .child(tab(Tab::Tags, "Tags"))
}

/// A section head of the pane (`.pane-head`): small capitals in the muted color, buttons at its right.
fn head(title: String, buttons: Vec<AnyElement>) -> View {
    let (size, line) = style::ui_sized("type.ui_tiny");
    view()
        .test_id("pane-head")
        .flex_row()
        .items_center()
        .pt(px(num("space.pane_head_y")))
        .pb(px(num("space.pane_head_y")))
        .pl(px(num("space.pane_head_left")))
        .pr(px(num("space.pane_head_right")))
        .child(
            ui_text(title, (size, line))
                .text_color(color::muted())
                .uppercase()
                .letter_spacing(px(size * num("type.head_tracking"))),
        )
        .child(view().flex_1())
        .children(buttons)
}

/// An icon button of the pane (`.icon-btn`): muted, the text color and a rounded background under the pointer.
fn icon_button(name: &'static str, run: impl Fn() + 'static) -> AnyElement {
    let hover = color::hover();
    let fg = color::fg();
    view()
        .test_id(name)
        .h(px(num("space.tool")))
        .min_w(px(num("space.tool_min")))
        .px(px(num("space.tool_pad")))
        .rounded(px(num("radius.tool")))
        .items_center()
        .justify_center()
        .text_color(color::muted())
        .hover(move |s| s.bg(hover).text_color(fg))
        .on_press(run)
        .child(icon(name, num("space.tool_icon")))
        .into_element()
}

fn none() {}

/// A text in the app's font, `size` on lines `line` high, at least a line tall (as a web text's box is).
fn ui_text(content: impl IntoReactive<SharedString>, (size, line): (f32, f32)) -> Text {
    text(content)
        .font_family(style::font())
        .text_size(px(size))
        .line_height(LineHeight::Points(px(line)))
        .min_h(px(line))
}

/// The Files pane: "My files" and the documents (`renderFiles`).
fn files() -> impl IntoElement {
    let ids = computed(|| store::read(|s| s.files.clone()));
    view()
        .child(head(
            "My files".into(),
            vec![
                icon_button("search", none),
                icon_button("inbox", none),
                icon_button("fold", none),
                // the web's "New…" menu offers a document, a folder and an import: here the document only
                icon_button("plus", new_doc),
            ],
        ))
        .child(dynamic(move || {
            view().children(ids.get().into_iter().map(|id| file_row(id, 0))).into_element()
        }))
}

/// A document's row (`.file-row`): its icon and name, on the selection color while it is open. A press opens
/// it; a long press, or the "⋯" shown under the pointer, edits its name (the web: a double click, or its menu).
fn file_row(id: String, depth: u16) -> impl IntoElement {
    let active = {
        let id = id.clone();
        computed(move || store::read(|s| s.doc == id))
    };
    let renaming = {
        let id = id.clone();
        computed(move || side().with(|s| s.renaming.as_deref() == Some(id.as_str())))
    };
    let hover = color::hover();
    let (open_id, long_id, more_id) = (id.clone(), id.clone(), id.clone());
    let (size, line) = style::ui();
    view()
        .test_id("file")
        .group()
        .flex_row()
        .items_center()
        .gap(px(num("space.file_gap")))
        .h(px(num("space.file_row")))
        .pl(px(num("space.file_pad") + num("space.file_indent") * f32::from(depth)))
        .pr(px(num("space.file_pad")))
        .bg(move || if active.get() { color::sel().into() } else { AdaptiveColor::TRANSPARENT })
        .hover(move |s| s.bg(hover))
        .on_press(move || open_doc(&open_id))
        .on_long_press(move || start_rename(&long_id))
        // the folder toggle's column (documents leave it empty)
        .child(view().w(px(num("space.file_box"))))
        .child(
            view()
                .w(px(num("space.file_box")))
                .text_color(color::muted())
                .child(icon("doc", num("space.file_icon"))),
        )
        .child(dynamic(move || {
            if renaming.get() {
                return rename_input(id.clone()).into_element();
            }
            let name = {
                let id = id.clone();
                move || store::read(|s| s.title(&id))
            };
            ui_text(name, (size, line))
                .flex_1()
                .truncate()
                .text_color(color::fg())
                .into_element()
        }))
        .child(
            view()
                .test_id("more")
                .hidden()
                .group_hover(|s| s.flex())
                .on_press(move || start_rename(&more_id))
                .child(
                    ui_text("⋯", (size, line))
                        .text_color(color::muted()),
                ),
        )
}

/// The field a document's name is edited in (`renameInput`): all of it selected; Return or leaving it keeps a
/// name that is not blank, Escape keeps the old one.
fn rename_input(id: String) -> impl IntoElement {
    let old = store::peek(|s| s.title(&id));
    let value = signal(old.clone());
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let commit = {
        let id = id.clone();
        let done = done.clone();
        move |save: bool| {
            if done.replace(true) {
                return;
            }
            let name = value.get_untracked().trim().to_owned();
            if save && !name.is_empty() && name != old {
                store::edit(|s| s.rename(&id, &name));
            }
            side().update(|s| s.renaming = None);
        }
    };
    let (on_submit, on_blur, on_escape) = (commit.clone(), commit.clone(), commit);
    let opts = KeyOptions { when: When::Always, ..Default::default() };
    on_key_with("escape", opts, move |_| on_escape(false));
    let len = crate::commands::utf16_len(&value.get_untracked());
    let (size, line) = style::ui();
    nsh_base::input()
        .test_id("rename")
        .value(move || value.get())
        .auto_focus(true)
        .selected_range((0, len))
        .flex_1()
        .font_family(style::font())
        .text_size(px(size))
        .line_height(LineHeight::Points(px(line)))
        .text_color(color::fg())
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::accent())
        .rounded(px(num("radius.row")))
        .px(px(num("space.rename_x")))
        .py(px(num("space.rename_y")))
        .on_change(move |t: &str| value.set(t.to_owned()))
        .on_submit(move |_: &str| on_submit(true))
        .on_blur(move || on_blur(true))
}

/// The Bookmarks pane: none can be made yet, so its hint (`renderBookmarks`).
fn bookmarks() -> impl IntoElement {
    let (size, line) = style::ui_sized("type.ui_small");
    view().child(head("Bookmarks".into(), vec![icon_button("star", none)])).child(
        view().px(px(num("space.pane_empty_x"))).py(px(num("space.pane_empty_y"))).child(
            ui_text("Bookmark a document, a zoomed item or a search with the ☆ button.", (size, line))
                .text_color(color::muted()),
        ),
    )
}

/// The Tags pane (`renderTags`): the open document's tags and all documents' tags, each a section that folds,
/// most used first or by name.
fn tags() -> impl IntoElement {
    let lists = computed(|| {
        store::read(|s| {
            let doc = s.tags_in(std::slice::from_ref(&s.doc));
            let all = s.tags_in(&s.files);
            (doc, all)
        })
    });
    dynamic(move || {
        let (doc, all) = lists.get();
        let (folded, alpha) = side().with(|s| (s.folded, s.alpha));
        view()
            .child(tag_section("Document tags", doc, folded.0, alpha, true))
            .child(tag_section("All tags", all, folded.1, alpha, false))
            .into_element()
    })
}

fn tag_section(title: &str, mut list: Vec<(String, usize)>, folded: bool, alpha: bool, mine: bool) -> View {
    if alpha {
        list.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then_with(|| a.0.cmp(&b.0)));
    }
    let mark = if folded { '▸' } else { '▾' };
    let mut buttons = Vec::new();
    if mine {
        // the sort order (`.btn.link`), for both sections
        let (size, line) = style::ui();
        buttons.push(
            view()
                .test_id("tag-sort")
                .py(px(num("space.link_y")))
                .on_press(|| side().update(|s| s.alpha = !s.alpha))
                .child(
                    ui_text(if alpha { "A–Z" } else { "By count" }, (size, line))
                        // a button: not in the head's capitals (the web's buttons do not inherit them)
                        .text_color(color::accent()),
                )
                .into_element(),
        );
    }
    let section = head(format!("{mark} {title} ({})", list.len()), buttons)
        .on_press(move || side().update(|s| if mine { s.folded.0 = !s.folded.0 } else { s.folded.1 = !s.folded.1 }));
    let (size, line) = style::ui();
    let hover = color::hover();
    let rows = (!folded).then(|| {
        list.into_iter().map(move |(tag, n)| {
            view()
                .test_id("tag")
                .flex_row()
                .gap(px(num("space.file_gap")))
                .px(px(num("space.tag_row_x")))
                .py(px(num("space.tag_row_y")))
                .hover(move |s| s.bg(hover))
                .child(
                    ui_text(tag, (size, line))
                        .text_color(color::fg()),
                )
                .child(
                    ui_text(format!("({n})"), (size, line))
                        .text_color(color::muted()),
                )
        })
    });
    view().child(section).children(rows.into_iter().flatten())
}
