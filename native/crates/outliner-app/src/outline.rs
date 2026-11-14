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
use crate::inline::{self, Span};
use crate::store::{self, Row};
use crate::style::{self, color, BODY_GAP, BULLET_W, DOT, EFS, FONT, LINE, MONO, PAD};

/// How an item is drawn, from its props.
#[derive(Debug, Clone, PartialEq, Default)]
struct Look {
    heading: i64,
    checked: bool,
    checkbox: bool,
    label: i64,
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
        }
    })
}

pub fn outline() -> impl IntoElement {
    let (_, right) = style::doc_pad();
    virtual_list(|| store::read(|s| s.rows.len()), row)
        .keys(|| store::read(|s| s.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>()))
        .estimated_item_size(px(LINE + 2.0 * PAD))
        .header(title())
        .test_id("outline")
        .flex_1()
        .pr(px(right))
        .bg(color::bg())
}

/// The zoom root's text above its items (`.o-title`): 28px, the document's name at the top level.
fn title() -> impl IntoElement {
    let phone = style::phone();
    text(move || {
        store::read(|s| {
            let t = store::content_of(s, &s.view.zoom);
            if t.is_empty() { "Untitled".to_owned() } else { t }
        })
    })
    .font_family(FONT)
    .text_size(px(EFS * 1.5556))
    .line_height(LineHeight::Points(px(EFS * 1.5556 * 1.2143)))
    .text_color(color::fg())
    .pl(px(style::text_left(0) - 26.0))
    .pt(px(if phone { 12.0 } else { 18.0 }) + px(2.0))
    .pb(px(12.0))
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
        .flex_row()
        .items_start()
        .py(px(PAD))
        .pl(move || px(style::text_left(depth()) - BODY_GAP - BULLET_W))
        // the guides of the levels above: one line per ancestor, under its bullet (`.o-kids` border)
        .child(dynamic(move || {
            (0..depth())
                .map(|k| {
                    view()
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
            view().flex_1().pl(px(BODY_GAP)).child(dynamic(move || match editing.get() {
                Some(_) => row_input(row.get_untracked()).into_element(),
                None => row_text(row, look).into_element(),
            })),
        )
}

/// The bullet (`.o-bullet`): a 6px dot in its 18px column, as tall as the text's line; ringed while its
/// children are folded. A press folds or unfolds.
fn bullet(row: Memo<Option<Row>>, look: Memo<Look>) -> impl IntoElement {
    let folded = move || row.get().is_some_and(|r| r.has_kids && r.collapsed);
    button()
        .w(px(BULLET_W))
        .h(move || px(look.get().type_size().1))
        .items_center()
        .justify_center()
        .on_press(move || {
            if let Some(r) = row.get_untracked() {
                cmd::toggle_collapse(&r.id, false);
            }
        })
        .child(
            view()
                .w(move || px(if folded() { DOT + 8.0 } else { DOT }))
                .h(move || px(if folded() { DOT + 8.0 } else { DOT }))
                .rounded(move || px(if folded() { (DOT + 8.0) / 2.0 } else { DOT / 2.0 }))
                .bg(color::muted())
                .border(move || px(if folded() { 4.0 } else { 0.0 }))
                .border_color(color::faint()),
        )
}

/// The checkbox of an item that shows one (`.o-check`): 16px, centered on the first line; a press checks it.
fn checkbox(row: Memo<Option<Row>>, look: Memo<Look>) -> impl IntoElement {
    let on = move || look.get().checked;
    button()
        .w(px(16.0))
        .h(px(16.0))
        .mt(move || px((look.get().type_size().1 - 16.0) / 2.0))
        .ml(px(2.0))
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
        .child(when(on, || text("✓").text_size(px(12.0)).text_color(color::bg())))
}

/// The row's text as formatted spans (`.o-content`); a press starts editing it.
fn row_text(row: Memo<Option<Row>>, look: Memo<Look>) -> impl IntoElement {
    let content = computed(move || row.get().map(|r| store::read(|s| store::content_of(s, &r.id))).unwrap_or_default());
    let spans = move || inline::parse(&content.get()).into_iter().map(span).collect::<Vec<_>>();
    // the item's look sets static styles: the text is made again when it changes (rarely: a heading, a check)
    dynamic(move || {
        let l = look.get();
        let (size, line) = l.type_size();
        let mut t = text("")
            // as wide as its text, as `.o-content` is (a color label covers the text only)
            .self_start()
            .font_family(FONT)
            .text_size(px(size))
            .line_height(LineHeight::Points(px(line)))
            .text_color(if l.checked { color::check() } else { color::fg() })
            .child(dynamic(spans))
            .on_press(move || {
                if let Some(r) = row.get_untracked() {
                    cmd::start_editing(&r.id, None);
                }
            });
        if l.heading > 0 {
            t = t.font_weight(FontWeight::BOLD);
        }
        // checked items are struck through (`.o-checked`)
        if l.checked {
            t = t.line_through();
        }
        // a color label is a rounded background behind the text (`.o-cl1` .. `.o-cl6`)
        if let Some(bg) = color::label(l.label) {
            t = t.bg(bg).rounded(px(3.0));
        }
        t.into_element()
    })
}

fn span(s: Span) -> Text {
    match s {
        Span::Plain(t) => text(t),
        Span::Bold(t) => text(t).font_weight(FontWeight::BOLD),
        Span::Italic(t) => text(t).italic(),
        // `.o-code`: Fira Mono at 8/9 of the text, red on gray
        // `.o-code`: Fira Mono at 8/9 of the text, red on a gray box. The web pads the box 5px each side; a span
        // here has no padding (narrow spaces would change the text that copies and screen readers get)
        Span::Code(t) => text(t)
            .font_family(MONO)
            .text_size(px(EFS * 0.8889))
            .text_color(color::code_fg())
            .bg(color::code_bg())
            .rounded(px(4.0)),
        // `.o-link` keeps the text's color, underlined; `.o-tag` is gray, underlined
        Span::Link { text: t, .. } => text(t).underline(),
        Span::Tag(t) => text(t).text_color(color::tag()).underline(),
    }
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
