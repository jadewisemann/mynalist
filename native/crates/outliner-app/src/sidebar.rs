//! The sidebar, as the web app's (`apps/web/src/ui/sidebar.ts`, `.sidebar` in styles.css): the Files, Bookmarks
//! and Tags tabs over their pane. On desktops it is a panel left of the document, shown or hidden by the top
//! bar's sidebar button and as wide as its drag handle leaves it; on phones a drawer over the document, behind it
//! a dimmed scrim that closes it. The files are the workspace's file tree ([`crate::workspace`]): folders that
//! fold, documents, the web's file menu (right click, the "⋯" under the pointer, a long press), renaming in the
//! row (a double click), dragging rows to reorder them or into folders (desktops; phones: "Move to…" in the
//! menu). The bookmarks open a document in the zoom they were made in. Search, the inbox and tag searches are
//! not here yet: their buttons are drawn and do nothing. The sidebar's state (shown, tab, width, tag order) is
//! kept on the device.

use nsh_core::keymap::When;
use nsh_core::prelude::*;
use nsh_core::storage::Storage;
use nsh_core::LineHeight;
use nsh_core::{TouchEvent, TouchPhase};
use outliner_core::{item, ROOT};

use crate::icons::icon;
use crate::{dialog, toast};
use crate::menu::{self, Item};
use crate::style::{self, color, num};
use crate::workspace::{self, Bookmark, Entry, Kind};
use crate::store;

/// The sidebar's tabs (`PaneName`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Files,
    Bookmarks,
    Tags,
}

impl Tab {
    fn name(self) -> &'static str {
        match self {
            Tab::Files => "files",
            Tab::Bookmarks => "bookmarks",
            Tab::Tags => "tags",
        }
    }
}

/// The keys the sidebar's state is kept under on the device: the web's `tag_sort` pref; the web keeps the rest
/// for the page's life only, the native app across starts.
const TAG_SORT: &str = "tag_sort";
const OPEN: &str = "sidebar_open";
const TAB: &str = "sidebar_tab";
const WIDTH: &str = "sidebar_width";

/// What the sidebar shows, apart from the files (the workspace's).
#[derive(Debug, Clone, PartialEq)]
struct Side {
    /// Shown or not, once toggled; until then shown on desktops and hidden on phones, as on the web.
    open: Option<bool>,
    tab: Tab,
    /// The width the drag handle left (desktops), until then `space.sidebar`.
    width: Option<f32>,
    /// The entry whose name is being edited in its row.
    renaming: Option<String>,
    /// The tag sections folded: this document's, all documents'.
    folded: (bool, bool),
    /// Tags in name order instead of most used first (the web's `tag_sort` pref).
    alpha: bool,
}

impl Default for Side {
    /// As kept on the device.
    fn default() -> Self {
        let tab = match Storage::get_item(TAB).as_deref() {
            Some("bookmarks") => Tab::Bookmarks,
            Some("tags") => Tab::Tags,
            _ => Tab::Files,
        };
        Side {
            open: Storage::get_item(OPEN).and_then(|v| v.parse().ok()),
            tab,
            width: Storage::get_item(WIDTH)
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|w| w.is_finite())
                .map(|w| w.clamp(num("space.sidebar_min"), num("space.sidebar_drag_max"))),
            renaming: None,
            folded: (false, false),
            alpha: Storage::get_item(TAG_SORT).as_deref() == Some("alpha"),
        }
    }
}

fn side() -> Signal<Side> {
    global::<Side>()
}

fn keep(key: &str, value: String) {
    if let Err(e) = Storage::set_item(key, value) {
        eprintln!("outliner: {key} could not be kept: {e:?}");
    }
}

/// Whether the sidebar shows (tracked).
pub fn is_open() -> bool {
    side().with(|s| s.open).unwrap_or(!style::phone())
}

/// Show or hide the sidebar (the top bar's button: `toggleSidebar`); `None` toggles.
pub fn toggle(open: Option<bool>) {
    let now = side().with_untracked(|s| s.open).unwrap_or(!style::phone());
    let open = open.unwrap_or(!now);
    side().update(|s| s.open = Some(open));
    keep(OPEN, open.to_string());
}

/// Show the sidebar on `tab` (the look checks' fixtures; not kept).
pub fn show(tab: Tab) {
    side().update(|s| {
        s.open = Some(true);
        s.tab = tab;
    });
}

fn set_tab(tab: Tab) {
    side().update(|s| s.tab = tab);
    keep(TAB, tab.name().to_owned());
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
    if changed {
        crate::settings::document_opened();
    }
    if changed && style::phone() {
        toggle(Some(false));
    }
}

/// A new document or folder in `parent` (the top level for `None`), its name being edited; a document opens
/// (`newFile`).
fn new_file(kind: Kind, parent: Option<String>) {
    // the row being edited lets go: its keys would act while the name is typed
    store::edit(|s| {
        s.editing = None;
        s.block = None;
    });
    let id = workspace::create(kind, parent.as_deref(), "Untitled");
    side().update(|s| s.renaming = Some(id.clone()));
    if kind == Kind::Doc {
        let changed = store::peek(|s| s.doc != id);
        store::edit(|s| s.switch(&id));
        if changed && style::phone() {
            toggle(Some(false));
        }
    }
}

fn start_rename(id: &str) {
    store::edit(|s| {
        s.editing = None;
        s.block = None;
    });
    side().update(|s| s.renaming = Some(id.to_owned()));
}

/// The "New…" menu (`newMenu`): a document, a folder, documents imported from OPML or text files.
fn new_menu(parent: Option<String>) -> Vec<Item> {
    let (p2, p3) = (parent.clone(), parent.clone());
    vec![
        menu::item("New document", move || new_file(Kind::Doc, parent.clone())),
        menu::item("New folder", move || new_file(Kind::Folder, p2.clone())),
        // the picker's task under no view's owner: the menu goes away as the item is chosen
        menu::item("Import OPML…", move || {
            let p = p3.clone();
            Owner::detached().run(move || crate::settings::import_files(p));
        }),
    ]
}

/// The menu of a file entry (`fileMenu`), at (`x`, `y`).
fn file_menu(e: &Entry, x: f32, y: f32) {
    let mut items = Vec::new();
    let id = e.id.clone();
    if e.kind == Kind::Folder {
        items.extend(new_menu(Some(id.clone())));
        items.push(menu::sep());
    } else {
        let (copy, mark) = (id.clone(), id.clone());
        items.push(menu::item("Make a copy", move || {
            if workspace::copy_doc(&copy).is_some() {
                toast::show("Copy created");
            }
        }));
        items.push(menu::item("Add to bookmarks", move || {
            let title = store::peek(|s| s.title(&mark));
            workspace::add_bookmark(&title, &mark, None);
        }));
        let (arch, archived) = (id.clone(), e.archived);
        items.push(menu::item(if archived { "Unarchive" } else { "Archive" }, move || {
            workspace::set_archived(&arch, !archived)
        }));
        items.push(menu::sep());
    }
    if style::phone() {
        // phones cannot drag a row: the folders to move it to instead
        let moving = id.clone();
        items.push(menu::item("Move to…", move || move_menu(&moving, x, y)));
    }
    let renamed = id.clone();
    items.push(menu::item("Rename", move || start_rename(&renamed)));
    let (gone, kind) = (id, e.kind);
    items.push(menu::item("Delete", move || {
        let what = if kind == Kind::Folder { "folder" } else { "document" };
        let title = name_of(&gone, kind);
        let gone = gone.clone();
        confirm(
            "Delete",
            format!("Are you sure you want to delete the {what} \"{title}\"?"),
            "Delete",
            move || workspace::remove(&gone),
        );
    }));
    menu::show(x, y, items);
}

/// Show the menu of entry `id` at (`x`, `y`) (a look check's fixture).
pub fn file_menu_at(id: &str, x: f32, y: f32) {
    if let Some(e) = workspace::get(id) {
        file_menu(&e, x, y);
    }
}

/// Where an entry can move to (phones): the top level and the folders it is not in itself.
fn move_menu(id: &str, x: f32, y: f32) {
    let mut items = Vec::new();
    let top = id.to_owned();
    items.push(menu::item("Top level", move || workspace::move_to(&top, None, usize::MAX)));
    for (folder, title, depth) in workspace::folders() {
        if workspace::is_within(&folder, id) {
            continue;
        }
        let moving = id.to_owned();
        let indent = "  ".repeat(usize::from(depth));
        items.push(menu::item(format!("{indent}{title}"), move || {
            workspace::move_to(&moving, Some(&folder), usize::MAX)
        }));
    }
    menu::show(x, y, items);
}

/// An entry's name: a document's root text (its name follows it), a folder's title.
fn name_of(id: &str, kind: Kind) -> String {
    match kind {
        Kind::Doc => store::peek(|s| s.title(id)),
        Kind::Folder => workspace::get(id).map(|e| e.title).unwrap_or_default(),
    }
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
                if t.phase == TouchPhase::End {
                    keep(WIDTH, x.to_string());
                }
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
            .on_press(move || set_tab(which))
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
fn icon_button(name: &'static str, run: impl Fn(&PressEvent) + 'static) -> AnyElement {
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

fn none(_: &PressEvent) {}

/// A text in the app's font, `size` on lines `line` high, at least a line tall (as a web text's box is).
pub(crate) fn ui_text(content: impl IntoReactive<SharedString>, (size, line): (f32, f32)) -> Text {
    text(content)
        .font_family(style::font())
        .text_size(px(size))
        .line_height(LineHeight::Points(px(line)))
        .min_h(px(line))
}

/// A row of the file tree as it shows: the entry (without what is in it), its depth, its folder and its place
/// among the entries there.
#[derive(Debug, Clone, PartialEq)]
struct Line {
    entry: Entry,
    depth: u16,
    parent: Option<String>,
    index: usize,
}

/// The rows of the file tree: what is in a folded folder does not show (`renderFiles`).
fn lines(es: &[Entry]) -> Vec<Line> {
    fn walk(es: &[Entry], depth: u16, parent: Option<&str>, out: &mut Vec<Line>) {
        for (index, e) in es.iter().enumerate() {
            let entry = Entry { children: Vec::new(), ..e.clone() };
            out.push(Line { entry, depth, parent: parent.map(str::to_owned), index });
            if e.kind == Kind::Folder && !e.collapsed {
                walk(&e.children, depth + 1, Some(&e.id), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(es, 0, None, &mut out);
    out
}

/// Where a dragged row would go, by the row under it: before it, after it, or into it (a folder's middle half).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    Before,
    After,
    Into,
}

/// A row being dragged (desktops): from the files or the bookmarks, the row's index, where in the row the
/// pointer went down and where on the screen, the row under it now.
#[derive(Debug, Clone, PartialEq)]
struct Drag {
    files: bool,
    from: usize,
    y0: f32,
    page0: f32,
    target: Option<(usize, Zone)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Dragging(Option<Drag>);

fn dragging() -> Signal<Dragging> {
    global::<Dragging>()
}

/// The zone a row is dropped in (`zone`): `at` is how far down the row the pointer is, from 0 to 1.
fn zone(folder: bool, at: f32) -> Zone {
    if folder && at > 0.25 && at < 0.75 {
        Zone::Into
    } else if at < 0.5 {
        Zone::Before
    } else {
        Zone::After
    }
}

/// The drop indicator a row shows while a row is dragged over it (`.drop-before`, `.drop-after`; `.drop-into`
/// is the row's background).
fn drop_zone(files: bool, index: ReadSignal<usize>) -> Memo<Option<Zone>> {
    computed(move || {
        let at = index.get();
        dragging().with(|d| d.0.as_ref().filter(|d| d.files == files).and_then(|d| d.target).filter(|t| t.0 == at).map(|t| t.1))
    })
}

/// The accent line at a row's top or bottom while a dragged row would go before or after it.
fn drop_line(zone: Memo<Option<Zone>>) -> impl IntoElement {
    let bar = num("space.step2");
    when(
        move || matches!(zone.get(), Some(Zone::Before | Zone::After)),
        move || {
            view()
                .test_id("drop-line")
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .h(px(bar))
                .top(move || if zone.get() == Some(Zone::After) { px(num("space.file_row") - bar) } else { px(0.0) })
                .bg(color::accent())
                .pointer_events(PointerEvents::None)
        },
    )
}

/// The touches of a row that drags it (desktops: the mouse), the web's drag and drop: the row under the pointer
/// and the zone in it as it moves, the move when it lets go. A row is `space.file_row` high; the rows follow
/// each other with nothing between.
fn drag_rows(files: bool, index: ReadSignal<usize>, count: impl Fn() -> usize + 'static, folder_at: impl Fn(usize) -> bool + 'static, drop: impl Fn(usize, usize, Zone) + 'static) -> impl Fn(&TouchEvent) + 'static {
    move |t: &TouchEvent| {
        let Some(p) = t.points.iter().find(|p| p.changed) else { return };
        match t.phase {
            TouchPhase::Start => {
                if side().with_untracked(|s| s.renaming.is_some()) {
                    return;
                }
                dragging().set(Dragging(Some(Drag {
                    files,
                    from: index.get_untracked(),
                    y0: p.location.y.0,
                    page0: p.page.y.0,
                    target: None,
                })));
            }
            TouchPhase::Move => {
                let Some(d) = dragging().with_untracked(|d| d.0.clone()) else { return };
                let h = num("space.file_row");
                let moved = p.page.y.0 - d.page0;
                // a press that wanders a little is not a drag
                let target = if moved.abs() < num("space.step4") {
                    None
                } else {
                    let off = d.y0 + moved + h * d.from as f32;
                    let n = count();
                    let at = (off / h).floor();
                    if n == 0 || at < 0.0 {
                        Some((0, Zone::Before))
                    } else if at as usize >= n {
                        Some((n - 1, Zone::After))
                    } else {
                        let at = at as usize;
                        Some((at, zone(files && folder_at(at), off / h - at as f32)))
                    }
                }
                .filter(|(at, _)| *at != d.from);
                if target != d.target {
                    dragging().set(Dragging(Some(Drag { target, ..d })));
                }
            }
            TouchPhase::End => {
                let d = dragging().with_untracked(|d| d.0.clone());
                dragging().set(Dragging(None));
                if let Some((at, z)) = d.as_ref().and_then(|d| d.target) {
                    drop(d.map_or(0, |d| d.from), at, z);
                }
            }
            TouchPhase::Cancel => dragging().set(Dragging(None)),
        }
    }
}

/// Drop file row `from` on row `to` (`drop`): into a folder, last; else before or after it, in its folder.
fn drop_file(from: usize, to: usize, z: Zone) {
    let ls = lines(&workspace::files_now());
    let (Some(a), Some(b)) = (ls.get(from), ls.get(to)) else { return };
    let id = a.entry.id.clone();
    if z == Zone::Into {
        workspace::move_to(&id, Some(&b.entry.id), usize::MAX);
        return;
    }
    let mut index = if z == Zone::Before { b.index } else { b.index + 1 };
    // the web's index counts the dragged row among its siblings: one less below it in the same folder
    if a.parent == b.parent && a.index < index {
        index -= 1;
    }
    workspace::move_to(&id, b.parent.as_deref(), index);
}

/// The Files pane: "My files" and the file tree (`renderFiles`). A right click on the pane offers the "New…"
/// menu.
fn files() -> impl IntoElement {
    let rows = computed(|| lines(&workspace::files()));
    view()
        .on_context_menu(|c: &ContextMenuEvent| menu::show(c.page_position.x.0, c.page_position.y.0, new_menu(None)))
        .child(head(
            "My files".into(),
            vec![
                icon_button("search", none),
                icon_button("inbox", none),
                icon_button("fold", |_| workspace::collapse_all()),
                icon_button("plus", |p| {
                    menu::show(p.page_position.x.0, p.page_position.y.0 + num("space.step7"), new_menu(None))
                }),
            ],
        ))
        .child(each(move || rows.get(), |l: &Line| l.entry.id.clone(), move |line, index| file_row(line, index, rows)))
}

/// A row of the file tree (`.file-row`): the folder's fold mark, its icon and name, on the selection color
/// while it is the open document. A press opens a document or folds a folder, a double press edits the name; a
/// right click, a long press or the "⋯" under the pointer shows its menu; on desktops it drags.
fn file_row(line: ReadSignal<Line>, index: ReadSignal<usize>, rows: Memo<Vec<Line>>) -> impl IntoElement {
    let id = line.get_untracked().entry.id;
    let folder = line.get_untracked().entry.kind == Kind::Folder;
    let active = {
        let id = id.clone();
        computed(move || store::read(|s| s.doc == id))
    };
    let renaming = {
        let id = id.clone();
        computed(move || side().with(|s| s.renaming.as_deref() == Some(id.as_str())))
    };
    let drop = drop_zone(true, index);
    let hover = color::hover();
    let sel = color::sel();
    let (size, line_h) = style::ui();
    let press = move |p: &PressEvent| {
        let e = line.get_untracked().entry;
        if e.kind == Kind::Folder {
            workspace::set_collapsed(&e.id, !e.collapsed);
        } else {
            open_doc(&e.id);
        }
        if p.clicks >= 2 {
            start_rename(&e.id);
        }
    };
    let at_menu = move |x: f32, y: f32| file_menu(&line.get_untracked().entry, x, y);
    let mut row = view()
        .test_id("file")
        .group()
        .flex_row()
        .items_center()
        .gap(px(num("space.file_gap")))
        .h(px(num("space.file_row")))
        .pl(move || px(num("space.file_pad") + num("space.file_indent") * f32::from(line.get().depth)))
        .pr(px(num("space.file_pad")))
        .opacity(move || if line.get().entry.archived { num("color.archived_opacity") } else { 1.0 })
        .bg(move || {
            if active.get() || drop.get() == Some(Zone::Into) {
                sel.into()
            } else {
                AdaptiveColor::TRANSPARENT
            }
        })
        .hover(move |s| s.bg(hover))
        .on_press(press)
        .on_long_press(move |p: &PressEvent| at_menu(p.page_position.x.0, p.page_position.y.0))
        .on_context_menu(move |c: &ContextMenuEvent| at_menu(c.page_position.x.0, c.page_position.y.0))
        // the folder's fold mark (`.tog`; documents leave its column empty)
        .child(view().w(px(num("space.file_box"))).items_center().pointer_events(PointerEvents::None).children(
            folder.then(|| {
                ui_text(move || if line.get().entry.collapsed { "▸" } else { "▾" }, style::ui_sized("type.ui_mini"))
                    .test_id("tog")
                    .text_color(color::muted())
            }),
        ))
        .child(
            view()
                .w(px(num("space.file_box")))
                .text_color(color::muted())
                .pointer_events(PointerEvents::None)
                .child(icon(if folder { "folder" } else { "doc" }, num("space.file_icon"))),
        )
        .child(dynamic(move || {
            if renaming.get() {
                return rename_input(id.clone()).into_element();
            }
            ui_text(
                move || {
                    let e = line.get().entry;
                    match e.kind {
                        Kind::Doc => store::read(|s| s.title(&e.id)),
                        Kind::Folder => e.title,
                    }
                },
                (size, line_h),
            )
            .test_id("file-name")
            .flex_1()
            .truncate()
            .text_color(color::fg())
            .pointer_events(PointerEvents::None)
            .into_element()
        }))
        .child(
            view()
                .test_id("more")
                .self_stretch()
                .justify_center()
                .hidden()
                .group_hover(|s| s.flex())
                .on_press(move |p: &PressEvent| at_menu(p.page_position.x.0, p.page_position.y.0))
                .child(ui_text("⋯", (size, line_h)).text_color(color::muted())),
        )
        .child(drop_line(drop));
    if !style::phone() {
        let count = move || rows.get_untracked().len();
        let folder_at = move |i: usize| rows.with_untracked(|r| r.get(i).is_some_and(|l| l.entry.kind == Kind::Folder));
        row = row.on_touch(drag_rows(true, index, count, folder_at, drop_file)).claim_touches(TouchClaim::on_move());
    }
    row
}

/// The field an entry's name is edited in (`renameInput`): all of it selected; Return or leaving it keeps a
/// name that is not blank, Escape keeps the old one.
fn rename_input(id: String) -> impl IntoElement {
    let old = workspace::get(&id).map(|e| name_of(&id, e.kind)).unwrap_or_default();
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
            side().update(|s| s.renaming = None);
            if save && !name.is_empty() && name != old {
                workspace::rename(&id, &name);
            }
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

/// Bookmark what the document shows (`bookmarkCurrent`): the document, or the item it is zoomed into, named
/// after it; the Bookmarks tab shows.
pub fn bookmark_current() {
    let (doc, zoom, title) = store::peek(|s| {
        let z = s.view.zoom.clone();
        let title = if z == ROOT {
            s.title(&s.doc)
        } else {
            let plain = outliner_core::inline::inline_to_plain(item::content(s.tree().get(&z)), None);
            if plain.trim().is_empty() { "Untitled".to_owned() } else { plain }
        };
        (s.doc.clone(), (z != ROOT).then_some(z), title)
    });
    workspace::add_bookmark(&title, &doc, zoom.as_deref());
    side().update(|s| s.open = Some(true));
    set_tab(Tab::Bookmarks);
    toast::show("Bookmarked");
}

/// Open a bookmark (`openBookmark`): its document, zoomed as it was made (a search bookmark opens its document
/// until search exists here).
fn open_bookmark(b: &Bookmark) {
    if store::peek(|s| s.tree_of(&b.doc).is_none()) {
        return;
    }
    let changed = store::peek(|s| s.doc != b.doc);
    store::edit(|s| {
        s.switch(&b.doc);
        let zoom = b.zoom.clone().filter(|z| s.tree().is_attached(z)).unwrap_or_else(|| ROOT.to_owned());
        if s.view.zoom != zoom {
            s.view.zoom = zoom;
            s.editing = None;
            s.block = None;
            s.reflow();
        }
    });
    if changed && style::phone() {
        toggle(Some(false));
    }
}

/// A question before an action (`modal(title, text, [Cancel, <button>])`).
fn confirm(title: &'static str, text: String, button: &'static str, run: impl Fn() + 'static) {
    let run = std::rc::Rc::new(run);
    dialog::show(move || {
        let run = run.clone();
        dialog::modal(
            "dialog",
            title,
            dialog::label(text.clone()),
            vec![
                dialog::Button::close("Cancel"),
                dialog::Button::new(button, true, move || {
                    run();
                    true
                }),
            ],
        )
    });
}

/// The bookmark's name in a dialog (`renameBookmark`): Save (or Return) keeps it, a blank one keeps the old.
fn rename_bookmark(b: &Bookmark) {
    let (id, old) = (b.id.clone(), b.title.clone());
    dialog::show(move || {
        let value = signal(old.clone());
        let save = {
            let (id, old) = (id.clone(), old.clone());
            move || {
                let t = value.get_untracked().trim().to_owned();
                workspace::rename_bookmark(&id, if t.is_empty() { &old } else { &t });
                true
            }
        };
        let submit = save.clone();
        let len = crate::commands::utf16_len(&old);
        let input = dialog::input("dialog-input", "")
            .value(move || value.get())
            .auto_focus(true)
            .selected_range((0, len))
            .on_change(move |t: &str| value.set(t.to_owned()))
            .on_submit(move |_: &str| {
                submit();
                dialog::close();
            });
        dialog::modal(
            "dialog",
            "Rename bookmark",
            input,
            vec![dialog::Button::close("Cancel"), dialog::Button::new("Save", true, save)],
        )
    });
}

fn remove_bookmark(b: &Bookmark) {
    let id = b.id.clone();
    confirm("Remove bookmark", format!("Remove the bookmark \"{}\"?", b.title), "Remove", move || {
        workspace::remove_bookmark(&id)
    });
}

/// The Bookmarks pane (`renderBookmarks`): the star bookmarks what the document shows; each bookmark opens on a
/// press, is renamed on a double press, has a menu (Rename, Remove), the "×" removes it, and on desktops it
/// drags to another place.
fn bookmarks() -> impl IntoElement {
    let list = computed(workspace::bookmarks);
    let (small, small_line) = style::ui_sized("type.ui_small");
    view()
        .child(head("Bookmarks".into(), vec![icon_button("star", |_| bookmark_current())]))
        .child(dynamic(move || {
            if !list.with(Vec::is_empty) {
                return view().into_element();
            }
            view()
                .px(px(num("space.pane_empty_x")))
                .py(px(num("space.pane_empty_y")))
                .child(
                    ui_text("Bookmark a document, a zoomed item or a search with the ☆ button.", (small, small_line))
                        .text_color(color::muted()),
                )
                .into_element()
        }))
        .child(each(move || list.get(), |b: &Bookmark| b.id.clone(), move |b, index| bookmark_row(b, index, list)))
}

fn bookmark_row(b: ReadSignal<Bookmark>, index: ReadSignal<usize>, list: Memo<Vec<Bookmark>>) -> impl IntoElement {
    let hover = color::hover();
    let (size, line) = style::ui();
    let drop = drop_zone(false, index);
    let kind = {
        let b = b.get_untracked();
        if b.global || b.q.is_some() {
            "search"
        } else if b.zoom.is_some() {
            "zoomin"
        } else {
            "star"
        }
    };
    let at_menu = move |x: f32, y: f32| {
        let (r, x2) = (b.get_untracked(), b.get_untracked());
        menu::show(
            x,
            y,
            vec![menu::item("Rename", move || rename_bookmark(&r)), menu::item("Remove", move || remove_bookmark(&x2))],
        );
    };
    let mut row = view()
        .test_id("file")
        .group()
        .flex_row()
        .items_center()
        .gap(px(num("space.file_gap")))
        .h(px(num("space.file_row")))
        .pl(px(num("space.tag_row_x")))
        .pr(px(num("space.file_pad")))
        .hover(move |s| s.bg(hover))
        .on_press(move |p: &PressEvent| {
            let b = b.get_untracked();
            open_bookmark(&b);
            if p.clicks >= 2 {
                rename_bookmark(&b);
            }
        })
        .on_long_press(move |p: &PressEvent| at_menu(p.page_position.x.0, p.page_position.y.0))
        .on_context_menu(move |c: &ContextMenuEvent| at_menu(c.page_position.x.0, c.page_position.y.0))
        .child(
            view()
                .w(px(num("space.file_box")))
                .text_color(color::muted())
                .pointer_events(PointerEvents::None)
                .child(icon(kind, num("space.file_icon"))),
        )
        .child(
            ui_text(move || b.get().title, (size, line))
                .test_id("file-name")
                .flex_1()
                .truncate()
                .text_color(color::fg())
                .pointer_events(PointerEvents::None),
        )
        .child(
            view()
                .test_id("more")
                .self_stretch()
                .justify_center()
                .hidden()
                .group_hover(|s| s.flex())
                .on_press(move || remove_bookmark(&b.get_untracked()))
                .child(ui_text("×", (size, line)).text_color(color::muted())),
        )
        .child(drop_line(drop));
    if !style::phone() {
        let count = move || list.get_untracked().len();
        let drop = |from: usize, to: usize, z: Zone| {
            let Some(id) = workspace::bookmarks_now().get(from).map(|b| b.id.clone()) else { return };
            let mut to = if z == Zone::After { to + 1 } else { to };
            if from < to {
                to -= 1;
            }
            workspace::move_bookmark(&id, to);
        };
        row = row.on_touch(drag_rows(false, index, count, |_| false, drop)).claim_touches(TouchClaim::on_move());
    }
    row
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
                .on_press(|| {
                    let alpha = !side().with_untracked(|s| s.alpha);
                    side().update(|s| s.alpha = alpha);
                    keep(TAG_SORT, if alpha { "alpha" } else { "count" }.to_owned());
                })
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
