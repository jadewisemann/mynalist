//! The web's popup menu (`showMenu` in `apps/web/src/ui/overlay.ts`): a menu of actions at a point (`.menu`,
//! `.mi`, `.sep`), kept on the screen, closed by a press outside it or Escape. Dialogs are `dialog.rs`'s.

use std::rc::Rc;

use nsh_core::keymap::When;
use nsh_core::prelude::*;

use crate::sidebar::ui_text;
use crate::style::{self, color, num};

/// An entry of a menu: an action, or a separator.
#[derive(Clone)]
pub struct Item {
    pub label: String,
    run: Option<Rc<dyn Fn()>>,
}

/// An action (`MenuItem` with `run`).
pub fn item(label: impl Into<String>, run: impl Fn() + 'static) -> Item {
    Item { label: label.into(), run: Some(Rc::new(run)) }
}

/// A separator (`sep`).
pub fn sep() -> Item {
    Item { label: String::new(), run: None }
}

/// The menu showing, at a point.
#[derive(Clone, Default)]
struct Over {
    /// Bumped by every change (the entries' actions cannot be compared).
    rev: u64,
    menu: Option<(f32, f32, Rc<Vec<Item>>)>,
}

impl PartialEq for Over {
    fn eq(&self, o: &Self) -> bool {
        self.rev == o.rev
    }
}

fn over() -> Signal<Over> {
    global::<Over>()
}

/// The height of a menu of `items` (`.menu`: its padding and border, an item a line and its padding high, a
/// separator its line and margins), to keep it on the screen as `place` does.
fn menu_height(items: &[Item]) -> f32 {
    let (_, line) = style::ui();
    let item_h = line + 2.0 * num("space.menu_item_y");
    let sep_h = num("space.step1") + 2.0 * num("space.step4");
    let body: f32 = items.iter().map(|i| if i.run.is_some() { item_h } else { sep_h }).sum();
    body + 2.0 * num("space.menu_y") + 2.0 * num("space.border")
}

/// Show a menu at (`x`, `y`) in the window (`showMenu`), kept `space.step4` inside its edges.
pub fn show(x: f32, y: f32, items: Vec<Item>) {
    let margin = num("space.step4");
    let win = window();
    let w = num("space.menu_min");
    let h = menu_height(&items);
    let x = x.min(win.width.0 - w - margin).max(margin);
    let y = y.min(win.height.0 - h - margin).max(margin);
    over().update(|o| {
        o.rev += 1;
        o.menu = Some((x, y, Rc::new(items)));
    });
}

/// Close the menu (`closeMenu`).
pub fn close() {
    over().update(|o| {
        if o.menu.take().is_some() {
            o.rev += 1;
        }
    });
}

/// Whether a menu shows (tracked).
pub fn is_open() -> bool {
    over().with(|o| o.menu.is_some())
}

/// The menu over everything else, while it shows (its box covers the window).
pub fn layer() -> impl IntoElement {
    dynamic(|| {
        let menu = over().with(|o| o.menu.clone());
        match menu {
            Some((x, y, items)) => menu_view(x, y, items).into_element(),
            None => view().into_element(),
        }
    })
}

/// The whole window (an overlay's box).
fn cover() -> View {
    view().absolute().top(px(0.0)).left(px(0.0)).w(|| px(window().width.0)).h(|| px(window().height.0))
}

fn menu_view(x: f32, y: f32, items: Rc<Vec<Item>>) -> impl IntoElement {
    on_key_with("escape", KeyOptions { when: When::Always, ..Default::default() }, |_| close());
    let (size, line) = style::ui();
    let hover = color::hover();
    let rows = items.iter().map(|it| match &it.run {
        Some(run) => {
            let run = run.clone();
            view()
                .test_id("menu-item")
                .px(px(num("space.menu_item_x")))
                .py(px(num("space.menu_item_y")))
                .hover(move |s| s.bg(hover))
                .on_press(move || {
                    close();
                    run();
                })
                .child(ui_text(it.label.clone(), (size, line)).text_color(color::fg()))
                .into_element()
        }
        None => view()
            .test_id("menu-sep")
            .h(px(num("space.step1")))
            .my(px(num("space.step4")))
            .bg(color::line())
            .into_element(),
    });
    cover()
        .child(cover().test_id("menu-scrim").on_press(close))
        .child(
            view()
                .test_id("popup")
                .absolute()
                .left(px(x))
                .top(px(y))
                .min_w(px(num("space.menu_min")))
                .py(px(num("space.menu_y")))
                .bg(color::bg())
                .border(px(num("space.border")))
                .border_color(color::chip_border())
                .rounded(px(num("radius.menu")))
                .shadow([BoxShadow {
                    offset_y: px(num("space.step5")),
                    blur: px(num("space.step11")),
                    color: rgba(style::look().color("color.shadow_menu")).into(),
                    ..Default::default()
                }])
                .children(rows),
        )
}
