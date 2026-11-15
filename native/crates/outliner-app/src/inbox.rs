//! The inbox (`apps/web/src/ui/inbox.ts`): `inbox_location` is "<doc>" or "<doc>/<item>", chosen in the
//! settings ("Your inbox") or with an item's "Set as inbox"; "Capture to inbox" (Mod+Shift+I, the menu) adds what
//! is typed there, one item per line with its indentation, at the start or the end of the inbox
//! (`inbox_move_position`); "Go to inbox" opens it.

use nsh_core::keymap::When;
use nsh_core::prelude::*;
use nsh_core::LineHeight;
use outliner_core::{item, NodeId, Tx, ROOT};

use crate::dialog::{self, Button};
use crate::finder::{self, Choice};
use crate::settings;
use crate::store;
use crate::style::{self, color, num};
use crate::toast;

/// The prefs' keys (the web's).
pub const LOCATION: &str = "inbox_location";
pub const POSITION: &str = "inbox_move_position";

const NO_INBOX: &str = "Set an inbox first: open an item’s menu and choose \"Set as inbox\".";

/// The inbox's document and item, when it is set and both are there (`inboxTarget`).
pub fn target() -> Option<(String, NodeId)> {
    let loc = settings::get().text(LOCATION).to_owned();
    if loc.is_empty() {
        return None;
    }
    let (doc, id) = loc.split_once('/').map_or((loc.as_str(), ROOT), |(d, i)| (d, i));
    store::peek(|s| {
        let t = s.tree_of(doc)?;
        (t.get(id).is_some() && t.is_attached(id)).then(|| (doc.to_owned(), id.to_owned()))
    })
}

/// Makes item `id` of document `doc` (its root: the document) the inbox.
pub fn set(doc: &str, id: &str) {
    let loc = if id == ROOT { doc.to_owned() } else { format!("{doc}/{id}") };
    settings::change(|p| p.set_text(LOCATION, &loc));
    toast::show("Inbox set");
}

/// Adds `text` (one item per line, indentation kept) to the inbox (`addToInbox`); whether it did.
pub fn add(text: &str) -> bool {
    let Some((doc, id)) = target() else {
        toast::show(NO_INBOX);
        return false;
    };
    let items = outliner_core::paste::parse_plain_text(text);
    if items.is_empty() {
        return false;
    }
    let top = settings::get().pick(POSITION) == "top";
    store::edit(|s| {
        let Some(session) = s.session_mut(&doc) else { return };
        let after = if top { None } else { session.live.get(&id).and_then(|n| n.children.last().cloned()) };
        let (ops, inverse) = {
            let mut tx = Tx::new(&mut session.live, store::now());
            outliner_core::commands::insert_portable(&mut tx, &id, after.as_deref(), &items);
            (std::mem::take(&mut tx.ops), std::mem::take(&mut tx.inverse))
        };
        session.commit(ops, inverse, None, None, true);
        if s.doc == doc {
            s.reflow();
        }
    });
    crate::workspace::changed(&doc);
    true
}

/// "Capture to inbox" (`captureDialog`): a box to type into; Add (or Mod+Return) adds it.
pub fn capture() {
    if target().is_none() {
        toast::show(NO_INBOX);
        return;
    }
    let typed = Owner::detached().run(|| signal(String::new()));
    let add_typed = move || {
        let t = typed.get_untracked();
        if !t.trim().is_empty() && add(&t) {
            toast::show("Successfully captured to your inbox.");
        }
    };
    dialog::show(move || {
        let opts = KeyOptions { when: When::Always, ..Default::default() };
        on_key_with("mod+enter", opts, move |_| {
            add_typed();
            dialog::close();
        });
        let size = num("type.ui_large");
        let area = nsh_base::input()
            .test_id("capture")
            .multiline(true)
            .auto_focus(true)
            .placeholder("Capture into my inbox…")
            .font_family(style::font())
            .text_size(px(size))
            .line_height(LineHeight::Points(px(size * num("type.ui_line"))))
            .text_color(color::fg())
            .bg(AdaptiveColor::TRANSPARENT)
            .border(px(0.0))
            .p(px(num("space.step6")))
            .min_h(|| px(window().height.0 * num("space.capture_vh")))
            .value(move || typed.get())
            .on_change(move |t: &str| typed.set(t.to_owned()));
        // the box fills the dialog's body, edge to edge (`.capture-wrap`)
        let body = view().mx(px(-num("space.step6"))).my(px(-num("space.step5"))).child(area);
        dialog::modal(
            "capture-dialog",
            "",
            body,
            vec![
                Button::close("Cancel"),
                Button::new("Add", true, move || {
                    add_typed();
                    true
                }),
            ],
        )
    });
}

/// "Go to inbox": its document opened, zoomed in on it.
pub fn goto() {
    let Some((doc, id)) = target() else {
        toast::show(NO_INBOX);
        return;
    };
    let changed = store::peek(|s| s.doc != doc);
    store::edit(|s| {
        s.switch(&doc);
        s.view.zoom = id.clone();
        s.editing = None;
        s.block = None;
        s.caret_rev += 1;
        s.reflow();
    });
    if changed {
        settings::document_opened();
    }
}

/// Where the inbox is, as the settings show it: the document's name and the item's text, or "not set".
pub fn label() -> String {
    let Some((doc, id)) = target() else { return "not set".to_owned() };
    store::peek(|s| {
        let title = s.title(&doc);
        if id == ROOT {
            return title;
        }
        let text = s.tree_of(&doc).map(|t| item::content(t.get(&id)).to_owned()).unwrap_or_default();
        let plain: String = outliner_core::inline::inline_to_plain(&text, None).chars().take(40).collect();
        format!("{title} › {plain}")
    })
}

/// The settings' "Change…" (`pickInbox`): every document and every item with text, in a finder.
pub fn pick() {
    let mut all = Vec::new();
    store::peek(|s| {
        for d in &s.files {
            let Some(t) = s.tree_of(d) else { continue };
            let title = s.title(d);
            let doc = d.clone();
            all.push(Choice::new(title.clone(), "Document", move || set(&doc, ROOT)));
            let mut stack: Vec<NodeId> = t.get(ROOT).map(|n| n.children.iter().rev().cloned().collect()).unwrap_or_default();
            while let Some(id) = stack.pop() {
                let Some(n) = t.get(&id) else { continue };
                stack.extend(n.children.iter().rev().cloned());
                let c = item::content(Some(n));
                if c.is_empty() {
                    continue;
                }
                let sub = format!("{title} › {}", crate::find::breadcrumb(t, &id, ROOT));
                let doc = d.clone();
                all.push(Choice::new(c, sub, move || set(&doc, &id)));
            }
        }
    });
    finder::open("Choose your inbox…", all);
}
