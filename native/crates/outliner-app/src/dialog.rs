//! The web app's dialogs (`modal` in `apps/web/src/ui/overlay.ts`, `.overlay` / `.modal` in styles.css): a box over
//! a dimmed screen with its title, its body and its buttons, and the pieces dialogs are made of (`.btn`, `.field`,
//! the inputs). One dialog shows at a time; the search and replace and the export dialogs use it.

use std::rc::Rc;

use nsh_core::prelude::*;
use nsh_core::LineHeight;

use crate::sidebar::ui_text;
use crate::style::{self, color, num};

/// A dialog's button: its label, whether it is the primary one, and what it does (`None`: it only closes the
/// dialog). A button that returns `false` keeps the dialog open (the web's `run` returning false).
pub struct Button {
    pub label: &'static str,
    pub primary: bool,
    pub run: Option<Rc<dyn Fn() -> bool>>,
}

impl Button {
    pub fn close(label: &'static str) -> Button {
        Button { label, primary: false, run: None }
    }

    pub fn new(label: &'static str, primary: bool, run: impl Fn() -> bool + 'static) -> Button {
        Button { label, primary, run: Some(Rc::new(run)) }
    }
}

/// The dialog showing, made by its maker each time the screen is made again (a change of the look).
#[derive(Clone, Default)]
struct Shown(Option<Rc<dyn Fn() -> AnyElement>>);

impl PartialEq for Shown {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

fn shown() -> Signal<Shown> {
    global::<Shown>()
}

/// Show a dialog: `make` makes its box ([`modal`]).
pub fn show(make: impl Fn() -> AnyElement + 'static) {
    shown().set(Shown(Some(Rc::new(make))));
}

pub fn close() {
    shown().set(Shown(None));
}

/// The dialog over the screen, while one shows (`.overlay`): Escape closes it.
pub fn overlay() -> impl IntoElement {
    when(
        || shown().with(|s| s.0.is_some()),
        || {
            on_key("escape", |_| close());
            let make = shown().with_untracked(|s| s.0.clone());
            view()
                .test_id("dialog-overlay")
                .absolute()
                .top(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .items_center()
                .pt(|| px(window().height.0 * num("space.overlay_top")))
                .bg(rgba(style::look().color("color.overlay")))
                .child(make.map(|m| m()).unwrap_or_else(|| view().into_element()))
        },
    )
}

/// A dialog's box (`.modal`): its title (`h2`), its body, and its buttons in its foot.
pub fn modal(test_id: &str, title: impl Into<SharedString>, body: impl IntoElement, buttons: Vec<Button>) -> AnyElement {
    let title_size = num("type.modal_title");
    let foot = view()
        .flex_row()
        .justify_end()
        .gap(px(num("space.modal_foot_gap")))
        .px(px(num("space.modal_x")))
        .py(px(num("space.modal_foot_y")))
        .border_t(px(num("space.border")))
        .border_color(color::line())
        .children(buttons.into_iter().map(|b| {
            let run = b.run.clone();
            btn(b.label, b.primary, move || {
                if run.as_ref().is_none_or(|r| r()) {
                    close();
                }
            })
            .into_element()
        }));
    view()
        .test_id(test_id)
        .flex_col()
        .bg(color::bg())
        .rounded(px(num("radius.modal")))
        .overflow_hidden()
        .w(|| px(num("space.modal").min(window().width.0 * num("space.modal_vw"))))
        .max_h(|| px(window().height.0 * num("space.modal_vh")))
        .shadow([BoxShadow {
            blur: px(num("space.shadow_modal")),
            offset_y: px(num("space.step7")),
            color: rgba(style::look().color("color.shadow_modal")).into(),
            ..Default::default()
        }])
        .child(
            ui_text(title.into(), (title_size, title_size * num("type.ui_line")))
                .text_color(color::fg())
                .px(px(num("space.modal_x")))
                .py(px(num("space.modal_head_y")))
                .border_b(px(num("space.border")))
                .border_color(color::line()),
        )
        .child(
            scroll()
                .flex_shrink(1.0)
                .child(view().px(px(num("space.modal_x"))).py(px(num("space.modal_body_y"))).child(body)),
        )
        .child(foot)
        .into_element()
}

/// The app's text at the document's size (`.modal`, `.field` take `--fs`).
pub fn label(t: impl IntoReactive<SharedString>) -> Text {
    ui_text(t, (style::efs(), style::efs() * num("type.ui_line"))).text_color(color::fg())
}

/// A small muted text (`.replace-count`, `.hint`).
pub fn small(t: impl IntoReactive<SharedString>) -> Text {
    ui_text(t, style::ui_sized("type.ui_small")).text_color(color::muted())
}

/// A button (`.btn`; `.btn.primary`).
pub fn btn(label_text: &'static str, primary: bool, run: impl Fn() + 'static) -> impl IntoElement {
    let (bg, fg, border) = if primary {
        (color::accent(), rgba(style::look().color("color.on_accent")), color::accent())
    } else {
        (color::bg(), color::fg(), color::chip_border())
    };
    nsh_base::button()
        .accessibility_label(Some(label_text.into()))
        .px(px(num("space.btn_x")))
        .py(px(num("space.btn_y")))
        .bg(bg)
        .border(px(num("space.border")))
        .border_color(border)
        .rounded(px(num("radius.block")))
        .items_center()
        .on_press(run)
        .child(label(label_text).text_color(fg))
}

/// A field: its label over its input (`.field`).
pub fn field(name: &'static str, input: impl IntoElement) -> impl IntoElement {
    view().flex_col().gap(px(num("space.field_gap"))).mb(px(num("space.field_bottom"))).child(label(name)).child(input)
}

/// A checkbox before its label (`.field.row` with an `input[type=checkbox]`), pressed as a whole.
pub fn check(name: &'static str, on: impl Fn() -> bool + Copy + 'static, flip: impl Fn() + 'static) -> impl IntoElement {
    let size = num("space.check");
    nsh_base::button()
        .accessibility_label(Some(name.into()))
        .selected(on)
        .flex_row()
        .items_center()
        .gap(px(num("space.field_row_gap")))
        .mb(px(num("space.field_bottom")))
        .on_press(flip)
        .child(
            view()
                .w(px(size))
                .h(px(size))
                .rounded(px(num("radius.check")))
                .border(px(num("space.check_border")))
                .border_color(move || if on() { color::accent() } else { color::check() })
                .bg(move || if on() { color::accent() } else { color::bg() }),
        )
        .child(label(name))
}

/// A dialog's input (`.field input`): the document's text size in a bordered box.
pub fn input(test_id: &str, placeholder: &'static str) -> nsh_base::Input {
    let size = style::efs();
    nsh_base::input()
        .test_id(test_id)
        .placeholder(placeholder)
        .font_family(style::font())
        .text_size(px(size))
        .line_height(LineHeight::Points(px(size * num("type.ui_line"))))
        .text_color(color::fg())
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.block")))
        .px(px(num("space.input_x")))
        .py(px(num("space.input_y")))
        .auto_correct(false)
}
