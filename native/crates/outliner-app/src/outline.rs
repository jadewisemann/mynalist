//! The outline view: a virtualized list of rows. A row shows its text as formatted spans; the row being edited
//! shows a native input instead (an EditText on Android, GPUI's text input on macOS), so the platform's input
//! methods, Korean composition included, work as in any text field. While it has the focus, the outliner's keys
//! are bound with `When::Always` so they reach the outliner before the field.

use nsh_base::{button, input};
use nsh_core::element::Position;
use nsh_core::LineHeight;
use nsh_core::keymap::When;
use nsh_core::prelude::*;
use outliner_core::item;

use crate::commands as cmd;
use crate::inline::{self, Block};
use crate::store::{self, Row};
use crate::style::{self, color, BODY_GAP, BULLET_W, DOT, EFS, FONT, LINE, PAD};

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
        match self.heading {
            1 => (EFS * 1.3333, EFS * 1.5556),
            2 => (EFS * 1.2222, EFS * 1.4444),
            3 => (EFS * 1.1111, EFS * 1.3333),
            _ => (EFS, LINE),
        }
    }
}

fn look_of(id: &str) -> Look {
    store::read(|s| {
        let n = s.tree().get(id);
        Look {
            heading: item::heading(n),
            checked: item::is_checked(n),
            checkbox: item::has_checkbox(n),
            label: item::color(n),
            note: Some(item::note(n)).filter(|t| !t.trim().is_empty()).map(str::to_owned),
        }
    })
}

pub fn outline() -> impl IntoElement {
    virtual_list(|| store::read(|s| s.rows.len()), row)
        .keys(|| store::read(|s| s.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>()))
        .estimated_item_size(px(LINE + 2.0 * PAD))
        .header(title())
        .test_id("outline")
        .flex_1()
        .bg(color::bg())
}

/// The zoom root's text above its items (`.o-title`): 28px, the document's name at the top level, formatted as
/// rows are; its note under it.
fn title() -> impl IntoElement {
    let phone = style::phone();
    let size = EFS * 1.5556;
    let content = computed(|| store::read(|s| store::content_of(s, &s.view.zoom)));
    let note = computed(|| {
        store::read(|s| Some(item::note(s.tree().get(&s.view.zoom)).to_owned()).filter(|t| !t.trim().is_empty()))
    });
    view()
        .test_id("title")
        .pl(px(style::text_left(0) - 26.0))
        .pr(px(style::doc_pad().1))
        .pt(px(if phone { 12.0 } else { 18.0 }) + px(2.0))
        .pb(px(12.0))
        .child(dynamic(move || {
            let src = content.get();
            if src.is_empty() {
                // `.o-title .o-content:empty::before`
                return text("Untitled")
                    .font_family(FONT)
                    .text_size(px(size))
                    .line_height(LineHeight::Points(px(size * 1.2143)))
                    .text_color(color::faint())
                    .into_element();
            }
            let look = inline::Look::new(size, store::now());
            view()
                .children(inline::blocks(&src).into_iter().map(|b| match b {
                    Block::Inline(tokens) => text("")
                        .font_family(FONT)
                        .text_size(px(size))
                        .line_height(LineHeight::Points(px(size * 1.2143)))
                        .text_color(color::fg())
                        .children(inline::spans(&tokens, &look))
                        .into_element(),
                    Block::Code { text, .. } => inline::code_block(&text, &look).into_element(),
                }))
                .into_element()
        }))
        .child(dynamic(move || match note.get() {
            Some(n) => note_text(&n, false).mt(px(2.0)).into_element(),
            None => view().into_element(),
        }))
}

/// A note (`.o-note`): 5/6 of the text in the note color, on 1.2 lines; formatted as the content is.
fn note_text(src: &str, checked: bool) -> View {
    let size = EFS * 0.8333;
    let fg = if checked { color::check() } else { color::note() };
    let look = inline::Look::new(size, store::now()).colored(fg, checked);
    view().test_id("note").children(inline::blocks(src).into_iter().map(move |b| match b {
        Block::Inline(tokens) => {
            let mut t = text("")
                .font_family(FONT)
                .text_size(px(size))
                .line_height(LineHeight::Points(px(size * 1.2)))
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
fn row(at: Position) -> impl IntoElement {
    let row = computed(move || store::read(|s| s.rows.get(at.index()).cloned()));
    let look = computed(move || row.get().map(|r| look_of(&r.id)).unwrap_or_default());
    let depth = move || row.get().map_or(0, |r| r.depth);
    // the id of this row while it is the one being edited: the input is made for that item, and made again
    // when the cell is reused for another row that is being edited too (after a zoom, say)
    let editing = computed(move || {
        let id = row.get().map(|r| r.id)?;
        store::read(|s| s.editing.as_ref().is_some_and(|c| c.id == id)).then_some(id)
    });
    view()
        .test_id("row")
        .flex_row()
        .items_start()
        .py(px(PAD))
        // the document's right padding: a list's padding does not reach its cells
        .pr(px(style::doc_pad().1))
        .pl(move || px(style::text_left(depth()) - BODY_GAP - bullet_w(row.get().map_or(0, |r| r.num))))
        // the guides of the levels above: one line per ancestor, under its bullet (`.o-kids` border)
        .child(dynamic(move || {
            (0..depth())
                .map(|k| {
                    view()
                        .test_id("guide")
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(style::bullet_center(k)))
                        .w(px(1.0))
                        .bg(color::guide())
                })
                .collect::<Vec<_>>()
        }))
        .child(bullet(row, look))
        .child(when(move || look.get().checkbox, move || checkbox(row, look)))
        .child(
            view()
                .flex_1()
                .pl(px(BODY_GAP))
                .child(dynamic(move || match editing.get() {
                    Some(_) => row_input(row.get_untracked()).into_element(),
                    None => row_text(row, look).into_element(),
                }))
                // the note under the content (`.o-note`), while it has text
                .child(dynamic(move || {
                    let l = look.get();
                    match &l.note {
                        Some(n) => note_text(n, l.checked).into_element(),
                        None => view().into_element(),
                    }
                })),
        )
}

/// The bullet column's width: 18px, or 28px for a number (`.o-numbered > .o-row > .o-bullet`).
fn bullet_w(num: u32) -> f32 {
    if num > 0 { BULLET_W + 10.0 } else { BULLET_W }
}

/// The bullet (`.o-bullet`): a 6px dot in its 18px column, as tall as the text's line; ringed while its
/// children are folded. Under a numbered parent, the row's number instead (`1.`), right-aligned 6px from the
/// text, in figures of one width, at .85em. A press folds or unfolds.
fn bullet(row: Memo<Option<Row>>, look: Memo<Look>) -> impl IntoElement {
    let folded = move || row.get().is_some_and(|r| r.has_kids && r.collapsed);
    let num = move || row.get().map_or(0, |r| r.num);
    button()
        .w(move || px(bullet_w(num())))
        .h(move || px(look.get().type_size().1))
        .items_center()
        .justify_center()
        .on_press(move || {
            if let Some(r) = row.get_untracked() {
                cmd::toggle_collapse(&r.id, false);
            }
        })
        .child(dynamic(move || {
            let n = num();
            if n > 0 {
                return view()
                    .test_id("bullet")
                    .self_stretch()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .pr(px(6.0))
                    .child(
                        text(format!("{n}."))
                            .font_family(FONT)
                            .text_size(px(EFS * 0.85))
                            .text_color(color::muted())
                            .tabular_nums(),
                    )
                    .into_element();
            }
            view()
                .test_id("bullet")
                .w(move || px(if folded() { DOT + 8.0 } else { DOT }))
                .h(move || px(if folded() { DOT + 8.0 } else { DOT }))
                .rounded(move || px(if folded() { (DOT + 8.0) / 2.0 } else { DOT / 2.0 }))
                .bg(color::muted())
                .border(move || px(if folded() { 4.0 } else { 0.0 }))
                .border_color(color::faint())
                .into_element()
        }))
}

/// The checkbox of an item that shows one (`.o-check`): 16px, centered on the first line; a press checks it.
fn checkbox(row: Memo<Option<Row>>, look: Memo<Look>) -> impl IntoElement {
    let on = move || look.get().checked;
    // the test id names the wrapper: a button takes none
    view().test_id("checkbox").mt(move || px((look.get().type_size().1 - 16.0) / 2.0)).ml(px(2.0)).child(
        button()
            .w(px(16.0))
            .h(px(16.0))
            .rounded(px(3.0))
            .border(px(1.5))
            .border_color(move || if on() { color::muted() } else { color::check() })
            .bg(move || if on() { color::muted() } else { color::bg() })
            .items_center()
            .justify_center()
            .on_press(move || {
                if let Some(r) = row.get_untracked() {
                    cmd::toggle_checked(&r.id, 0, true);
                }
            })
            .child(when(on, || text("✓").text_size(px(12.0)).text_color(color::bg()))),
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
        let shown = inline::Look::new(size, store::now()).colored(fg, l.checked);
        let edit = move || {
            if let Some(r) = row.get_untracked() {
                cmd::start_editing(&r.id, None);
            }
        };
        let paragraph = |tokens: &[outliner_core::inline::Token]| {
            let mut t = text("")
                .font_family(FONT)
                .text_size(px(size))
                .line_height(LineHeight::Points(px(line)))
                .text_color(fg)
                .children(inline::spans(tokens, &shown))
                .on_press(edit);
            if l.heading > 0 {
                t = t.font_weight(FontWeight::BOLD);
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
                t = t.bg(bg).rounded(px(3.0));
            }
            return t.into_element();
        }
        let mut v = view().test_id("row-text").on_press(edit);
        if let Some(bg) = color::label(l.label) {
            v = v.bg(bg).rounded(px(3.0));
        }
        v.children(blocks.drain(..).map(|b| match b {
            Block::Inline(tokens) => paragraph(&tokens).into_element(),
            Block::Code { text, .. } => inline::code_block(&text, &shown).into_element(),
        }))
        .into_element()
    })
}

/// The input of the row being edited. Made when editing moves to the row, gone when it moves away; the keys
/// bound here live as long as it does. Its text is the item's (a command may change it while it shows), and a
/// command that places the caret moves its selection there.
fn row_input(row: Option<Row>) -> impl IntoElement {
    let Some(row) = row else { return view().into_element() };
    let id = row.id.clone();
    let caret = store::peek(|s| s.editing.as_ref().map_or(0, |c| c.at));
    let sel = signal(caret);
    let sel_end = signal(caret);
    let field = element_ref();

    // a command (not typing) put the caret somewhere in this row: select it there
    {
        let id = id.clone();
        let seen = std::cell::Cell::new(store::peek(|s| s.caret_rev));
        effect(move || {
            let (rev, at) = store::read(|s| (s.caret_rev, s.editing.as_ref().filter(|c| c.id == id).map(|c| c.at)));
            if rev != seen.get() {
                seen.set(rev);
                if let Some(at) = at {
                    sel.set(at);
                    sel_end.set(at);
                    field.set_selection(at, at);
                }
            }
        });
    }

    let opts = KeyOptions { when: When::Always, repeat: true, ..Default::default() };
    let bind = |keys: &str, f: fn(&str, u32)| {
        let id = id.clone();
        on_key_with(keys, opts, move |_| {
            // only the row being edited acts (a spare cell out of view may still hold an input)
            if store::peek(|s| s.editing.as_ref().is_some_and(|c| c.id == id)) {
                f(&id, sel.get_untracked());
            }
        });
    };
    bind("up", |id, _| cmd::step(id, false));
    bind("down", |id, _| cmd::step(id, true));
    bind("tab", cmd::indent);
    bind("shift+tab", cmd::outdent);
    bind("mod+up", cmd::move_up);
    bind("mod+down", cmd::move_down);
    bind("shift+mod+backspace", cmd::delete_item);
    bind("mod+enter", |id, at| cmd::toggle_checked(id, at, false));
    bind("shift+mod+c", cmd::toggle_checkbox);
    bind("mod+.", |id, _| cmd::toggle_collapse(id, false));
    bind("shift+mod+.", |id, _| cmd::toggle_collapse(id, true));
    bind("mod+]", |id, _| cmd::zoom_in(id));
    bind("mod+[", |_, _| cmd::zoom_out());
    bind("mod+z", |_, _| cmd::undo(false));
    bind("shift+mod+z", |_, _| cmd::undo(true));
    bind("mod+y", |_, _| cmd::undo(true));

    let (value_id, change_id, submit_id, key_id) = (id.clone(), id.clone(), id.clone(), id.clone());
    input()
        .element_ref(&field)
        .value(move || store::read(|s| store::content_of(s, &value_id)))
        .test_id("row-input")
        .auto_focus(true)
        .selected_range((caret, caret))
        .submit_behavior(SubmitBehavior::Submit)
        .auto_correct(false)
        .p_0()
        .border(px(0.0))
        .bg(AdaptiveColor::TRANSPARENT)
        .font_family(FONT)
        .text_size(px(EFS))
        .line_height(LineHeight::Points(px(LINE)))
        .min_h(px(LINE))
        .text_color(color::fg())
        .on_change(move |t: &str| cmd::set_text(&change_id, t, sel.get_untracked()))
        .on_selection_change(move |s: &Selection| {
            sel.set(s.start);
            sel_end.set(s.end);
        })
        .on_submit(move |_: &str| cmd::enter(&submit_id, sel.get_untracked(), sel_end.get_untracked()))
        .on_key_press(move |k: &str| {
            if k == "Backspace" && sel.get_untracked() == 0 && sel_end.get_untracked() == 0 {
                cmd::backspace(&key_id);
            }
        })
        .into_element()
}
