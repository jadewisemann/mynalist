//! The keymap (the web's `editor/keymap.ts`): the commands with their default keys, the user's changes
//! (`shortcut_override`) and the WorkFlowy-style keys over them, and the keys' names as the settings show them.
//! Keys are written as the web writes them (`Mod+Shift+.`: `Mod` is Command on Apple keyboards and Control
//! elsewhere, `Ctrl` is Control itself on Apple keyboards), so a keymap made on one app works on the other.

use std::collections::BTreeMap;

/// A command's group in the keymap list (the web's help sections).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Basics,
    General,
    Lists,
    Navigate,
    Search,
    Format,
}

/// A command of the keymap: its id, label, group and default keys.
#[derive(Debug, Clone, Copy)]
pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub group: Group,
    pub keys: &'static [&'static str],
}

const fn cmd(id: &'static str, label: &'static str, group: Group, keys: &'static [&'static str]) -> Command {
    Command { id, label, group, keys }
}

/// Every command of the web's keymap, in its order (docs/02-editor-keyboard.md §1).
pub const COMMANDS: &[Command] = &[
    cmd("indent", "Indent", Group::Basics, &["Tab"]),
    cmd("unindent", "Unindent", Group::Basics, &["Shift+Tab"]),
    cmd("collapse", "Expand/collapse", Group::Basics, &["Mod+."]),
    cmd("zoom-in", "Zoom in", Group::Basics, &["Mod+]"]),
    cmd("zoom-out", "Zoom out", Group::Basics, &["Mod+["]),
    cmd("delete-left", "Delete item", Group::Basics, &["Mod+Shift+Backspace"]),
    cmd("help", "Shortcuts & formatting help", Group::General, &["Mod+/", "Mod+Shift+/"]),
    cmd("settings", "Open settings", Group::General, &["Mod+,"]),
    cmd("toggle-file-pane", "Toggle file pane", Group::General, &["Mod+Shift+F"]),
    cmd("toggle-bookmark-pane", "Toggle bookmark pane", Group::General, &["Mod+Shift+B"]),
    cmd("toggle-tag-pane", "Toggle tag pane", Group::General, &[]),
    cmd("toggle-show-checked", "Show/hide checked items", Group::General, &[]),
    cmd("toggle-show-notes", "Show/hide notes", Group::General, &[]),
    cmd("toggle-doc-checked", "Show/hide checked items in this document", Group::General, &[]),
    cmd("toggle-doc-notes", "Show/hide notes in this document", Group::General, &[]),
    cmd("toggle-spellcheck", "Toggle spell checker", Group::General, &[]),
    cmd("version-history", "Version history", Group::General, &[]),
    cmd("download-backup", "Download backup", Group::General, &[]),
    cmd("save", "Save / sync now", Group::General, &["Mod+S"]),
    cmd("inbox-capture", "Capture to inbox", Group::General, &["Mod+Shift+I"]),
    cmd("bookmark", "Bookmark current view", Group::General, &[]),
    cmd("delete-right", "Delete item (focus next)", Group::Lists, &["Mod+Shift+Delete"]),
    cmd("goto-note", "Move between item and note", Group::Lists, &["Shift+Enter"]),
    cmd("toggle-checked", "Toggle completed", Group::Lists, &["Mod+Enter"]),
    cmd("uncheck-children", "Uncheck item and descendants", Group::Lists, &[]),
    cmd("collapse-all", "Expand/collapse all", Group::Lists, &["Mod+Shift+."]),
    cmd("toggle-siblings", "Expand/collapse siblings", Group::Lists, &[]),
    cmd("toggle-checkbox", "Toggle checkbox", Group::Lists, &["Mod+Shift+C"]),
    cmd("toggle-children-checkbox", "Toggle checkbox on children", Group::Lists, &[]),
    cmd("toggle-numbered", "Toggle numbered list", Group::Lists, &["Mod+Shift+X"]),
    cmd("duplicate", "Duplicate", Group::Lists, &[]),
    cmd("select-up", "Select up", Group::Lists, &["Shift+Up"]),
    cmd("select-down", "Select down", Group::Lists, &["Shift+Down"]),
    cmd("select-all", "Select everything", Group::Lists, &["Mod+Shift+A"]),
    cmd("undo", "Undo", Group::Lists, &["Mod+Z"]),
    cmd("redo", "Redo", Group::Lists, &["Mod+Y", "Mod+Shift+Z"]),
    cmd("copy-link", "Copy link to item", Group::Lists, &[]),
    cmd("get-link", "Get link to item", Group::Lists, &[]),
    cmd("show-references", "Show all references", Group::Lists, &[]),
    cmd("insert-template", "Insert template…", Group::Lists, &[]),
    cmd("delete-checked", "Delete completed items", Group::Lists, &[]),
    cmd("sort-asc", "Sort children A→Z", Group::Lists, &[]),
    cmd("sort-desc", "Sort children Z→A", Group::Lists, &[]),
    cmd("sort-date-new", "Sort children by date (new→old)", Group::Lists, &[]),
    cmd("sort-date-old", "Sort children by date (old→new)", Group::Lists, &[]),
    cmd("expand-1", "Expand to level 1", Group::Lists, &[]),
    cmd("expand-2", "Expand to level 2", Group::Lists, &[]),
    cmd("expand-3", "Expand to level 3", Group::Lists, &[]),
    cmd("expand-4", "Expand to level 4", Group::Lists, &[]),
    cmd("move-to", "Move item to…", Group::Navigate, &["Mod+Shift+M"]),
    cmd("swap-up", "Move up", Group::Navigate, &["Mod+Up"]),
    cmd("swap-down", "Move down", Group::Navigate, &["Mod+Down"]),
    cmd("zoom-prev-sibling", "Zoom into previous sibling", Group::Navigate, &["Mod+Shift+["]),
    cmd("zoom-next-sibling", "Zoom into next sibling", Group::Navigate, &["Mod+Shift+]"]),
    cmd("goto-inbox", "Go to inbox", Group::Navigate, &[]),
    cmd("goto-parent", "Go to parent item", Group::Navigate, &[]),
    cmd("goto-prev-sibling", "Go to previous sibling", Group::Navigate, &[]),
    cmd("goto-next-sibling", "Go to next sibling", Group::Navigate, &[]),
    cmd("goto-item-start", "Go to start of item text", Group::Navigate, &[]),
    cmd("goto-item-end", "Go to end of item text", Group::Navigate, &[]),
    cmd("goto-first", "Go to first item", Group::Navigate, &["Mod+Home"]),
    cmd("goto-last", "Go to last item", Group::Navigate, &["Mod+End"]),
    cmd("back", "Back", Group::Navigate, &["Alt+Left"]),
    cmd("forward", "Forward", Group::Navigate, &["Alt+Right"]),
    cmd("search", "Search in document", Group::Search, &["Mod+F"]),
    cmd("go-anywhere", "File finder", Group::Search, &["Mod+O"]),
    cmd("goto-item", "Item finder", Group::Search, &["Mod+Shift+O"]),
    cmd("search-replace", "Search and replace", Group::Search, &[]),
    cmd("bold", "Bold", Group::Format, &["Mod+B"]),
    cmd("italic", "Italic", Group::Format, &["Mod+I"]),
    cmd("code", "Inline code", Group::Format, &["Mod+`"]),
    cmd("code-block", "Code block", Group::Format, &[]),
    cmd("link", "Link", Group::Format, &["Mod+K"]),
    cmd("strike", "Strikethrough", Group::Format, &[]),
    cmd("highlight", "Highlight", Group::Format, &[]),
    cmd("latex", "LaTeX", Group::Format, &[]),
    cmd("newline", "New line in item", Group::Format, &["Mod+Shift+Enter"]),
    cmd("cycle-heading", "Cycle heading", Group::Format, &["Mod+Shift+H"]),
    cmd("heading-1", "Heading 1", Group::Format, &[]),
    cmd("heading-2", "Heading 2", Group::Format, &[]),
    cmd("heading-3", "Heading 3", Group::Format, &[]),
    cmd("heading-0", "Clear heading", Group::Format, &[]),
    cmd("cycle-color", "Cycle color label", Group::Format, &["Mod+Shift+L"]),
    cmd("color-1", "Color label 1", Group::Format, &[]),
    cmd("color-2", "Color label 2", Group::Format, &[]),
    cmd("color-3", "Color label 3", Group::Format, &[]),
    cmd("color-4", "Color label 4", Group::Format, &[]),
    cmd("color-5", "Color label 5", Group::Format, &[]),
    cmd("color-6", "Color label 6", Group::Format, &[]),
    cmd("color-0", "Clear color label", Group::Format, &[]),
    cmd("insert-date", "Insert current date", Group::Format, &[]),
    cmd("insert-time", "Insert current time", Group::Format, &[]),
];

/// Commands whose keys cannot be changed: they are part of text editing itself.
pub const FIXED: &[&str] = &["undo", "redo", "select-up", "select-down"];

/// The WorkFlowy-style keys (`workflowy_shortcuts`), over the defaults.
fn workflowy(id: &str, apple: bool) -> Option<Vec<String>> {
    let k: &str = match id {
        "zoom-in" => "Alt+Right",
        "zoom-out" => "Alt+Left",
        "collapse" => {
            if apple {
                "Ctrl+Up"
            } else {
                "Mod+Up"
            }
        }
        "swap-up" => "Alt+Shift+Up",
        "swap-down" => "Alt+Shift+Down",
        _ => return None,
    };
    Some(vec![k.to_owned()])
}

/// Which command each key runs (`Keymap` on the web).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Keymap {
    /// Key to command id, in the order the keys were set (a later command taking a key wins it).
    map: Vec<(String, &'static str)>,
}

impl Keymap {
    /// The keymap of the user's changes `overrides` (`shortcut_override`) and the WorkFlowy-style keys, on an
    /// Apple keyboard or not.
    pub fn new(overrides: &BTreeMap<String, Vec<String>>, workflowy_keys: bool, apple: bool) -> Keymap {
        let mut map: Vec<(String, &'static str)> = Vec::new();
        for c in COMMANDS {
            let keys = overrides
                .get(c.id)
                .cloned()
                .or_else(|| if workflowy_keys { workflowy(c.id, apple) } else { None })
                .unwrap_or_else(|| c.keys.iter().map(|k| (*k).to_owned()).collect());
            for k in keys {
                // a JS Map: setting a key again keeps its place and changes its command
                match map.iter_mut().find(|(x, _)| *x == k) {
                    Some(e) => e.1 = c.id,
                    None => map.push((k, c.id)),
                }
            }
        }
        // the WorkFlowy keys win over the defaults of later commands on the same keys (Alt+Left/Right are also
        // Back/Forward), but not over a key the user gave a command
        if workflowy_keys {
            for c in COMMANDS.iter().filter(|c| !overrides.contains_key(c.id)) {
                for k in workflowy(c.id, apple).unwrap_or_default() {
                    match map.iter_mut().find(|(x, _)| *x == k) {
                        Some(e) => {
                            let owned = overrides.get(e.1).is_some_and(|o| o.contains(&k));
                            if !owned {
                                e.1 = c.id;
                            }
                        }
                        None => map.push((k, c.id)),
                    }
                }
            }
        }
        Keymap { map }
    }

    /// The command key `combo` runs.
    pub fn resolve(&self, combo: &str) -> Option<&'static str> {
        self.map.iter().find(|(k, _)| k == combo).map(|(_, id)| *id)
    }

    /// The keys of command `id`.
    pub fn keys_for(&self, id: &str) -> Vec<String> {
        self.map.iter().filter(|(_, c)| *c == id).map(|(k, _)| k.clone()).collect()
    }

    /// The overrides after `combo` is added to command `id` (taken from the command that had it), as the web's
    /// keymap editor records a key.
    pub fn with_key(&self, overrides: &BTreeMap<String, Vec<String>>, id: &str, combo: &str) -> BTreeMap<String, Vec<String>> {
        let mut o = overrides.clone();
        if let Some(taken) = self.resolve(combo).filter(|t| *t != id) {
            o.insert(taken.to_owned(), self.keys_for(taken).into_iter().filter(|k| k != combo).collect());
        }
        let mut keys = self.keys_for(id);
        if !keys.iter().any(|k| k == combo) {
            keys.push(combo.to_owned());
        }
        o.insert(id.to_owned(), keys);
        o
    }

    /// The overrides after `combo` is taken from command `id`.
    pub fn without_key(&self, overrides: &BTreeMap<String, Vec<String>>, id: &str, combo: &str) -> BTreeMap<String, Vec<String>> {
        let mut o = overrides.clone();
        o.insert(id.to_owned(), self.keys_for(id).into_iter().filter(|k| k != combo).collect());
        o
    }
}

/// The NSH keystroke of a web key combo (`Mod+Shift+.` is `mod+shift+.`) on an Apple keyboard or not, or `None`
/// for a key NSH has no name for (or `Mod+Ctrl` where the two are one key).
pub fn keystroke(combo: &str, apple: bool) -> Option<String> {
    let k = combo.to_ascii_lowercase();
    nsh_core::keymap::Keystroke::parse_with(&k, apple).ok().map(|_| k)
}

/// A key combo as the settings show it (`prettyCombo`): ⌘⇧. on Apple keyboards, Ctrl+Shift+. elsewhere.
pub fn pretty(combo: &str, apple: bool) -> String {
    let parts: Vec<&str> = combo
        .split('+')
        .map(|p| match p {
            "Mod" => {
                if apple {
                    "⌘"
                } else {
                    "Ctrl"
                }
            }
            "Ctrl" => "⌃",
            "Alt" if apple => "⌥",
            "Shift" if apple => "⇧",
            "Enter" => "↵",
            "Backspace" => "⌫",
            p => p,
        })
        .collect();
    parts.join(if apple { "" } else { "+" })
}

/// The modifiers a recorded key may have, as the web writes them, in its order.
const MODS: [&str; 4] = ["Mod", "Ctrl", "Alt", "Shift"];

/// The keys a recording listens for, in the web's names (`keyOf`): letters, digits, punctuation, the named keys
/// and F1 to F12.
pub fn recordable_keys() -> Vec<String> {
    let mut keys: Vec<String> = ('A'..='Z').chain('0'..='9').map(String::from).collect();
    keys.extend([".", ",", "/", "\\", "[", "]", "`", "-", "=", ";", "'"].map(String::from));
    keys.extend(
        ["Up", "Down", "Left", "Right", "Enter", "Backspace", "Delete", "Tab", "Escape", "Home", "End", "Space", "PageUp", "PageDown"]
            .map(String::from),
    );
    keys.extend((1..=12).map(|n| format!("F{n}")));
    keys
}

/// Every combo of `key` with at least one modifier (and the plain function keys), in the web's spelling; on
/// keyboards that are not Apple's `Ctrl` is the same key as `Mod`, so it is left out.
pub fn combos_of(key: &str, apple: bool) -> Vec<String> {
    let mods: Vec<&str> = MODS.iter().copied().filter(|m| apple || *m != "Ctrl").collect();
    let mut out = Vec::new();
    for bits in 0..(1u32 << mods.len()) {
        let held: Vec<&str> = mods.iter().enumerate().filter(|(i, _)| bits & (1 << i) != 0).map(|(_, m)| *m).collect();
        let fkey = key.len() > 1 && key.starts_with('F');
        if held.is_empty() && !fkey || key == "Escape" {
            continue;
        }
        let mut parts = held;
        parts.push(key);
        out.push(parts.join("+"));
    }
    out
}

/// Where a command's keys act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// While an item's text is edited.
    Editing,
    /// While items are selected.
    Block,
    /// Anywhere: the app's commands.
    App,
}

/// What a command does, and where its keys act.
pub type Action = (fn(), &'static [Scope]);

/// What command `id` does in the native app, where its keys act; `None` for a command the app does not have yet.
/// The editor's commands run through `commands::exec` (the web's `Editor.exec`), the app's here.
pub fn action(id: &str) -> Option<Action> {
    use crate::commands as cmd;
    use outliner_core::commands::Dir;
    use Scope::{App, Block, Editing};
    const BOTH: &[Scope] = &[Editing, Block];
    const EDIT: &[Scope] = &[Editing];
    // a command of the editor (`Editor.exec`), by its id: a closure taking nothing is a plain `fn()`
    macro_rules! exec {
        ($id:literal, $scopes:expr) => {
            (
                || {
                    cmd::exec($id);
                },
                $scopes,
            )
        };
    }
    let a: Action = match id {
        "indent" => (cmd::indent, &[Block]),
        "unindent" => (cmd::outdent, BOTH),
        "collapse" => (|| cmd::toggle_collapse(false), BOTH),
        "collapse-all" => (|| cmd::toggle_collapse(true), BOTH),
        "zoom-in" => (cmd::zoom_in, BOTH),
        "zoom-out" => (cmd::zoom_out, EDIT),
        "delete-left" => (|| cmd::delete_items(Dir::Left), BOTH),
        "delete-right" => (|| cmd::delete_items(Dir::Right), &[Block]),
        "goto-note" => (cmd::goto_note, EDIT),
        "toggle-checked" => (|| cmd::toggle_checked(false), BOTH),
        "toggle-checkbox" => (cmd::toggle_checkbox, BOTH),
        "swap-up" => (cmd::move_up, BOTH),
        "swap-down" => (cmd::move_down, BOTH),
        "select-all" => (cmd::select_all, BOTH),
        "undo" => (|| cmd::undo(false), BOTH),
        "redo" => (|| cmd::undo(true), BOTH),
        "uncheck-children" => exec!("uncheck-children", BOTH),
        "toggle-siblings" => exec!("toggle-siblings", BOTH),
        "toggle-children-checkbox" => exec!("toggle-children-checkbox", BOTH),
        "toggle-numbered" => exec!("toggle-numbered", BOTH),
        "duplicate" => exec!("duplicate", BOTH),
        "delete-checked" => exec!("delete-checked", BOTH),
        "sort-asc" => exec!("sort-asc", BOTH),
        "sort-desc" => exec!("sort-desc", BOTH),
        "sort-date-new" => exec!("sort-date-new", BOTH),
        "sort-date-old" => exec!("sort-date-old", BOTH),
        "expand-1" => exec!("expand-1", BOTH),
        "expand-2" => exec!("expand-2", BOTH),
        "expand-3" => exec!("expand-3", BOTH),
        "expand-4" => exec!("expand-4", BOTH),
        "zoom-prev-sibling" => exec!("zoom-prev-sibling", BOTH),
        "zoom-next-sibling" => exec!("zoom-next-sibling", BOTH),
        "goto-parent" => exec!("goto-parent", BOTH),
        "goto-prev-sibling" => exec!("goto-prev-sibling", BOTH),
        "goto-next-sibling" => exec!("goto-next-sibling", BOTH),
        "goto-item-start" => exec!("goto-item-start", EDIT),
        "goto-item-end" => exec!("goto-item-end", EDIT),
        "goto-first" => exec!("goto-first", BOTH),
        "goto-last" => exec!("goto-last", BOTH),
        "bold" => exec!("bold", BOTH),
        "italic" => exec!("italic", BOTH),
        "code" => exec!("code", BOTH),
        "code-block" => exec!("code-block", BOTH),
        "strike" => exec!("strike", BOTH),
        "highlight" => exec!("highlight", BOTH),
        "latex" => exec!("latex", BOTH),
        "link" => exec!("link", EDIT),
        "newline" => exec!("newline", EDIT),
        "insert-date" => exec!("insert-date", EDIT),
        "insert-time" => exec!("insert-time", EDIT),
        "cycle-heading" => exec!("cycle-heading", BOTH),
        "heading-1" => exec!("heading-1", BOTH),
        "heading-2" => exec!("heading-2", BOTH),
        "heading-3" => exec!("heading-3", BOTH),
        "heading-0" => exec!("heading-0", BOTH),
        "cycle-color" => exec!("cycle-color", BOTH),
        "color-1" => exec!("color-1", BOTH),
        "color-2" => exec!("color-2", BOTH),
        "color-3" => exec!("color-3", BOTH),
        "color-4" => exec!("color-4", BOTH),
        "color-5" => exec!("color-5", BOTH),
        "color-6" => exec!("color-6", BOTH),
        "color-0" => exec!("color-0", BOTH),
        // the app's commands (`App.appCommand`): whatever has the focus
        "move-to" => (crate::finder::move_to, BOTH),
        "insert-template" => (crate::finder::insert_template, BOTH),
        "copy-link" => (crate::itemmenu::copy_link_here, BOTH),
        "get-link" => (crate::itemmenu::get_link_here, &[App]),
        "show-references" => (crate::itemmenu::references_here, &[App]),
        "go-anywhere" => (crate::finder::go_anywhere, &[App]),
        "goto-item" => (crate::finder::goto_item, &[App]),
        "search-replace" => (crate::find::replace_dialog, &[App]),
        "bookmark" => (crate::sidebar::bookmark_current, &[App]),
        "settings" => (|| crate::settings::open(crate::settings::Tab::Prefs), &[App]),
        "toggle-file-pane" => (|| crate::sidebar::toggle_tab(crate::sidebar::Tab::Files), &[App]),
        "toggle-bookmark-pane" => (|| crate::sidebar::toggle_tab(crate::sidebar::Tab::Bookmarks), &[App]),
        "toggle-tag-pane" => (|| crate::sidebar::toggle_tab(crate::sidebar::Tab::Tags), &[App]),
        "help" => (|| crate::help::toggle(None), &[App]),
        "back" => (|| crate::store::travel(false), &[App]),
        "forward" => (|| crate::store::travel(true), &[App]),
        "inbox-capture" => (crate::inbox::capture, &[App]),
        "goto-inbox" => (crate::inbox::goto, &[App]),
        "toggle-show-checked" => (
            || {
                crate::settings::change(|p| {
                    let on = p.flag("show_checked");
                    p.set_flag("show_checked", !on);
                })
            },
            &[App],
        ),
        "toggle-show-notes" => (
            || {
                crate::settings::change(|p| {
                    let to = if p.pick("show_notes") == "hide" { "show" } else { "hide" };
                    p.set_pick("show_notes", to);
                })
            },
            &[App],
        ),
        _ => return None,
    };
    Some(a)
}

/// The keymap the settings make (`applyPrefs`): the user's changes and the WorkFlowy-style keys.
pub fn current() -> Keymap {
    let p = crate::settings::get();
    Keymap::new(&p.shortcuts(), p.flag("workflowy_shortcuts"), nsh_core::prelude::apple_keys())
}

/// The keys of command `id`, as NSH keystrokes.
pub fn keys_of(id: &str) -> Vec<String> {
    let apple = nsh_core::prelude::apple_keys();
    current().keys_for(id).iter().filter_map(|k| keystroke(k, apple)).collect()
}

/// The keys of the commands acting in `scope`, as NSH keystrokes with their action (`skip`: commands bound
/// elsewhere).
pub fn bindings(scope: Scope, skip: &[&str]) -> Vec<(String, fn())> {
    let km = current();
    let apple = nsh_core::prelude::apple_keys();
    let mut out = Vec::new();
    for c in COMMANDS.iter().filter(|c| !skip.contains(&c.id)) {
        let Some((run, scopes)) = action(c.id) else { continue };
        if !scopes.contains(&scope) {
            continue;
        }
        for k in km.keys_for(c.id) {
            if let Some(ks) = keystroke(&k, apple) {
                out.push((ks, run));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The web's command list (`COMMANDS` in apps/web/src/editor/keymap.ts), read from its source: ids, labels and
    /// default keys in order.
    fn web_commands() -> Vec<(String, String, Vec<String>)> {
        let src = include_str!("../../../../apps/web/src/editor/keymap.ts");
        let mut out = Vec::new();
        let quoted = |s: &str| -> Vec<String> { s.split('\'').skip(1).step_by(2).map(String::from).collect() };
        for line in src.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix("{ id: '") {
                let id = rest.split('\'').next().unwrap().to_owned();
                let label = l.split("label: '").nth(1).unwrap().split('\'').next().unwrap().to_owned();
                let keys = l.split("keys: [").nth(1).unwrap().rsplit_once("] }").unwrap().0;
                out.push((id, label, quoted(keys)));
            }
            if l.starts_with("...[1, 2, 3, 4, 5, 6]") {
                out.extend((1..=6).map(|n| (format!("color-{n}"), format!("Color label {n}"), Vec::new())));
            }
        }
        out
    }

    #[test]
    fn the_commands_are_the_webs() {
        let web = web_commands();
        let ours: Vec<(String, String, Vec<String>)> = COMMANDS
            .iter()
            .map(|c| (c.id.to_owned(), c.label.to_owned(), c.keys.iter().map(|k| (*k).to_owned()).collect()))
            .collect();
        assert_eq!(ours, web);
    }

    #[test]
    fn overrides_and_workflowy_keys_go_over_the_defaults() {
        let none = BTreeMap::new();
        let km = Keymap::new(&none, false, false);
        assert_eq!(km.resolve("Mod+."), Some("collapse"));
        assert_eq!(km.keys_for("redo"), ["Mod+Y", "Mod+Shift+Z"]);
        let wf = Keymap::new(&none, true, true);
        assert_eq!(wf.keys_for("collapse"), ["Ctrl+Up"]);
        assert_eq!(wf.resolve("Alt+Shift+Up"), Some("swap-up"));
        // Alt+Left is also "back": the WorkFlowy key wins it (as on the web), unless the user gave it to Back
        assert_eq!(wf.resolve("Alt+Left"), Some("zoom-out"));
        assert_eq!(wf.resolve("Alt+Right"), Some("zoom-in"));
        assert!(wf.keys_for("back").is_empty());
        let mut b = BTreeMap::new();
        b.insert("back".to_owned(), vec!["Alt+Left".to_owned()]);
        let wb = Keymap::new(&b, true, true);
        assert_eq!(wb.resolve("Alt+Left"), Some("back"));
        assert_eq!(wb.resolve("Alt+Right"), Some("zoom-in"));
        let mut o = BTreeMap::new();
        o.insert("collapse".to_owned(), vec!["Mod+Shift+K".to_owned()]);
        let km = Keymap::new(&o, true, false);
        assert_eq!(km.keys_for("collapse"), ["Mod+Shift+K"]);
        assert_eq!(km.resolve("Mod+."), None);
    }

    #[test]
    fn recording_a_key_takes_it_from_its_command() {
        let none = BTreeMap::new();
        let km = Keymap::new(&none, false, false);
        let o = km.with_key(&none, "toggle-numbered", "Mod+.");
        assert_eq!(o["collapse"], Vec::<String>::new());
        assert_eq!(o["toggle-numbered"], ["Mod+Shift+X", "Mod+."]);
        let km = Keymap::new(&o, false, false);
        assert_eq!(km.resolve("Mod+."), Some("toggle-numbered"));
        let o = km.without_key(&o, "toggle-numbered", "Mod+Shift+X");
        assert_eq!(Keymap::new(&o, false, false).keys_for("toggle-numbered"), ["Mod+."]);
    }

    #[test]
    fn combos_are_shown_and_bound_as_nsh_keystrokes() {
        assert_eq!(pretty("Mod+Shift+.", true), "⌘⇧.");
        assert_eq!(pretty("Mod+Shift+Backspace", false), "Ctrl+Shift+⌫");
        for c in COMMANDS {
            for k in c.keys {
                assert!(keystroke(k, false).is_some() && keystroke(k, true).is_some(), "{k}");
            }
        }
        for key in recordable_keys() {
            for apple in [true, false] {
                for c in combos_of(&key, apple) {
                    assert!(keystroke(&c, apple).is_some(), "{c}");
                }
            }
        }
        assert!(keystroke("Mod+Ctrl+A", false).is_none());
        assert_eq!(combos_of("K", false).len(), 7);
        assert_eq!(combos_of("F2", false).len(), 8);
    }
}
