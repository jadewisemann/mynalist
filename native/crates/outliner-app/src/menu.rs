//! The web's popup menu (`showMenu` in `apps/web/src/ui/overlay.ts`): a menu of actions at a point (`.menu`,
//! `.mi`, `.sep`), kept on the screen, closed by a press outside it or Escape. Dialogs are `dialog.rs`'s.

use std::rc::Rc;

use nsh_core::keymap::When;
use nsh_core::prelude::*;

use crate::sidebar::ui_text;
use crate::style::{self, color, num};

/// An entry of a menu: an action (with its key, or a submenu), a row of its own (`custom`), or a separator.
#[derive(Clone)]
pub struct Item {
    pub label: String,
    run: Option<Rc<dyn Fn()>>,
    /// The key that runs the same command, shown at the right (`.k`).
    key: Option<String>,
    /// A submenu, opened by a press (`sub`).
    sub: Option<Rc<Vec<Item>>>,
    /// A row of its own (`custom`) and its height.
    custom: Option<(Rc<dyn Fn() -> AnyElement>, f32)>,
}

/// An action (`MenuItem` with `run`).
pub fn item(label: impl Into<String>, run: impl Fn() + 'static) -> Item {
    Item { label: label.into(), run: Some(Rc::new(run)), key: None, sub: None, custom: None }
}

/// A separator (`sep`).
pub fn sep() -> Item {
    Item { label: String::new(), run: None, key: None, sub: None, custom: None }
}

/// An entry opening a submenu (`sub`).
pub fn sub(label: impl Into<String>, items: Vec<Item>) -> Item {
    Item { label: label.into(), run: None, key: None, sub: Some(Rc::new(items)), custom: None }
}

/// A row of its own (`custom`), `height` high: `make` makes it.
pub fn custom(label: impl Into<String>, height: f32, make: impl Fn() -> AnyElement + 'static) -> Item {
    Item { label: label.into(), run: None, key: None, sub: None, custom: Some((Rc::new(make), height)) }
}

impl Item {
    /// The same action with its key shown (`key`).
    pub fn key(self, key: Option<String>) -> Item {
        Item { key, ..self }
    }

    fn is_sep(&self) -> bool {
        self.run.is_none() && self.sub.is_none() && self.custom.is_none()
    }
}

/// The menu showing, at a point.
#[derive(Clone, Default)]
struct Over {
    /// Bumped by every change (the entries' actions cannot be compared).
    rev: u64,
    menu: Option<(f32, f32, Rc<Vec<Item>>)>,
    /// The submenu open beside it.
    sub: Option<(f32, f32, Rc<Vec<Item>>)>,
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
    let body: f32 = items
        .iter()
        .map(|i| match &i.custom {
            Some((_, h)) => *h,
            None if i.is_sep() => sep_h,
            None => item_h,
        })
        .sum();
    body + 2.0 * num("space.menu_y") + 2.0 * num("space.border")
}

/// Show a menu at (`x`, `y`) in the window (`showMenu`), kept `space.step4` inside its edges.
pub fn show(x: f32, y: f32, items: Vec<Item>) {
    let (x, y) = place(x, y, &items);
    over().update(|o| {
        o.rev += 1;
        o.menu = Some((x, y, Rc::new(items)));
        o.sub = None;
    });
}

/// Where a menu of `items` asked for at (`x`, `y`) goes: kept `space.step4` inside the window's edges.
fn place(x: f32, y: f32, items: &[Item]) -> (f32, f32) {
    let margin = num("space.step4");
    let win = window();
    let w = num("space.menu_min");
    let h = menu_height(items);
    (x.min(win.width.0 - w - margin).max(margin), y.min(win.height.0 - h - margin).max(margin))
}

/// Open submenu `items` beside its entry, which is at `y` in a menu at `x` (`openSub`): at the right of the menu,
/// or at its left when there is no room.
fn open_sub(x: f32, y: f32, items: Rc<Vec<Item>>) {
    let w = num("space.menu_min");
    let inset = num("space.step4");
    let right = x + w - inset;
    let sx = if right + w + inset > window().width.0 { (x - w + inset).max(inset) } else { right };
    let (sx, sy) = place(sx, y - num("space.menu_y"), &items);
    over().update(|o| {
        o.rev += 1;
        o.sub = Some((sx, sy, items));
    });
}

/// Close the menu (`closeMenu`).
pub fn close() {
    over().update(|o| {
        if o.menu.take().is_some() {
            o.rev += 1;
        }
        o.sub = None;
    });
}

/// Whether a menu shows (tracked).
pub fn is_open() -> bool {
    over().with(|o| o.menu.is_some())
}

/// The menu over everything else, while it shows (its box covers the window), and its submenu.
pub fn layer() -> impl IntoElement {
    dynamic(|| {
        let (menu, sub) = over().with(|o| (o.menu.clone(), o.sub.clone()));
        match menu {
            Some((x, y, items)) => cover()
                .child(cover().test_id("menu-scrim").on_press(close))
                .child(menu_view(x, y, items, "popup"))
                .children(sub.map(|(sx, sy, items)| menu_view(sx, sy, items, "submenu")))
                .into_element(),
            None => view().into_element(),
        }
    })
}

/// The whole window (an overlay's box).
fn cover() -> View {
    view().absolute().top(px(0.0)).left(px(0.0)).w(|| px(window().width.0)).h(|| px(window().height.0))
}

fn menu_view(x: f32, y: f32, items: Rc<Vec<Item>>, test_id: &str) -> impl IntoElement {
    on_key_with("escape", KeyOptions { when: When::Always, ..Default::default() }, |_| close());
    let (size, line) = style::ui();
    let hover = color::hover();
    let item_h = line + 2.0 * num("space.menu_item_y");
    let mut top = y + num("space.menu_y") + num("space.border");
    let rows: Vec<AnyElement> = items
        .iter()
        .map(|it| {
            let at = top;
            if let Some((make, h)) = &it.custom {
                top += h;
                return view().test_id("menu-custom").h(px(*h)).child(make()).into_element();
            }
            if it.is_sep() {
                top += num("space.step1") + 2.0 * num("space.step4");
                return view()
                    .test_id("menu-sep")
                    .h(px(num("space.step1")))
                    .my(px(num("space.step4")))
                    .bg(color::line())
                    .into_element();
            }
            top += item_h;
            let (run, sub) = (it.run.clone(), it.sub.clone());
            // `.k`: the key at the right, muted and small; `▸` for a submenu
            let hint = it.key.clone().or_else(|| sub.is_some().then(|| "▸".to_owned()));
            view()
                .test_id("menu-item")
                .flex_row()
                .justify_between()
                .items_center()
                .gap(px(num("space.step10")))
                .px(px(num("space.menu_item_x")))
                .py(px(num("space.menu_item_y")))
                .hover(move |s| s.bg(hover))
                .on_press(move || {
                    if let Some(sub) = &sub {
                        open_sub(x, at, sub.clone());
                        return;
                    }
                    close();
                    if let Some(run) = &run {
                        run();
                    }
                })
                .child(ui_text(it.label.clone(), (size, line)).text_color(color::fg()))
                .children(hint.map(|k| ui_text(k, style::ui_sized("type.ui_tiny")).test_id("menu-key").text_color(color::muted())))
                .into_element()
        })
        .collect();
    // a menu taller than the window scrolls (kept `space.step4` inside its edges)
    let room = window().height.0 - 2.0 * num("space.step4") - 2.0 * (num("space.menu_y") + num("space.border"));
    view()
        .test_id(test_id)
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
        .child(scroll().max_h(px(room)).child(view().children(rows)))
}
