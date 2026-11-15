//! The outline view: a virtualized list of rows. A row shows its text as formatted spans; the row being edited
//! shows a native input instead (an EditText on Android, GPUI's text input on macOS), so the platform's input
//! methods, Korean composition included, work as in any text field. While it has the focus, the outliner's keys
//! are bound with `When::Always` so they reach the outliner before the field.

use nsh_base::{button, input};
use nsh_core::element::Position;
use nsh_core::LineHeight;
use nsh_core::{TouchEvent, TouchPhase};
use nsh_core::keymap::When;
use nsh_core::prelude::*;
use outliner_core::commands::Dir;
use outliner_core::item;

use crate::commands as cmd;
use crate::inline::{self, Block};
use crate::icons;
use crate::keymap;
use crate::store::{self, Row, Thread};
use crate::style::{self, color, num};

/// How an item is drawn, from its props.
#[derive(Debug, Clone, PartialEq, Default)]
struct Look {
    heading: i64,
    checked: bool,
    checkbox: bool,
    label: i64,
    /// The note, when it shows (it has more than spaces).
    note: Option<String>,
}

impl Look {
    /// Text size and line height (`.o-h1` .. `.o-h3`).
    fn type_size(&self) -> (f32, f32) {
        style::heading(self.heading)
    }
}

fn look_of(id: &str) -> Look {
    // notes hidden (`[data-notes='hide']`): none shows, unless it is edited
    let notes = crate::settings::get().pick("show_notes") != "hide";
    store::read(|s| {
        let n = s.tree().get(id);
        Look {
            heading: item::heading(n),
            checked: item::is_checked(n),
            checkbox: item::has_checkbox(n),
            label: item::color(n),
            note: Some(item::note(n)).filter(|t| notes && !t.trim().is_empty()).map(str::to_owned),
        }
    })
}

pub fn outline() -> impl IntoElement {
    let prefs = crate::settings::get();
    // the rows of the bullet thread, read by every cell (none with "Bullet threading" off)
    let threading = prefs.flag("bullet_threading");
    let path = computed(move || if threading { store::read(|s| s.path_rows()) } else { Vec::new() });
    // the rows that show as selected, read by every cell
    let selected = computed(|| store::read(|s| s.selected_rows()));
    // how many items link to each item, while backlinks show (`show_backlinks`)
    let counts = computed(crate::links::counts);
    // Escape with nothing edited or selected: back where the last Escape left
    on_key("escape", |_| {
        if store::peek(|s| s.block.is_none() && s.editing.is_none()) {
            cmd::resume();
        }
    });
    // Mod+F with nothing edited: the search box
    on_key("mod+f", |_| crate::find::open(None));
    virtual_list(|| store::read(|s| s.rows.len()), move |at| row(at, path, selected, counts))
        .keys(|| store::read(|s| s.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>()))
        .estimated_item_size(px(style::line() + 2.0 * style::pad()))
        .header(title())
        .footer(view().child(doc_end()).child(crate::links::backlinks()))
        .test_id("outline")
        .flex_1()
        // the narrow document layout: no wider than `space.doc_max`, in the middle (`.doc-wrap`)
        .max_w(if prefs.flag("center_doc") { px(num("space.doc_max")) } else { px(f32::MAX) })
        .bg(color::bg())
}

/// The zoom root's text above its items (`.o-title`): 28px, the document's name at the top level, formatted as
/// rows are; its note under it. Both are edited in place as an item's are (a press on either); a right click
/// (a long press on phones) opens the item menu on the zoom root.
fn title() -> impl IntoElement {
    let size = style::efs() * num("type.title");
    let title_line = size * num("type.title_line");
    let content = computed(|| store::read(|s| store::content_of(s, &s.view.zoom)));
    let note = computed(|| {
        store::read(|s| Some(item::note(s.tree().get(&s.view.zoom)).to_owned()).filter(|t| !t.trim().is_empty()))
    });
    // the zoom root and whether its note is edited, while it is
    let editing = computed(|| store::read(|s| s.editing.as_ref().filter(|c| c.id == s.view.zoom).map(|c| (c.id.clone(), c.note))));
    let menu_at = |x: f32, y: f32| crate::itemmenu::open(&cmd::zoom(), x, y);
    view()
        .test_id("title")
        .pl(px(style::text_left(0) - num("space.title_offset")))
        .pr(px(style::doc_pad().1))
        .pt(px(num("space.doc_top") + num("space.title_top")))
        .pb(px(num("space.title_bottom")))
        .on_context_menu(move |c: &ContextMenuEvent| menu_at(c.page_position.x.0, c.page_position.y.0))
        .on_long_press(move |p: &PressEvent| menu_at(p.page_position.x.0, p.page_position.y.0))
        .child(dynamic(move || {
            if let Some((id, false)) = editing.get() {
                return title_input(id, size, title_line).into_element();
            }
            let edit = || cmd::start_editing(&cmd::zoom(), None);
            let src = content.get();
            if src.is_empty() {
                // `.o-title .o-content:empty::before`
                return text("Untitled")
                    .test_id("title-text")
                    .font_family(style::editor_font())
                    .text_size(px(size))
                    .font_weight(style::weight("type.title_weight"))
                    .line_height(LineHeight::Points(px(title_line)))
                    .text_color(color::faint())
                    .on_press(edit)
                    .into_element();
            }
            let look = inline::Look::new(size, store::now()).pressable().dates_of(&cmd::zoom(), false);
            view()
                .test_id("title-text")
                .on_press(edit)
                .children(inline::blocks(&src).into_iter().map(|b| match b {
                    Block::Inline(tokens) => text("")
                        .font_family(style::editor_font())
                        .text_size(px(size))
                        .font_weight(style::weight("type.title_weight"))
                        .line_height(LineHeight::Points(px(title_line)))
                        .text_color(color::fg())
                        .children(inline::spans(&tokens, &look))
                        .on_press(edit)
                        .into_element(),
                    Block::Code { text, .. } => inline::code_block(&text, &look).into_element(),
                }))
                .into_element()
        }))
        .child(dynamic(move || {
            if let Some((id, true)) = editing.get() {
                return view().mt(px(num("space.title_note"))).child(note_input(id, false)).into_element();
            }
            match note.get() {
                Some(n) => note_text(&n, false, &[], &cmd::zoom())
                    .mt(px(num("space.title_note")))
                    .on_press(|| cmd::start_editing_note(&cmd::zoom()))
                    .into_element(),
                None => view().into_element(),
            }
        }))
}

/// The input of the title being edited (`.o-title .o-content`): the title's type. Return makes the zoom root's
/// first item (`enter` on the title), Down from its last line goes to the first item, Escape puts the caret at
/// its start (the web's title keeps the caret); at the top level, the document's name follows its text.
fn title_input(id: String, size: f32, line: f32) -> impl IntoElement {
    let field = element_ref();
    let (sel, sel_end) = follow_caret(&id, false, field);
    crate::links::register(field);
    item_keys(&id, false, true);
    let opts = KeyOptions { when: When::Always, repeat: true, ..Default::default() };
    {
        let id = id.clone();
        on_key_with("escape", opts, move |_| {
            if is_edited(&id, false) {
                cmd::start_editing(&id, Some(0));
            }
        });
    }
    let caret_box = signal(None::<(f32, f32)>);
    let field_h = signal(0.0_f32);
    {
        let id = id.clone();
        let opts = KeyOptions { pass_through: true, ..opts };
        on_key_with("down", opts, move |_| {
            if is_edited(&id, false) && on_edge(caret_box.get_untracked(), field_h.get_untracked(), true) {
                cmd::step(&id, true, false);
            }
        });
    }
    let (value_id, change_id, submit_id, caret_id, paste_id) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
    input()
        .element_ref(&field)
        .value(move || store::read(|s| store::content_of(s, &value_id)))
        .test_id("title-input")
        .auto_focus(true)
        .selected_range((sel.get_untracked(), sel_end.get_untracked()))
        .multiline(true)
        .submit_behavior(SubmitBehavior::Submit)
        .auto_correct(false)
        .p_0()
        .border(px(0.0))
        .bg(AdaptiveColor::TRANSPARENT)
        .font_family(style::editor_font())
        .text_size(px(size))
        .font_weight(style::weight("type.title_weight"))
        .line_height(LineHeight::Points(px(line)))
        .min_h(px(line))
        .text_color(color::fg())
        .selection_color(color::selection())
        .on_change(move |t: &str| {
            let old = store::peek(|s| store::content_of(s, &change_id));
            cmd::set_text(&change_id, false, t, sel.get_untracked());
            if change_id == outliner_core::ROOT {
                let doc = store::peek(|s| s.doc.clone());
                crate::workspace::sync_title(&doc, t.trim());
            }
            crate::datepicker::typed(&change_id, false, &old, t);
            crate::links::typed(&change_id, false, &old, t);
        })
        .paste_behavior(PasteBehavior::Report)
        .on_paste(move |p: &PasteEvent| {
            cmd::paste(&paste_id, false, sel.get_untracked(), sel_end.get_untracked(), &p.text, &p.html);
        })
        .on_caret_move(move |c: &CaretEvent| {
            let b = c.bounds;
            caret_box.set((b.size.height.0 > 0.0).then_some((b.origin.y.0, b.origin.y.0 + b.size.height.0)));
            crate::datepicker::caret_at(&caret_id, false, c.page_bounds);
            crate::links::caret_at(c.page_bounds);
        })
        .on_layout(move |b: &Bounds<Points>| field_h.set(b.size.height.0))
        .on_selection_change(on_selection(&id, false, sel, sel_end))
        .on_submit(move |_: &str| cmd::enter(&submit_id, sel.get_untracked(), sel_end.get_untracked()))
}

/// The room below the rows, a row tall: a press there edits the last row, or gives an empty document its first
/// item (the web: a click below the document).
fn doc_end() -> impl IntoElement {
    view()
        .test_id("doc-end")
        .h(px(style::line() + 2.0 * style::pad()))
        .on_press(cmd::focus_end)
}

/// A note (`.o-note`): 5/6 of the text in the note color, on 1.2 lines; formatted as the content is. With the
/// notes setting on "1st line", one line cut with an ellipsis, its line breaks as spaces (`[data-notes='teaser']`:
/// `white-space: nowrap`).
fn note_text(src: &str, checked: bool, marks: &[String], id: &str) -> View {
    let size = style::efs() * num("type.note");
    let note_line = size * num("type.note_line");
    let fg = if checked { color::check() } else { color::note() };
    let look = inline::Look::new(size, store::now()).colored(fg, checked).marking(marks, src).pressable().dates_of(id, true);
    let teaser = crate::settings::get().pick("show_notes") == "teaser";
    if teaser {
        let one = src.replace('\n', " ");
        let tokens = outliner_core::inline::parse_inline(&one);
        let mut t = text("")
            .font_family(style::editor_font())
            .text_size(px(size))
            .line_height(LineHeight::Points(px(note_line)))
            .text_color(fg)
            .children(inline::spans(&tokens, &look))
            .truncate();
        if checked {
            t = t.line_through();
        }
        return view().test_id("note").child(t);
    }
    view().test_id("note").children(inline::blocks(src).into_iter().map(move |b| match b {
        Block::Inline(tokens) => {
            let mut t = text("")
                .font_family(style::editor_font())
                .text_size(px(size))
                .line_height(LineHeight::Points(px(note_line)))
                .text_color(fg)
                .children(inline::spans(&tokens, &look));
            if checked {
                t = t.line_through();
            }
            t.into_element()
        }
        Block::Code { text, .. } => inline::code_block(&text, &look).into_element(),
    }))
}

/// A cell, reused for other rows as the list scrolls: its bindings read the row where it is now.
fn row(
    at: Position,
    path: Memo<Vec<usize>>,
    selected: Memo<Vec<Option<u16>>>,
    counts: Memo<std::collections::HashMap<outliner_core::NodeId, usize>>,
) -> impl IntoElement {
    let row = computed(move || store::read(|s| s.rows.get(at.index()).cloned()));
    let look = computed(move || row.get().map(|r| look_of(&r.id)).unwrap_or_default());
    let thread = computed(move || row.get().map(|r| path.with(|p| Thread::of(p, at.index(), r.depth))).unwrap_or_default());
    let depth = move || row.get().map_or(0, |r| r.depth);
    // the depth of the selected item this row is in (the item itself or one above it), while it is in one
    let picked = computed(move || selected.with(|v| v.get(at.index()).copied().flatten()));
    // the id of this row while it is the one being edited, and whether its note is: the input is made for that
    // item and field, and made again when the cell is reused for another row that is being edited too (after a
    // zoom, say)
    let editing = computed(move || {
        let id = row.get().map(|r| r.id)?;
        store::read(|s| s.editing.as_ref().filter(|c| c.id == id).map(|c| c.note)).map(|note| (id, note))
    });
    let highlight = crate::settings::get().flag("current_highlight");
    let cell = element_ref();
    let hover_bar = crate::itemmenu::bar_on();
    let mut v = view().element_ref(&cell);
    if !style::phone() {
        // a drag from the row's text onto other rows selects them (desktops: the mouse)
        v = v.on_touch(drag_select(at));
    }
    if hover_bar {
        v = v
            .on_mouse_enter(move |_: &MouseEvent| {
                if let Some(r) = row.get_untracked() {
                    crate::itemmenu::entered(&r.id);
                }
            })
            .on_mouse_leave(|_: &MouseEvent| crate::itemmenu::left());
    }
    register_cell(at, cell);
    v.test_id("row")
        .flex_row()
        .items_start()
        .py(px(style::pad()))
        // the row being edited, with "Highlight current item" on (`.o-hl-current .o-current > .o-row`)
        .bg(move || {
            if highlight && editing.get().is_some() { AdaptiveColor::from(color::current()) } else { AdaptiveColor::TRANSPARENT }
        })
        // the document's right padding: a list's padding does not reach its cells
        .pr(px(style::doc_pad().1))
        .pl(move || px(style::text_left(depth()) - style::body_gap() - bullet_w(row.get().map_or(0, |r| r.num))))
        // a selected item's background (`.o-selected > .o-row`), and under it that of its children's box
        // (`.o-selected > .o-kids`)
        .child(dynamic(move || match (picked.get(), row.get()) {
            (Some(d), Some(r)) => selected_bg(d, &r).into_element(),
            _ => view().into_element(),
        }))
        // the guides of the levels above: one line per ancestor, under its bullet (`.o-kids` border); a selected
        // item's children's guide gives way to the selection's background (`.o-selected > .o-kids`: transparent)
        .child(dynamic(move || {
            let hide = picked.get().filter(|&d| d < depth());
            (0..depth())
                .map(|k| {
                    let c = if Some(k) == hide { AdaptiveColor::TRANSPARENT } else { color::guide().into() };
                    view()
                        .test_id("guide")
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(style::guide_x(k)))
                        .w(px(num("space.guide_w")))
                        .bg(c)
                })
                .collect::<Vec<_>>()
        }))
        // the bullet thread over them
        .child(dynamic(move || {
            let (t, r) = (thread.get(), row.get());
            r.map(|r| thread_marks(&t, &r, look.get().type_size().1)).unwrap_or_default()
        }))
        // the count of the items linking here (`.o-backlinks`), left of the bullet
        .child(dynamic(move || {
            let Some(r) = row.get() else { return view().into_element() };
            match counts.with(|c| c.get(&r.id).copied()) {
                Some(n) => {
                    let left = style::text_left(r.depth) - style::body_gap() - bullet_w(r.num);
                    crate::links::badge(&r.id, n, look.get().type_size().1, left).into_element()
                }
                None => view().into_element(),
            }
        }))
        .child(bullet(row, look, thread))
        .child(when(move || look.get().checkbox, move || checkbox(row, look)))
        .child(
            view()
                .flex_1()
                // after a checkbox, the text sits its gap from the checkbox's visible box (`.o-check + .o-body`)
                .pl(move || px(if look.get().checkbox { check_gap().max(0.0) } else { style::body_gap() }))
                .ml(move || px(if look.get().checkbox { check_gap().min(0.0) } else { 0.0 }))
                .child(dynamic(move || match editing.get() {
                    Some((_, false)) => row_input(row.get_untracked()).into_element(),
                    _ => row_text(row, look).into_element(),
                }))
                // the note under the content (`.o-note`), while it has text or is being edited
                .child(dynamic(move || {
                    if let Some((id, true)) = editing.get() {
                        return note_input(id, look.get_untracked().checked).into_element();
                    }
                    let l = look.get();
                    let marks = row.get().map(|r| crate::find::highlight(&r.id)).unwrap_or_default();
                    match &l.note {
                        Some(n) => note_text(n, l.checked, &marks, &row.get_untracked().map(|r| r.id).unwrap_or_default())
                            .on_press(move |p: &PressEvent| {
                                if let Some(r) = row.get_untracked() {
                                    press_row(&r.id, true, &p.modifiers);
                                }
                            })
                            .into_element(),
                        None => view().into_element(),
                    }
                })),
        )
        // where dragged items would go (`.o-drop-line`)
        .child(dynamic(move || match dropping_at(at.index()) {
            Some((mode, depth)) => drop_line(mode, depth).into_element(),
            None => view().into_element(),
        }))
        // the hover toolbar over the row (`.hover-bar`), while the pointer is on it
        .child(dynamic(move || {
            let Some(r) = row.get() else { return view().into_element() };
            if hover_bar && crate::itemmenu::shows_on(&r.id) && !dragging_items() {
                crate::itemmenu::bar(&r.id).into_element()
            } else {
                view().into_element()
            }
        }))
}

/// The background of a row in a selected item at `depth` (`--sel`): the item's own row from its bullet, rounded
/// (`.o-row`); a row under it from the selected item's children's guide, square (`.o-kids`).
fn selected_bg(depth: u16, r: &Row) -> View {
    let own = r.depth == depth;
    let left = if own { style::text_left(depth) - style::body_gap() - bullet_w(r.num) } else { style::guide_x(depth) };
    let bg = view()
        .test_id("selected")
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .left(px(left))
        .right(px(style::doc_pad().1))
        .bg(color::sel());
    if own { bg.rounded(px(num("radius.row"))) } else { bg }
}

/// Whether the platform's command key is among `m` (Command on Apple keyboards, Control elsewhere).
fn mod_held(m: &nsh_core::keymap::Modifiers) -> bool {
    if apple_keys() { m.cmd } else { m.ctrl }
}

/// A press on a row's content (or its note, `note`): edits it there, or in select mode selects the item or takes
/// it out of the selection. With Shift (desktops), the items from the selection's start to it are selected; with
/// the command key, it joins the selection or leaves it (`onMouseDown`).
fn press_row(id: &str, note: bool, m: &nsh_core::keymap::Modifiers) {
    let something = store::peek(|s| s.editing.is_some() || s.block.is_some());
    if m.shift && something {
        cmd::select_to(id);
    } else if mod_held(m) || store::peek(|s| s.select_mode) {
        cmd::toggle_in_block(id);
    } else if note {
        cmd::start_editing_note(id);
    } else {
        cmd::start_editing(id, None);
    }
}

/// The bullet thread's pieces in a row (`.o-thread` in the web's styles.css): straight through the row on the
/// guides it passes, from an ancestor's bullet down its children's guide, and a rounded turn from the parent's
/// guide into the bullet of a row on the path. `bullet_h`: the bullet's box, which the thread meets at its middle.
fn thread_marks(t: &Thread, r: &Row, bullet_h: f32) -> Vec<View> {
    let w = num("space.thread");
    let mid = style::pad() + bullet_h / 2.0;
    let line = |depth: u16| view().absolute().left(px(style::thread_x(depth) - w / 2.0)).w(px(w));
    let mut out: Vec<View> = t
        .through
        .iter()
        .map(|&k| line(k).test_id("thread").top(px(0.0)).bottom(px(0.0)).bg(color::thread()))
        .collect();
    if t.down {
        out.push(line(r.depth).test_id("thread").top(px(mid)).bottom(px(0.0)).bg(color::thread()));
    }
    if t.elbow {
        let left = style::thread_x(r.depth - 1) - w / 2.0;
        out.push(
            view()
                .test_id("elbow")
                .absolute()
                .left(px(left))
                .top(px(0.0))
                .w(px(style::elbow_end(r.depth, r.num > 0) - left))
                .h(px(mid + w / 2.0))
                .border_l(px(w))
                .border_b(px(w))
                .border_color(color::thread())
                .rounded_bl(px(num("radius.elbow"))),
        );
    }
    out
}

/// The bullet column's width: the bullet's, or the number's (`.o-numbered > .o-row > .o-bullet`).
fn bullet_w(n: u32) -> f32 {
    if n > 0 { num("space.number") } else { style::bullet_w() }
}

/// The bullet (`.o-bullet`): a 6px dot in its 18px column, as tall as the text's line; ringed while its
/// children are folded. Under a numbered parent, the row's number instead (`1.`), right-aligned 6px from the
/// text, in figures of one width, at .85em. On the bullet thread, both take its color. A press folds or unfolds;
/// on desktops, under the pointer, the dot of an item with children turns into a triangle pointing down, or
/// right while folded.
///
/// A right click (a long press on phones) opens the item menu; on desktops, dragging it moves the item (the
/// selected items, when it is one of them) to where it is let go, a copy with the command key held.
fn bullet(row: Memo<Option<Row>>, look: Memo<Look>, thread: Memo<Thread>) -> impl IntoElement {
    let folded = move || row.get().is_some_and(|r| r.has_kids && r.collapsed);
    let kids = move || row.get().is_some_and(|r| r.has_kids);
    let number = move || row.get().map_or(0, |r| r.num);
    let on_path = move || thread.get().path;
    let menu_at = move |x: f32, y: f32| {
        if let Some(r) = row.get_untracked() {
            crate::itemmenu::open(&r.id, x, y);
        }
    };
    // the menu and the drag are the bullet's box's (a button takes none of them)
    let mut host = view()
        .test_id("bullet-box")
        .on_context_menu(move |c: &ContextMenuEvent| menu_at(c.page_position.x.0, c.page_position.y.0))
        .on_long_press(move |p: &PressEvent| menu_at(p.page_position.x.0, p.page_position.y.0));
    if !style::phone() {
        host = host
            .on_press_down(|p: &PressEvent| COPY.with(|c| c.set(mod_held(&p.modifiers))))
            .on_touch(drag_items(row))
            .claim_touches(TouchClaim::on_move());
    }
    host.child(button().group()
        .w(move || px(bullet_w(number())))
        .h(move || px(look.get().type_size().1))
        .items_center()
        .justify_center()
        .on_press(move || {
            if let Some(r) = row.get_untracked() {
                if store::peek(|s| s.select_mode) {
                    cmd::toggle_in_block(&r.id);
                } else if crate::settings::get().flag("use_bullet_to_zoom") {
                    // "Use bullet point to zoom in": the bullet zooms in on its item instead of folding it
                    cmd::zoom_into(&r.id);
                } else {
                    cmd::collapse(&[r.id], false);
                }
            }
        })
        .child(dynamic(move || {
            let n = number();
            if n > 0 {
                return view()
                    .test_id("bullet")
                    .self_stretch()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .pr(px(num("space.number_gap")))
                    .child(
                        text(format!("{n}."))
                            .font_family(style::editor_font())
                            .text_size(px(style::efs() * num("type.number")))
                            .text_color(if on_path() { color::thread() } else { color::muted() })
                            .tabular_nums(),
                    )
                    .into_element();
            }
            // the ring of a folded item is a border around the dot (a box shadow on the web)
            let ring = move || if folded() { num("space.fold_ring") } else { 0.0 };
            let size = move || style::dot() + 2.0 * ring();
            // a folded dot keeps its look on the path (`.o-collapsed > .o-row > .o-bullet:empty::before`)
            let fill = move || if on_path() && !folded() { color::thread() } else { color::muted() };
            let dot = view()
                .test_id("bullet")
                .w(move || px(size()))
                .h(move || px(size()))
                .rounded(move || px(size() / 2.0))
                .bg(fill)
                .border(move || px(ring()))
                .border_color(color::faint());
            if !kids() || style::phone() {
                return dot.into_element();
            }
            view()
                .child(dot.group_hover(|s| s.hidden()))
                .child(fold_mark(folded()).hidden().group_hover(|s| s.flex()))
                .into_element()
        })))
}

/// The triangle a bullet with children shows under the pointer (`.o-bullet:hover:empty::before` on items with
/// children): pointing down, or right while `folded`, in the text color.
fn fold_mark(folded: bool) -> View {
    let (base, h) = (num("space.fold_mark"), num("space.fold_mark_h"));
    let (w, ht, d) = if folded {
        (h, base, format!("M0 0L{h} {}L0 {base}z", base / 2.0))
    } else {
        (base, h, format!("M0 0H{base}L{} {h}z", base / 2.0))
    };
    view().test_id("fold-mark").text_color(color::fg()).child(icons::shape(&d, w, ht))
}

/// Space from the checkbox's touch target to the text: the gap from its visible box, less the target's margin
/// around that box (phones).
fn check_gap() -> f32 {
    num("space.check_gap") - (num("space.check_hit") - num("space.check")) / 2.0
}

/// The check mark's size, of the checkbox's: the web draws it as a rotated border, here it is a character.
const MARK: f32 = 0.75;

/// The checkbox of an item that shows one (`.o-check`): 16px, centered on the first line; a press checks it.
fn checkbox(row: Memo<Option<Row>>, look: Memo<Look>) -> impl IntoElement {
    let on = move || look.get().checked;
    // the test id names the wrapper: a button takes none
    let size = num("space.check");
    // the touch target (phones: larger than the box), the box centered in it
    let hit = num("space.check_hit");
    let target = view()
        .test_id("checkbox")
        .w(px(hit))
        .h(px(hit))
        .items_center()
        .justify_center()
        .mt(move || px((look.get().type_size().1 - hit) / 2.0))
        .ml(px(num("space.check_left")));
    target.child(
        button()
            .w(px(size))
            .h(px(size))
            .rounded(px(num("radius.check")))
            .border(px(num("space.check_border")))
            .border_color(move || if on() { color::muted() } else { color::check() })
            .bg(move || if on() { color::muted() } else { color::bg() })
            .items_center()
            .justify_center()
            .on_press(move || {
                if let Some(r) = row.get_untracked() {
                    cmd::check(&r.id);
                }
            })
            .child(when(on, move || text("✓").text_size(px(size * MARK)).text_color(color::bg()))),
    )
}

/// The row's text as formatted spans (`.o-content`), code blocks under their lines; a press starts editing it.
fn row_text(row: Memo<Option<Row>>, look: Memo<Look>) -> impl IntoElement {
    let content = computed(move || row.get().map(|r| store::read(|s| store::content_of(s, &r.id))).unwrap_or_default());
    // the item's look sets static styles: the text is made again when it changes (rarely: a heading, a check)
    dynamic(move || {
        let l = look.get();
        let (size, line) = l.type_size();
        let src = content.get();
        let fg = if l.checked { color::check() } else { color::fg() };
        // a search's words marked in a matching item (`.o-match`)
        let marks = row.get().map(|r| crate::find::highlight(&r.id)).unwrap_or_default();
        let shown = inline::Look::new(size, store::now()).colored(fg, l.checked).marking(&marks, &src).pressable();
        let shown = match row.get_untracked() {
            Some(r) => shown.dates_of(&r.id, false),
            None => shown,
        };
        let edit = move |p: &PressEvent| {
            if let Some(r) = row.get_untracked() {
                press_row(&r.id, false, &p.modifiers);
            }
        };
        let paragraph = |tokens: &[outliner_core::inline::Token]| {
            let mut t = text("")
                .font_family(style::editor_font())
                .text_size(px(size))
                .line_height(LineHeight::Points(px(line)))
                .text_color(fg)
                .children(inline::spans(tokens, &shown))
                .on_press(edit);
            if l.heading > 0 {
                t = t.font_weight(style::weight("type.heading_weight"));
            }
            // checked items are struck through (`.o-checked`)
            if l.checked {
                t = t.line_through();
            }
            t
        };
        let mut blocks = inline::blocks(&src);
        if let [Block::Inline(tokens)] = &blocks[..] {
            // as wide as its text, as `.o-content` is (a color label covers the text only)
            let mut t = paragraph(tokens).self_start().test_id("row-text");
            // a color label is a rounded background behind the text (`.o-cl1` .. `.o-cl6`)
            if let Some(bg) = color::label(l.label) {
                t = t.bg(bg).rounded(px(num("radius.label")));
            }
            return t.into_element();
        }
        let mut v = view().test_id("row-text").on_press(edit);
        if let Some(bg) = color::label(l.label) {
            v = v.bg(bg).rounded(px(num("radius.label")));
        }
        v.children(blocks.drain(..).map(|b| match b {
            Block::Inline(tokens) => paragraph(&tokens).into_element(),
            Block::Code { text, .. } => inline::code_block(&text, &shown).into_element(),
        }))
        .into_element()
    })
}

fn content_len(id: &str) -> u32 {
    store::peek(|s| cmd::utf16_len(&store::content_of(s, id)))
}

fn note_of(id: &str) -> String {
    store::peek(|s| item::note(s.tree().get(id)).to_owned())
}

/// Whether `id`'s content (or its note, `note`) is what is being edited: a spare cell out of view may still hold
/// an input, whose keys must not act.
fn is_edited(id: &str, note: bool) -> bool {
    store::peek(|s| s.editing.as_ref().is_some_and(|c| c.id == id && c.note == note))
}

/// The keys of the item being edited, in its content or its note (`note`): bound with `When::Always`, so they
/// reach the outliner before the input. They act on the store's caret, which the input keeps current.
fn item_keys(id: &str, note: bool, title: bool) {
    let opts = KeyOptions { when: When::Always, repeat: true, ..Default::default() };
    let bind = |keys: &str, f: fn()| {
        let id = id.to_owned();
        on_key_with(keys, opts, move |_| {
            if is_edited(&id, note) {
                f();
            }
        });
    };
    // the keymap's commands (`settings` > Keymap, `keymap::COMMANDS`); Tab is the input's own (below)
    for (keys, run) in keymap::bindings(keymap::Scope::Editing, &["indent"]) {
        let id = id.to_owned();
        on_key_with(&keys, opts, move |_| {
            if is_edited(&id, note) {
                run();
            }
        });
    }
    if !title {
        bind("escape", cmd::stop_editing);
    }
    bind("mod+d", crate::datepicker::add_date);
    bind("mod+f", || crate::find::open(None));
}

/// The input of the row being edited. Made when editing moves to the row, gone when it moves away; the keys
/// bound here live as long as it does. Its text is the item's (a command may change it while it shows), and a
/// command that places the caret moves its selection there.
fn row_input(row: Option<Row>) -> impl IntoElement {
    let Some(row) = row else { return view().into_element() };
    let id = row.id.clone();
    let field = element_ref();
    let (sel, sel_end) = follow_caret(&id, false, field);
    crate::links::register(field);

    item_keys(&id, false, false);
    let opts = KeyOptions { when: When::Always, repeat: true, ..Default::default() };
    for keys in keymap::keys_of("indent") {
        let id = id.clone();
        on_key_with(&keys, opts, move |_| {
            if is_edited(&id, false) {
                cmd::indent();
            }
        });
    }
    // Up and Down move to the row above or below from the input's first or last line only (`verticalMove` on
    // the web); elsewhere they go on to the input, which moves its caret between its lines. Shift+Up and
    // Shift+Down select the item there (`extendBlock`), and elsewhere extend the text selection.
    let caret_box = signal(None::<(f32, f32)>);
    let field_h = signal(0.0_f32);
    for (keys, down, select) in [("up", false, false), ("down", true, false), ("shift+up", false, true), ("shift+down", true, true)] {
        let id = id.clone();
        let opts = KeyOptions { pass_through: true, ..opts };
        on_key_with(keys, opts, move |_| {
            if !is_edited(&id, false) {
                return;
            }
            let edge = on_edge(caret_box.get_untracked(), field_h.get_untracked(), down);
            if !select {
                if edge {
                    cmd::step(&id, down, false);
                }
                return;
            }
            let (a, b) = (sel.get_untracked(), sel_end.get_untracked());
            let at_end = if down { b == content_len(&id) } else { a == 0 };
            if edge || at_end {
                cmd::extend(down);
            }
        });
    }
    // Mod+A with all the text selected selects the item
    {
        let id = id.clone();
        let opts = KeyOptions { pass_through: true, ..opts };
        on_key_with("mod+a", opts, move |_| {
            let len = content_len(&id);
            if is_edited(&id, false) && sel.get_untracked() == 0 && sel_end.get_untracked() == len && len > 0 {
                cmd::extend(true);
            }
        });
    }
    // a multiline input breaks the line on Alt+Return: an item's text gets no line break from it
    on_key_with("alt+enter", opts, |_| {});

    let (value_id, change_id, submit_id, key_id, caret_id, paste_id) =
        (id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
    input()
        .element_ref(&field)
        .value(move || store::read(|s| store::content_of(s, &value_id)))
        .test_id("row-input")
        .auto_focus(true)
        .selected_range((sel.get_untracked(), sel_end.get_untracked()))
        // several lines, as tall as its text (`.o-content` wraps); a plain return still submits (splits the row)
        .multiline(true)
        .submit_behavior(SubmitBehavior::Submit)
        .auto_correct(false)
        .p_0()
        .border(px(0.0))
        .bg(AdaptiveColor::TRANSPARENT)
        .font_family(style::editor_font())
        .text_size(px(style::efs()))
        .line_height(LineHeight::Points(px(style::line())))
        .min_h(px(style::line()))
        .text_color(color::fg())
        .selection_color(color::selection())
        .on_change(move |t: &str| {
            let old = store::peek(|s| store::content_of(s, &change_id));
            cmd::set_text(&change_id, false, t, sel.get_untracked());
            crate::datepicker::typed(&change_id, false, &old, t);
            crate::links::typed(&change_id, false, &old, t);
        })
        // a paste is the app's: several lines, HTML or OPML become items (`onPaste`)
        .paste_behavior(PasteBehavior::Report)
        .on_paste(move |p: &PasteEvent| {
            cmd::paste(&paste_id, false, sel.get_untracked(), sel_end.get_untracked(), &p.text, &p.html);
        })
        .on_caret_move(move |c: &CaretEvent| {
            let b = c.bounds;
            // an empty caret: the platform could not place it
            caret_box.set((b.size.height.0 > 0.0).then_some((b.origin.y.0, b.origin.y.0 + b.size.height.0)));
            // the date picker and the suggestions open under it
            crate::datepicker::caret_at(&caret_id, false, c.page_bounds);
            crate::links::caret_at(c.page_bounds);
        })
        .on_layout(move |b: &Bounds<Points>| field_h.set(b.size.height.0))
        .on_selection_change(on_selection(&id, false, sel, sel_end))
        .on_submit(move |_: &str| cmd::enter(&submit_id, sel.get_untracked(), sel_end.get_untracked()))
        .on_key_press(move |k: &str| {
            if k == "Backspace" && sel.get_untracked() == 0 && sel_end.get_untracked() == 0 {
                cmd::backspace(&key_id);
            }
        })
        .into_element()
}

/// The selection of the input editing `id`'s content or note: starts where the store's caret is, follows the
/// input (and tells the store), and moves the input's selection when a command (not typing) puts the caret
/// somewhere in this field.
fn follow_caret(id: &str, note: bool, field: ElementRef) -> (Signal<u32>, Signal<u32>) {
    let (at, end) = store::peek(|s| s.editing.as_ref().map_or((0, 0), |c| (c.at, c.end)));
    let sel = signal(at);
    let sel_end = signal(end);
    let id = id.to_owned();
    let seen = std::cell::Cell::new(store::peek(|s| s.caret_rev));
    {
        effect(move || {
            let (rev, at) = store::read(|s| {
                (s.caret_rev, s.editing.as_ref().filter(|c| c.id == id && c.note == note).map(|c| (c.at, c.end)))
            });
            if rev != seen.get() {
                seen.set(rev);
                if let Some((a, b)) = at {
                    sel.set(a);
                    sel_end.set(b);
                    field.set_selection(a, b);
                }
            }
        });
    }
    (sel, sel_end)
}

/// The input's report of its selection: kept in `sel`..`sel_end` and in the store.
fn on_selection(id: &str, note: bool, sel: Signal<u32>, sel_end: Signal<u32>) -> impl Fn(&Selection) + 'static {
    let id = id.to_owned();
    move |s: &Selection| {
        sel.set(s.start);
        sel_end.set(s.end);
        cmd::caret_moved(&id, note, s.start, s.end);
        crate::links::caret_moved(&id, note, s.end);
    }
}

/// The input of the note being edited (`.o-note.o-editing`): under the content, in the note's type; several
/// lines (Return breaks the line, as in the web's notes). Shift+Return goes back to the content; Up from its first
/// line goes there too, Down from its last to the row below; Backspace or Left at its start leave it for the end of
/// the content (Backspace only when it is empty).
fn note_input(id: String, checked: bool) -> impl IntoElement {
    let size = style::efs() * num("type.note");
    let note_line = size * num("type.note_line");
    let field = element_ref();
    let (sel, sel_end) = follow_caret(&id, true, field);
    crate::links::register(field);
    item_keys(&id, true, false);
    let opts = KeyOptions { when: When::Always, repeat: true, ..Default::default() };
    // Tab types a tab in a note (the item is not indented)
    {
        let id = id.clone();
        on_key_with("tab", opts, move |_| {
            if is_edited(&id, true) {
                let (a, b) = (sel.get_untracked(), sel_end.get_untracked());
                cmd::replace(&id, true, a.min(b), a.max(b), "\t", a.min(b) + 1);
            }
        });
    }
    let caret_box = signal(None::<(f32, f32)>);
    let field_h = signal(0.0_f32);
    for (keys, down) in [("up", false), ("down", true)] {
        let id = id.clone();
        let opts = KeyOptions { pass_through: true, ..opts };
        on_key_with(keys, opts, move |_| {
            if is_edited(&id, true) && on_edge(caret_box.get_untracked(), field_h.get_untracked(), down) {
                cmd::step(&id, down, true);
            }
        });
    }
    // Left at the start, and Backspace at the start of an empty note, go to the end of the content
    for (keys, empty) in [("left", false), ("backspace", true)] {
        let id = id.clone();
        let opts = KeyOptions { pass_through: true, ..opts };
        on_key_with(keys, opts, move |_| {
            let at_start = sel.get_untracked() == 0 && sel_end.get_untracked() == 0;
            if is_edited(&id, true) && at_start && (!empty || note_of(&id).is_empty()) {
                cmd::leave_note(&id);
            }
        });
    }
    let (value_id, change_id, caret_id, paste_id) = (id.clone(), id.clone(), id.clone(), id.clone());
    let fg = if checked { color::check() } else { color::note() };
    view().test_id("note").child(
        input()
            .element_ref(&field)
            .value(move || store::read(|s| item::note(s.tree().get(&value_id)).to_owned()))
            .test_id("note-input")
            .auto_focus(true)
            .selected_range((sel.get_untracked(), sel_end.get_untracked()))
            .multiline(true)
            .submit_behavior(SubmitBehavior::Newline)
            .auto_correct(false)
            .p_0()
            .border(px(0.0))
            .bg(AdaptiveColor::TRANSPARENT)
            .font_family(style::editor_font())
            .text_size(px(size))
            .line_height(LineHeight::Points(px(note_line)))
            .min_h(px(note_line))
            .text_color(fg)
            .selection_color(color::selection())
            .on_change(move |t: &str| {
                let old = note_of(&change_id);
                cmd::set_text(&change_id, true, t, sel.get_untracked());
                crate::datepicker::typed(&change_id, true, &old, t);
                crate::links::typed(&change_id, true, &old, t);
            })
            // a note takes a paste as text, line breaks and all
            .paste_behavior(PasteBehavior::Report)
            .on_paste(move |p: &PasteEvent| {
                cmd::paste(&paste_id, true, sel.get_untracked(), sel_end.get_untracked(), &p.text, &p.html);
            })
            .on_caret_move(move |c: &CaretEvent| {
                let b = c.bounds;
                caret_box.set((b.size.height.0 > 0.0).then_some((b.origin.y.0, b.origin.y.0 + b.size.height.0)));
                crate::datepicker::caret_at(&caret_id, true, c.page_bounds);
                crate::links::caret_at(c.page_bounds);
            })
            .on_layout(move |b: &Bounds<Points>| field_h.set(b.size.height.0))
            .on_selection_change(on_selection(&id, true, sel, sel_end)),
    )
}

/// The keys of selected items (`handleHardKey` with a block selection on the web), bound while there are some:
/// no input has the focus then.
pub(crate) fn block_keys() -> impl IntoElement {
    let opts = KeyOptions { repeat: true, ..Default::default() };
    let bind = |keys: &str, f: fn()| on_key_with(keys, opts, move |_| f());
    bind("escape", cmd::clear_block);
    bind("backspace", || cmd::delete_items(Dir::Left));
    bind("delete", || cmd::delete_items(Dir::Right));
    bind("enter", cmd::replace_with_empty);
    bind("up", || cmd::leave_block("up"));
    bind("down", || cmd::leave_block("down"));
    bind("left", || cmd::leave_block("left"));
    bind("right", || cmd::leave_block("right"));
    bind("shift+up", || cmd::extend(false));
    bind("shift+down", || cmd::extend(true));
    bind("mod+a", cmd::select_more);
    // the keymap's commands (`settings` > Keymap)
    for (keys, run) in keymap::bindings(keymap::Scope::Block, &[]) {
        on_key_with(&keys, opts, move |_| run());
    }
    bind("mod+c", || cmd::copy_block(false));
    bind("mod+x", || cmd::copy_block(true));
    bind("mod+f", || crate::find::open(None));
    // a paste replaces the selected items with what is on the clipboard
    bind("mod+v", || {
        spawn(async {
            let text = clipboard_string().await;
            cmd::paste_over_block(&text);
        });
    });
    view()
}

// ---- dragging: selecting rows, moving items ----------------------------------------------------------------

thread_local! {
    /// The list's cells: the row each shows now and the cell, for the drags' hit tests.
    static CELLS: std::cell::RefCell<Vec<(std::rc::Rc<std::cell::Cell<usize>>, ElementRef)>> = Default::default();
    /// Where the rows were in the window when a drag began: row index to its top and height.
    static BOXES: std::cell::RefCell<Vec<(usize, f32, f32)>> = Default::default();
    /// The command key was held when the bullet was pressed: the drag copies.
    static COPY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// A drag from a row's text: the row it started on.
    static SELECTING: std::cell::RefCell<Option<NodeIdOf>> = const { std::cell::RefCell::new(None) };
}

type NodeIdOf = outliner_core::NodeId;

/// What a drag of items shows: the row a drop mark is on (with the mode and the row's depth), the pointer, how
/// many items, whether they are copied.
type DragView = (Option<(usize, DropMode, u16)>, (f32, f32), usize, bool);

/// Keep cell `cell` (showing row `at`) for the hit tests.
fn register_cell(at: Position, cell: ElementRef) {
    let index = std::rc::Rc::new(std::cell::Cell::new(0));
    {
        let index = index.clone();
        effect(move || index.set(at.index()));
    }
    CELLS.with(|c| c.borrow_mut().push((index, cell)));
}

/// Measure where the shown rows are in the window (cells out of view or gone are left out), then run `then`
/// with the left of the rows.
fn measure_rows(then: impl FnOnce(f32) + 'static) {
    let cells = CELLS.with(|c| c.borrow().clone());
    // ask for every cell at once: the answers come with the same layout
    let asked: Vec<_> = cells.iter().map(|(i, c)| (i.clone(), c.measure())).collect();
    spawn(async move {
        let mut boxes = Vec::new();
        let mut left = None;
        let mut gone = Vec::new();
        for (k, (i, answer)) in asked.into_iter().enumerate() {
            match answer.await {
                Some(m) if m.frame.size.height.0 > 0.0 => {
                    boxes.push((i.get(), m.page.y.0, m.frame.size.height.0));
                    left.get_or_insert(m.page.x.0);
                }
                Some(_) => {}
                None => gone.push(k),
            }
        }
        // the cells no longer shown anywhere are forgotten
        CELLS.with(|c| {
            let mut c = c.borrow_mut();
            for k in gone.into_iter().rev() {
                if k < c.len() {
                    c.remove(k);
                }
            }
        });
        BOXES.with(|b| *b.borrow_mut() = boxes);
        then(left.unwrap_or(0.0));
    });
}

/// The row at window height `y` when the drag began: its index and how far down it `y` is, from 0 to 1.
fn row_at(y: f32) -> Option<(usize, f32)> {
    let n = store::peek(|s| s.rows.len());
    BOXES.with(|b| b.borrow().iter().filter(|(i, ..)| *i < n).find(|(_, t, h)| y >= *t && y < t + h).map(|(i, t, h)| (*i, (y - t) / h)))
}

/// The touches of a row that select rows by dragging across them (`startDragSelect`): from the row the drag
/// started on to the row under the pointer.
fn drag_select(at: Position) -> impl Fn(&TouchEvent) + 'static {
    move |t: &TouchEvent| {
        let Some(p) = t.points.iter().find(|p| p.changed) else { return };
        match t.phase {
            TouchPhase::Start => {
                // a bullet's drag moves items instead
                if DRAG.with(|d| d.borrow().is_some()) {
                    return;
                }
                let Some(id) = store::peek(|s| s.rows.get(at.index()).map(|r| r.id.clone())) else { return };
                SELECTING.with(|s| *s.borrow_mut() = Some(id));
                BOXES.with(|b| b.borrow_mut().clear());
                measure_rows(|_| {});
            }
            TouchPhase::Move => {
                let Some(from) = SELECTING.with(|s| s.borrow().clone()) else { return };
                let Some((i, _)) = row_at(p.page.y.0) else { return };
                if let Some(to) = store::peek(|s| s.rows.get(i).map(|r| r.id.clone())) {
                    if to != from || store::peek(|s| s.block.is_some()) {
                        cmd::drag_select(&from, &to);
                    }
                }
            }
            TouchPhase::End | TouchPhase::Cancel => SELECTING.with(|s| *s.borrow_mut() = None),
        }
    }
}

/// Where dragged items would go, by the row under the pointer (`hitTest`'s modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropMode {
    Before,
    After,
    Inside,
}

/// Where dragged items go: under `parent` after `after`, shown at row `index` (`mode`).
#[derive(Debug, Clone, PartialEq)]
struct DropAt {
    index: usize,
    parent: NodeIdOf,
    after: Option<NodeIdOf>,
    mode: DropMode,
}

/// Items being dragged by a bullet: the items, where the finger went down, the rows' left in the window (once
/// the rows are measured), whether the drag has begun (the pointer moved), where it would drop, and the pointer.
#[derive(Debug, Clone, PartialEq)]
struct Drag {
    ids: Vec<NodeIdOf>,
    start: (f32, f32),
    left: Option<f32>,
    begun: bool,
    target: Option<DropAt>,
    at: (f32, f32),
}

thread_local! {
    static DRAG: std::cell::RefCell<Option<Drag>> = const { std::cell::RefCell::new(None) };
}

/// What the views show of a drag: where it would drop and the pointer (a signal; the drag itself is above).
#[derive(Debug, Clone, PartialEq, Default)]
struct Shown(Option<DragView>);

fn shown() -> Signal<Shown> {
    global::<Shown>()
}

fn show_drag(d: Option<&Drag>) {
    let v = d.filter(|d| d.begun).map(|d| {
        let depth = d.target.as_ref().map(|t| {
            let depth = store::peek(|s| s.rows.get(t.index).map_or(0, |r| r.depth));
            (t.index, t.mode, depth)
        });
        (depth, d.at, d.ids.len(), COPY.with(std::cell::Cell::get))
    });
    let v = Shown(v);
    if shown().with_untracked(|s| *s != v) {
        shown().set(v);
    }
}

/// Show item `id`'s row with a drop mark before it and the drag's label at `at`, as while an item is dragged
/// there (the fixtures' "drop").
pub(crate) fn show_drop(id: &str, at: (f32, f32)) {
    let Some((index, depth)) = store::peek(|s| s.row_index(id).map(|i| (i, s.rows[i].depth))) else { return };
    shown().set(Shown(Some((Some((index, DropMode::Before, depth)), at, 1, false))));
}

/// Whether items are being dragged (tracked).
fn dragging_items() -> bool {
    shown().with(|s| s.0.is_some())
}

/// The drop mark row `index` shows (tracked), with its depth.
fn dropping_at(index: usize) -> Option<(DropMode, u16)> {
    shown().with(|s| s.0.as_ref().and_then(|(t, ..)| *t).filter(|t| t.0 == index).map(|t| (t.1, t.2)))
}

/// The hit test (`hitTest`): the row under the pointer at window (`x`, `y`) and where in it the items would go.
fn hit(d: &Drag, x: f32, y: f32) -> Option<DropAt> {
    let left = d.left?;
    let (index, frac) = row_at(y)?;
    store::peek(|s| {
        let t = s.tree();
        let r = s.rows.get(index)?;
        let tid = r.id.clone();
        if d.ids.iter().any(|i| *i == tid || t.ancestors(&tid).contains(i)) {
            return None;
        }
        let parent = t.get(&tid)?.parent.clone()?;
        let upper = frac < 0.5;
        let kids = outliner_core::view::visible_children(t, &tid, &s.view);
        // the web's threshold for "inside": past the text's start by 100px and a fifth of the rest of the row,
        // and 30px more (`hitTest`)
        let text_left = left + style::text_left(r.depth);
        let width = window().width.0 - text_left - style::doc_pad().1;
        let threshold = text_left + 100.0 + 0.2 * (width - 100.0).max(0.0) + 30.0;
        if !upper && kids.is_empty() && x > threshold {
            let all = t.get(&tid)?.children.clone();
            let top_first = crate::settings::get().pick("move_item_sent_to_position") == "top";
            let after = if top_first { None } else { all.last().cloned() };
            return Some(DropAt { index, parent: tid, after, mode: DropMode::Inside });
        }
        if upper {
            let prev = t.prev_sibling(&tid).cloned();
            if prev.as_ref().is_some_and(|p| d.ids.contains(p)) && d.ids.len() == 1 {
                return None;
            }
            return Some(DropAt { index, parent, after: prev, mode: DropMode::Before });
        }
        if !kids.is_empty() {
            return Some(DropAt { index, parent: tid, after: None, mode: DropMode::After });
        }
        Some(DropAt { index, parent, after: Some(tid), mode: DropMode::After })
    })
}

/// The touches of a bullet that drag its item (the selected items, when it is one of them): the drag begins once
/// the pointer moves 3 points; the items move (or are copied) where it lets go.
fn drag_items(row: Memo<Option<Row>>) -> impl Fn(&TouchEvent) + 'static {
    move |t: &TouchEvent| {
        let Some(p) = t.points.iter().find(|p| p.changed) else { return };
        let (x, y) = (p.page.x.0, p.page.y.0);
        match t.phase {
            TouchPhase::Start => {
                let Some(r) = row.get_untracked() else { return };
                let ids = store::peek(|s| match &s.block {
                    Some(b) if b.ids.contains(&r.id) => outliner_core::view::normalize_block(s.tree(), &b.ids),
                    _ => vec![r.id.clone()],
                });
                DRAG.with(|d| {
                    *d.borrow_mut() = Some(Drag { ids, start: (x, y), left: None, begun: false, target: None, at: (x, y) })
                });
                BOXES.with(|b| b.borrow_mut().clear());
                measure_rows(|left| {
                    DRAG.with(|d| {
                        if let Some(d) = d.borrow_mut().as_mut() {
                            d.left = Some(left);
                        }
                    })
                });
            }
            TouchPhase::Move => {
                let d = DRAG.with(|d| {
                    let mut b = d.borrow_mut();
                    let d = b.as_mut()?;
                    d.at = (x, y);
                    if !d.begun && (x - d.start.0).hypot(y - d.start.1) > 3.0 {
                        d.begun = true;
                    }
                    if d.begun {
                        d.target = hit(d, x, y);
                    }
                    Some(d.clone())
                });
                show_drag(d.as_ref());
            }
            TouchPhase::End => {
                let d = DRAG.with(|d| d.borrow_mut().take());
                show_drag(None);
                if let Some(t) = d.as_ref().filter(|d| d.begun).and_then(|d| d.target.clone()) {
                    let d = d.unwrap();
                    cmd::move_items(&d.ids, &t.parent, t.after.as_deref(), COPY.with(std::cell::Cell::get));
                }
            }
            TouchPhase::Cancel => {
                DRAG.with(|d| *d.borrow_mut() = None);
                show_drag(None);
            }
        }
    }
}

/// The drop mark (`.o-drop-line`): an accent line at the top of the row (before it) or its bottom (after it;
/// one indent in for inside it), from the text's start, a ring at its left end.
fn drop_line(mode: DropMode, depth: u16) -> impl IntoElement {
    let (h, ring, border) = (num("space.step2"), num("space.step6"), num("space.step2"));
    let left = style::text_left(depth) + if mode == DropMode::Inside { style::indent() } else { 0.0 };
    let mut line = view()
        .test_id(match mode {
            DropMode::Before => "drop-before",
            DropMode::After => "drop-after",
            DropMode::Inside => "drop-inside",
        })
        .absolute()
        .left(px(left))
        .right(px(style::doc_pad().1))
        .h(px(h))
        .bg(color::accent())
        .rounded(px(num("space.step1")))
        .pointer_events(PointerEvents::None);
    line = if mode == DropMode::Before { line.top(px(-h / 2.0)) } else { line.bottom(px(-h / 2.0)) };
    line.child(
        view()
            .absolute()
            .left(px(-num("space.step4")))
            .top(px(-num("space.step3")))
            .w(px(ring))
            .h(px(ring))
            .rounded(px(ring / 2.0))
            .border(px(border))
            .border_color(color::accent())
            .bg(color::bg()),
    )
}

/// The label by the pointer while items are dragged (`.o-drag-label`): "Move 2 items", "Copy 1 item".
pub(crate) fn drag_label() -> impl IntoElement {
    dynamic(|| {
        let Some((_, (x, y), n, copy)) = shown().get().0 else { return view().into_element() };
        let what = if copy { "Copy" } else { "Move" };
        let label = format!("{what} {n} item{}", if n > 1 { "s" } else { "" });
        crate::sidebar::ui_text(label, style::ui_sized("type.ui_tiny"))
            .test_id("drag-label")
            .absolute()
            .left(px(x + num("space.step7")))
            .top(px(y + num("space.step5")))
            .bg(color::fg())
            .text_color(color::bg())
            .px(px(num("space.step6")))
            .py(px(num("space.step3")))
            .rounded(px(num("radius.code")))
            .pointer_events(PointerEvents::None)
            .into_element()
    })
}

/// Whether the caret (its top and bottom in the input, `None` when unknown) is on the input's first line (`down`
/// false) or last line (`down` true), as the web's `onFirstLine` / `onLastLine` tell: within 0.7 of a line from
/// that edge. Unknown places count as an edge, so the keys still move between rows.
fn on_edge(caret: Option<(f32, f32)>, height: f32, down: bool) -> bool {
    let Some((top, bottom)) = caret else { return true };
    if height <= 0.0 {
        return true;
    }
    let near = style::line() * 0.7;
    if down { height - bottom < near } else { top < near }
}
