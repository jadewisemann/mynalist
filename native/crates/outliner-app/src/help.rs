//! The "Shortcuts & formatting" panel (`apps/web/src/ui/help.ts`, `.help-panel`): the triggers, the formatting
//! and every command of the keymap with its keys as the keymap in use has them, by group; beside the document on
//! desktops, over the whole screen on phones. And the tutorial and "What's new" dialogs (`ui/tour.ts`), opened
//! from the menu (the web shows no tour on its own at the first start either).

use nsh_core::prelude::*;

use crate::dialog::{self, Button};
use crate::icons::icon;
use crate::keymap::{self, Group, COMMANDS};
use crate::sidebar::ui_text;
use crate::style::{self, color, num};

const TRIGGERS: &[(&str, &str)] = &[("#  or  @", "Tag autocomplete"), ("!", "Date picker"), ("[[", "Link to another item or document")];

const FORMATS: &[(&str, &str)] = &[
    ("**bold**", "Bold"),
    ("__italic__", "Italic"),
    ("~~strike~~", "Strikethrough"),
    ("==highlight==", "Highlight"),
    ("`code`", "Inline code"),
    ("```lang↵…```", "Code block"),
    ("[text](url)", "Link"),
    ("![alt](url)", "Image"),
    ("$$E=mc^2$$", "LaTeX"),
    ("#tag @tag", "Tags"),
    ("!(2026-01-31 14:00)", "Date"),
    ("!(2026-01-31 | 1w)", "Recurring date"),
];

#[derive(Clone, Copy, Default, PartialEq)]
struct Open(bool);

fn open() -> Signal<Open> {
    global::<Open>()
}

/// Shows the panel, or hides it (`toggle`); `Some` shows or hides it.
pub fn toggle(show: Option<bool>) {
    open().update(|o| o.0 = show.unwrap_or(!o.0));
}

pub fn is_open() -> bool {
    open().with(|o| o.0)
}

fn group_name(g: Group) -> &'static str {
    match g {
        Group::Basics => "Basics",
        Group::General => "General",
        Group::Lists => "Lists",
        Group::Navigate => "Navigate",
        Group::Search => "Search",
        Group::Format => "Format",
    }
}

/// The panel's sections: their heads and rows (a name and its keys or syntax), as the web lists them.
pub fn sections() -> Vec<(&'static str, Vec<(String, String)>)> {
    let km = keymap::current();
    let apple = apple_keys();
    let mut out: Vec<(&'static str, Vec<(String, String)>)> = vec![
        ("Triggers", TRIGGERS.iter().map(|(a, b)| ((*b).to_owned(), (*a).to_owned())).collect()),
        ("Formatting", FORMATS.iter().map(|(a, b)| ((*b).to_owned(), (*a).to_owned())).collect()),
    ];
    for c in COMMANDS {
        let keys = km.keys_for(c.id);
        let shown = if keys.is_empty() {
            "—".to_owned()
        } else {
            keys.iter().map(|k| keymap::pretty(k, apple)).collect::<Vec<_>>().join(" / ")
        };
        let name = group_name(c.group);
        match out.iter_mut().find(|(g, _)| *g == name) {
            Some((_, rows)) => rows.push((c.label.to_owned(), shown)),
            None => out.push((name, vec![(c.label.to_owned(), shown)])),
        }
    }
    out
}

/// The panel while it shows: beside the document on desktops (`.help-panel`).
pub fn panel() -> impl IntoElement {
    when(|| is_open() && !style::phone(), || body().w(px(num("space.help"))).border_l(px(num("space.border"))))
}

/// The panel while it shows on phones: over the whole screen.
pub fn sheet() -> impl IntoElement {
    when(
        || is_open() && style::phone(),
        || body().absolute().top(px(0.0)).bottom(px(0.0)).left(px(0.0)).right(px(0.0)).pt_safe_area(),
    )
}

fn body() -> View {
    let (small, small_line) = style::ui_sized("type.ui_small");
    let head_size = num("type.ui_head");
    let hover = color::hover();
    let close = nsh_base::button()
        .h(px(num("space.topbar_tool")))
        .min_w(px(num("space.topbar_tool_min")))
        .px(px(num("space.topbar_tool_pad")))
        .rounded(px(num("radius.topbar_tool")))
        .items_center()
        .justify_center()
        .text_color(color::topbar_fg())
        .accessibility_label(Some("Close".into()))
        .active(move |s| s.bg(hover))
        .on_press(|| toggle(Some(false)))
        .child(icon("close", num("space.topbar_icon")));
    let head = view()
        .flex_row()
        .items_center()
        .pt(px(num("space.step6")))
        .pb(px(num("space.step6")))
        .pr(px(num("space.step7")))
        .pl(px(num("space.step9")))
        .border_b(px(num("space.border")))
        .border_color(color::line())
        .child(
            ui_text("Shortcuts & formatting", (head_size, head_size * num("type.ui_line")))
                .font_weight(style::weight("type.heading_weight"))
                .text_color(color::fg())
                .flex_1(),
        )
        .child(close);
    let mut list = view().pt(px(num("space.step4"))).px(px(num("space.step9"))).pb(px(num("space.step11")));
    for (name, rows) in sections() {
        list = list.child(
            ui_text(name, (small, small_line))
                .font_weight(style::weight("type.heading_weight"))
                .text_color(color::fg())
                .mt(px(num("space.help_sec_top")))
                .mb(px(num("space.step4"))),
        );
        for (label, keys) in rows {
            list = list.child(
                view()
                    .test_id("help-row")
                    .flex_row()
                    .justify_between()
                    .gap(px(num("space.step4")))
                    .py(px(num("space.step3")))
                    .px(px(num("space.step4")))
                    .border_b(px(num("space.border")))
                    .border_color(color::line())
                    .child(ui_text(label, (small, small_line)).text_color(color::fg()).flex_shrink(1.0))
                    .child(ui_text(keys, (small, small_line)).font_family(style::mono()).text_color(color::muted())),
            );
        }
    }
    let (tiny, tiny_line) = style::ui_sized("type.ui_tiny");
    list = list.child(
        ui_text("Change shortcuts in Settings → Keymap.", (tiny, tiny_line)).text_color(color::muted()).mt(px(num("space.step8"))),
    );
    view()
        .test_id("help")
        .flex_col()
        .bg(color::bg())
        .border_color(color::line())
        .child(head)
        .child(scroll().flex_1().child(list))
}

// ---- the tutorial and what's new ------------------------------------------------------------------------

const STEPS: &[(&str, &str)] = &[
    ("Items", "Everything is a list of items. Enter adds an item, Tab and Shift+Tab indent and unindent, Ctrl+↑/↓ moves an item."),
    ("Zoom", "Ctrl+] zooms into an item so it fills the page (Ctrl+[ zooms back out). Click a bullet to expand or collapse; double-click it to expand or collapse everything below."),
    ("Notes and checkboxes", "Shift+Enter adds a note under an item. Ctrl+Enter checks an item off; Ctrl+Shift+C gives it a checkbox."),
    ("Formatting", "**bold**, __italic__, `code`, ==highlight==, [links](https://…), #tags and dates like !(2026-12-31). Type # or ! to get suggestions and a date picker."),
    ("Links between items", "Type [[ to link to any item in any document. With \"Show backlinks\" on, linked items show who refers to them."),
    ("Search", "Ctrl+F filters the document as you type. Shift+Enter in the search box shows a flat list of results, Ctrl+Enter searches every document."),
    ("Find and capture", "Ctrl+O jumps to a document, Ctrl+Shift+O to any item. Set an item as your inbox from its menu, then Ctrl+Shift+I captures a thought into it from anywhere."),
    ("Your data", "Everything works offline and syncs to your own server. Version history (document menu) and Download backup (main menu) keep you safe."),
];

/// The pref the tutorial sets when it is done or skipped (the web's).
pub const FINISHED: &str = "finished_onboarding";

/// The tutorial (`tutorial`): a step at a time, its dots to jump to one, Skip, and Next (Done on the last).
pub fn tutorial() {
    // under no view's owner: the menu that opened it goes away
    let at = Owner::detached().run(|| signal(0usize));
    let done = || {
        dialog::close();
        crate::settings::finish_onboarding();
    };
    dialog::show(move || {
        let (size, line) = style::ui();
        let head = (size, line);
        let dots = view()
            .flex_row()
            .justify_center()
            .gap(px(num("space.step5")))
            .my(px(num("space.step7")))
            .children((0..STEPS.len()).map(move |k| {
                let d = num("space.step6");
                nsh_base::button()
                    .accessibility_label(Some(format!("Step {}", k + 1).into()))
                    .w(px(d))
                    .h(px(d))
                    .rounded(px(d / 2.0))
                    .bg(move || if at.get() == k { color::accent() } else { color::faint() })
                    .on_press(move || at.set(k))
                    .into_element()
            }));
        let foot = view()
            .flex_row()
            .justify_between()
            .child(dialog::btn("Skip", false, done))
            .child(dynamic(move || {
                let last = at.get() == STEPS.len() - 1;
                dialog::btn(if last { "Done" } else { "Next" }, true, move || {
                    if at.get_untracked() < STEPS.len() - 1 {
                        at.update(|i| *i += 1);
                    } else {
                        done();
                    }
                })
                .into_element()
            }));
        let body = view()
            .test_id("tour")
            .child(
                ui_text(move || format!("{}. {}", at.get() + 1, STEPS[at.get()].0), head)
                    .font_weight(style::weight("type.heading_weight"))
                    .text_color(color::fg())
                    .mt(px(num("space.step4")))
                    .mb(px(num("space.step6"))),
            )
            .child(ui_text(move || STEPS[at.get()].1, (size, size * num("type.tour_line"))).text_color(color::fg()))
            .child(dots)
            .child(foot);
        dialog::modal("tour-dialog", "Welcome to Outliner", body, Vec::new())
    });
}

const NEWS: &[(&str, &[&str])] = &[(
    "October 2026",
    &[
        "Version history with restore, and nightly server backups; Download backup from the main menu.",
        "Search results screens: flat search (Shift+Enter) and search everywhere (Ctrl+Enter), with sorting.",
        "Search and replace, more sort orders, backlinks, inbox capture (Ctrl+Shift+I).",
        "Settings with a keymap editor, per-document view options, article layout, mind map, locked documents.",
        "Syntax highlighting and a Copy button for code blocks; LaTeX rendering.",
        "File uploads, shared read-only links and a calendar feed (with a sync server).",
    ],
)];

/// "What's new" (`whatsNew`).
pub fn whats_new() {
    dialog::show(|| {
        let ui = style::ui();
        let mut body = view().test_id("news");
        for (when, items) in NEWS {
            body = body.child(
                ui_text(*when, ui).font_weight(style::weight("type.heading_weight")).text_color(color::fg()).mb(px(num("space.step4"))),
            );
            for t in items.iter() {
                body = body.child(
                    view()
                        .flex_row()
                        .gap(px(num("space.step4")))
                        .child(ui_text("•", ui).text_color(color::fg()))
                        .child(ui_text(*t, ui).text_color(color::fg()).flex_shrink(1.0)),
                );
            }
        }
        dialog::modal("news-dialog", "What's new", body, vec![Button::close("Okay")])
    });
}

/// "About" (`aboutDialog`): what the app is, and where it syncs (this app: on the device only, for now).
pub fn about() {
    dialog::show(|| {
        let ui = style::ui();
        let para = |t: &'static str| ui_text(t, ui).text_color(color::fg()).mb(px(num("space.step6")));
        let body = view()
            .test_id("about")
            .child(para("A self-hosted, keyboard-first outliner. Your documents live on this device and on your own sync server."))
            .child(para("Its behaviour follows Dynalist, re-implemented from observation (no Dynalist code or assets)."))
            .child(ui_text("Sync: local only", ui).text_color(color::muted()));
        dialog::modal("about-dialog", "About Outliner", body, vec![Button::new("Okay", true, || true)])
    });
}

/// "Import from WorkFlowy…": how to export a WorkFlowy list as OPML, and the OPML import.
pub fn workflowy_import() {
    dialog::show(|| {
        let body = ui_text(
            "In WorkFlowy, open the list you want, choose Export → OPML and save the file. Then import it here; each file becomes a document.",
            style::ui(),
        )
        .text_color(color::fg());
        let choose = Button::new("Choose OPML file…", true, || {
            Owner::detached().run(|| crate::settings::import_files(None));
            true
        });
        dialog::modal("workflowy-dialog", "Import from WorkFlowy", view().child(body), vec![Button::close("Cancel"), choose])
    });
}
