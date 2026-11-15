//! The export dialog, as the web app's (`exportDialog` in `apps/web/src/ui/exporter.ts`): the document, the zoomed
//! item or the selected items as plain text (indented with spaces, asterisks, dashes or not at all), HTML or OPML,
//! all of it or only what the view shows (`export_visible_only`, kept on the device); the result can be copied,
//! and on desktops saved to a file. Phones only copy (NSH 0.5.0 has no share sheet).

use nsh_core::file_dialog::{FileFilter, SaveDialog};
use nsh_core::prelude::*;
use nsh_core::LineHeight;
use outliner_core::export::{
    export_html, export_opml, export_text, ExportScope, HtmlExportOptions, IndentStyle, OpmlExportOptions,
    TextExportOptions,
};
use outliner_core::inline::inline_to_plain;
use outliner_core::{item, NodeId, ROOT};

use crate::dialog::{self, Button};
use crate::style::{self, color, num};
use crate::{links, store, toast};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Format {
    #[default]
    Text,
    Html,
    Opml,
}

const FORMATS: [(Format, &str); 3] = [(Format::Text, "Plain text"), (Format::Html, "Formatted (HTML)"), (Format::Opml, "OPML")];
const INDENTS: [(IndentStyle, &str); 4] = [
    (IndentStyle::Spaces, "Indent with spaces"),
    (IndentStyle::Asterisks, "Asterisks"),
    (IndentStyle::Dashes, "Dashes"),
    (IndentStyle::None, "No indentation"),
];

/// The pref "Export visible items only" (the web's `export_visible_only`).
const VISIBLE_ONLY: &str = "export_visible_only";

#[derive(Clone, PartialEq, Default)]
struct Export {
    /// The items exported (the document's root: its items).
    ids: Vec<NodeId>,
    title: String,
    format: Format,
    indent: IndentStyle,
    visible_only: bool,
}

fn state() -> Signal<Export> {
    global::<Export>()
}

/// The menu's "Export…": the selected items, or the zoomed item (the document at the top).
pub fn open() {
    let ids = store::peek(|s| match &s.block {
        Some(b) => b.ids.clone(),
        None => vec![s.view.zoom.clone()],
    });
    open_items(ids);
}

/// Export items `ids` (the item menu's "Export…", `exportItems`).
pub fn open_items(ids: Vec<NodeId>) {
    let (ids, title) = store::peek(|s| {
        let title = match ids.as_slice() {
            [one] if one != ROOT => {
                let t = inline_to_plain(item::content(s.tree().get(one)), None);
                if t.is_empty() { s.title(&s.doc) } else { t }
            }
            _ => s.title(&s.doc),
        };
        (ids, title)
    });
    state().set(Export { ids, title, visible_only: crate::find::pref(VISIBLE_ONLY), ..Export::default() });
    dialog::show(make);
}

/// The text exported now.
fn output(e: &Export) -> String {
    store::read(|s| {
        let t = s.tree();
        let ids: Vec<NodeId> = match e.ids.as_slice() {
            [one] if one == ROOT => t.get(ROOT).map(|n| n.children.clone()).unwrap_or_default(),
            _ => e.ids.clone(),
        };
        let filter = s.view.filter.clone();
        let include = move |id: &str| filter.as_ref().is_none_or(|f| f.contains(id));
        let scope = ExportScope {
            visible_only: e.visible_only,
            show_checked: Some(s.view.show_checked),
            notes: Default::default(),
            include: if e.visible_only && s.view.filter.is_some() { Some(&include) } else { None },
        };
        match e.format {
            Format::Text => export_text(t, &ids, &scope, &TextExportOptions { indent: e.indent, ..Default::default() }),
            Format::Html => export_html(
                t,
                &ids,
                &scope,
                &HtmlExportOptions { document: true, title: Some(e.title.clone()), ..Default::default() },
            ),
            Format::Opml => export_opml(
                t,
                &ids,
                &scope,
                &OpmlExportOptions {
                    title: Some(e.title.clone()),
                    source: Some(links::item_url(&s.doc, ROOT)),
                    ..Default::default()
                },
            ),
        }
    })
}

/// A choice of buttons, the chosen one primary (the web's format tabs, and its indent `<select>` in view).
fn choices<T: Copy + PartialEq + 'static>(
    opts: &'static [(T, &'static str)],
    on: impl Fn() -> T + Copy + 'static,
    pick: impl Fn(T) + Copy + 'static,
) -> impl IntoElement {
    view().flex_row().flex_wrap().gap(px(num("space.step5"))).children(opts.iter().map(move |(v, l)| {
        let v = *v;
        dynamic(move || {
            let chosen = on() == v;
            let (bg, fg, border) = if chosen {
                (color::accent(), rgba(style::look().color("color.on_accent")), color::accent())
            } else {
                (color::bg(), color::fg(), color::chip_border())
            };
            nsh_base::button()
                .accessibility_label(Some((*l).into()))
                .selected(chosen)
                .px(px(num("space.btn_x")))
                .py(px(num("space.btn_y")))
                .bg(bg)
                .border(px(num("space.border")))
                .border_color(border)
                .rounded(px(num("radius.block")))
                .on_press(move || pick(v))
                .child(dialog::label(*l).text_color(fg))
                .into_element()
        })
        .into_element()
    }))
}

fn make() -> AnyElement {
    let (title, count) = state().with_untracked(|e| (e.title.clone(), e.ids.len()));
    let label = if count > 1 { format!("Export {count} items") } else { format!("Export \"{title}\"") };
    let out = computed(|| output(&state().get()));
    let (size, line) = style::ui_sized("type.ui_small");
    let body = view()
        .child(
            view()
                .mb(px(num("space.step7")))
                .child(choices(&FORMATS, || state().with(|e| e.format), |f| state().update(|e| e.format = f))),
        )
        .child(when(
            || state().with(|e| e.format == Format::Text),
            || {
                view().mb(px(num("space.step6"))).child(choices(
                    &INDENTS,
                    || state().with(|e| e.indent),
                    |i| state().update(|e| e.indent = i),
                ))
            },
        ))
        .child(dialog::check(
            "Export visible items only",
            || state().with(|e| e.visible_only),
            || {
                let on = !state().with_untracked(|e| e.visible_only);
                state().update(|e| e.visible_only = on);
                crate::settings::change(|p| p.set_flag(VISIBLE_ONLY, on));
            },
        ))
        .child(
            nsh_base::input()
                .test_id("export-output")
                .value(move || out.get())
                .readonly(true)
                .multiline(true)
                .auto_correct(false)
                .h(px(num("space.export_out")))
                .font_family(style::mono())
                .text_size(px(size))
                .line_height(LineHeight::Points(px(line)))
                .text_color(color::fg())
                .bg(color::bg())
                .border(px(num("space.border")))
                .border_color(color::chip_border())
                .rounded(px(num("radius.block")))
                .px(px(num("space.input_x")))
                .py(px(num("space.input_y"))),
        );
    let mut buttons = vec![Button::new("Copy", style::phone(), move || {
        set_clipboard(out.get_untracked());
        toast::show("Copied!");
        false
    })];
    if !style::phone() {
        buttons.push(Button::new("Download", true, move || {
            save(out.get_untracked());
            true
        }));
    }
    dialog::modal("export", label, body, buttons)
}

/// Desktops: the output saved where the save dialog says, named after the title.
fn save(text: String) {
    let (format, title) = state().with_untracked(|e| (e.format, e.title.clone()));
    let (ext, kind) = match format {
        Format::Text => ("txt", "Text"),
        Format::Html => ("html", "HTML"),
        Format::Opml => ("opml", "OPML"),
    };
    let name = format!("{}.{ext}", if title.is_empty() { "export" } else { &title });
    spawn(async move {
        let dialog = SaveDialog::new().file_name(name).filter(FileFilter::new(kind, [ext]));
        match pick_save_path(dialog).await {
            Ok(Some(path)) => match std::fs::write(&path, &text) {
                Ok(()) => toast::show(format!("Saved {}", path.display())),
                Err(e) => toast::show(format!("Could not write {}: {e}", path.display())),
            },
            Ok(None) => {}
            Err(e) => toast::show(format!("Could not export: {e}")),
        }
    });
}
