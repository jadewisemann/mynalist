//! The outline view: a virtualized list of rows. A row shows its text as formatted spans; the row being edited
//! shows a native input instead (an EditText on Android, GPUI's text input on macOS), so the platform's input
//! methods, Korean composition included, work as in any text field. While it has the focus, the outliner's keys
//! are bound with `When::Always` so they reach the outliner before the field.

use nsh_base::{button, input, theme};
use nsh_core::element::Position;
use nsh_core::keymap::When;
use nsh_core::prelude::*;
use outliner_core::item;

use crate::commands as cmd;
use crate::inline::{self, Span};
use crate::store::{self, Row};

const INDENT: f32 = 21.0;
const ROW: f32 = 28.0;

pub fn outline() -> impl IntoElement {
    virtual_list(|| store::read(|s| s.rows.len()), row)
        .keys(|| store::read(|s| s.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>()))
        .estimated_item_size(px(ROW))
        .test_id("outline")
        .flex_1()
}

/// A cell, reused for other rows as the list scrolls: its bindings read the row where it is now.
fn row(at: Position) -> impl IntoElement {
    let row = computed(move || store::read(|s| s.rows.get(at.index()).cloned()));
    let editing = computed(move || {
        let id = row.get().map(|r| r.id);
        store::read(|s| s.editing.as_ref().map(|c| c.id.clone())) == id && id.is_some()
    });
    view()
        .flex_row()
        .items_start()
        .min_h(px(ROW))
        .pl(move || px(12.0 + INDENT * f32::from(row.get().map_or(0, |r| r.depth))))
        .pr(px(12.0))
        .child(bullet(row))
        .child(
            view().flex_1().py(px(4.0)).child(
                when(move || editing.get(), move || row_input(row.get_untracked()))
                    .otherwise(move || row_text(row)),
            ),
        )
}

/// The bullet: a dot, ringed while the row's children are folded. A press folds or unfolds.
fn bullet(row: Memo<Option<Row>>) -> impl IntoElement {
    let folded = move || row.get().is_some_and(|r| r.has_kids && r.collapsed);
    button()
        .w(px(INDENT))
        .h(px(ROW))
        .items_center()
        .justify_center()
        .on_press(move || {
            if let Some(r) = row.get_untracked() {
                cmd::toggle(&r.id);
            }
        })
        .child(
            view()
                .w(px(6.0))
                .h(px(6.0))
                .rounded(px(3.0))
                .bg(move || theme().foreground.opacity(0.75))
                .border(move || px(if folded() { 4.0 } else { 0.0 }))
                .border_color(move || theme().muted),
        )
}

/// The row's text as formatted spans; a press starts editing it.
fn row_text(row: Memo<Option<Row>>) -> impl IntoElement {
    let content = computed(move || {
        let Some(r) = row.get() else { return (String::new(), false) };
        store::read(|s| {
            let n = s.tree().get(&r.id);
            (item::content(n).to_owned(), item::is_checked(n))
        })
    });
    let spans = move || {
        let (src, _) = content.get();
        inline::parse(&src).into_iter().map(span).collect::<Vec<_>>()
    };
    text("")
        .text_color(move || theme().foreground)
        .opacity(move || if content.get().1 { 0.45_f32 } else { 1.0 })
        .child(dynamic(spans))
        .on_press(move || {
            if let Some(r) = row.get_untracked() {
                cmd::start_editing(&r.id, None);
            }
        })
}

fn span(s: Span) -> Text {
    match s {
        Span::Plain(t) => text(t),
        Span::Bold(t) => text(t).font_weight(FontWeight::BOLD),
        Span::Italic(t) => text(t).italic(),
        Span::Code(t) => text(t)
            .font_family("ui-monospace")
            .bg(move || theme().muted)
            .rounded(px(3.0)),
        Span::Link { text: t, .. } => text(t).text_color(move || theme().primary).underline(),
        Span::Tag(t) => text(t).text_color(move || theme().muted_foreground),
    }
}

/// The input of the row being edited. Made when editing moves to the row, gone when it moves away; the keys
/// bound here live as long as it does.
fn row_input(row: Option<Row>) -> impl IntoElement {
    let Some(row) = row else { return view().into_element() };
    let id = row.id.clone();
    let (initial, caret) = store::peek(|s| {
        let at = s.editing.as_ref().map_or(0, |c| c.at);
        (store::content_of(s, &id), at)
    });
    let value = signal(initial);
    let sel = signal(caret);
    let sel_end = signal(caret);

    let bind = |keys: &str, f: fn(&str, u32)| {
        let id = id.clone();
        on_key_with(keys, KeyOptions { when: When::Always, repeat: true, ..Default::default() }, move |_| {
            f(&id, sel.get_untracked());
        });
    };
    bind("up", |id, _| cmd::step(id, false));
    bind("down", |id, _| cmd::step(id, true));
    bind("tab", cmd::indent);
    bind("shift+tab", cmd::outdent);
    bind("mod+enter", |id, _| cmd::toggle_checked(id));
    bind("mod+z", |_, _| cmd::undo(false));
    bind("shift+mod+z", |_, _| cmd::undo(true));
    bind("mod+.", |id, _| cmd::toggle(id));

    let (change_id, submit_id, key_id) = (id.clone(), id.clone(), id.clone());
    input()
        .value(move || value.get())
        .test_id("row-input")
        .auto_focus(true)
        .selected_range((caret, caret))
        .submit_behavior(SubmitBehavior::Submit)
        .auto_correct(false)
        .p_0()
        .border(px(0.0))
        .bg(AdaptiveColor::TRANSPARENT)
        .text_color(move || theme().foreground)
        .on_change(move |t: &str| {
            value.set(t.to_owned());
            cmd::set_text(&change_id, t, sel.get_untracked());
        })
        .on_selection_change(move |s: &Selection| {
            sel.set(s.start);
            sel_end.set(s.end);
        })
        .on_submit(move |_: &str| cmd::split(&submit_id, sel.get_untracked()))
        .on_key_press(move |k: &str| {
            if k == "Backspace" && sel.get_untracked() == 0 && sel_end.get_untracked() == 0 {
                cmd::merge_up(&key_id);
            }
        })
        .into_element()
}
