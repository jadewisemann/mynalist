//! Links between items, as the web app's (`apps/web/src/ui/popups.ts`, `paintBacklinks` / `showReferences` in
//! `apps/web/src/main.ts`): typing `[[` in an item suggests the items of every document (`search::fuzzy_filter`
//! over their plain text, the open document's first) and picking one writes a link to it; typing `#` or `@`
//! suggests the tags in use. A press on a link to an item opens it (`refs::parse_item_url`). With the
//! `show_backlinks` pref, the items linking to the zoomed item are listed under the document, and a row linked
//! to shows how many link to it (a press lists them).

use std::rc::Rc;

use nsh_core::keymap::When;
use nsh_core::prelude::*;
use nsh_core::LineHeight;
use outliner_core::inline::inline_to_plain;
use outliner_core::refs::{backlink_counts, find_backlinks, parse_item_url, DocTree};
use outliner_core::search::fuzzy_filter;
use outliner_core::{item, text16, NodeId, ROOT};

use crate::dialog::{self, Button};
use crate::find;
use crate::sidebar::ui_text;
use crate::store;
use crate::style::{self, color, num};

/// The most suggestions shown (`MAX_TAGS`, `MAX_LINKS`).
const MAX_TAGS: usize = 8;
const MAX_LINKS: usize = 10;

/// An item a `[[` link can point to (`LinkCandidate`).
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub doc: String,
    pub id: NodeId,
    /// The item's source text.
    pub text: String,
    /// "Doc › parent › …"
    pub path: String,
}

/// The internal URL of an item (`itemUrl`): the web's `#d=<doc>&z=<item>`, without an origin (the native app
/// has no address of its own; the web takes a URL starting with `#` as its own too).
pub fn item_url(doc: &str, id: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => (b as char).to_string(),
                b' ' => "+".to_owned(),
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    if id == ROOT { format!("#d={}", enc(doc)) } else { format!("#d={}&z={}", enc(doc), enc(id)) }
}

/// The items of every document for `[[` links (`linkCandidates`): the open document's first, each document's in
/// document order; items with some text, not the documents' titles.
pub fn candidates(s: &store::State) -> Vec<Candidate> {
    let mut docs = s.files.clone();
    docs.sort_by_key(|d| *d != s.doc);
    let mut out = Vec::new();
    for d in docs {
        let Some(t) = s.tree_of(&d) else { continue };
        let title = s.title(&d);
        let mut stack: Vec<NodeId> = t.get(ROOT).map(|n| n.children.iter().rev().cloned().collect()).unwrap_or_default();
        while let Some(id) = stack.pop() {
            let Some(n) = t.get(&id) else { continue };
            stack.extend(n.children.iter().rev().cloned());
            let text = item::content(Some(n));
            if text.trim().is_empty() {
                continue;
            }
            let crumb = find::breadcrumb(t, &id, ROOT);
            let path = if crumb.is_empty() { title.clone() } else { format!("{title} › {crumb}") };
            out.push(Candidate { doc: d.clone(), id, text: text.to_owned(), path });
        }
    }
    out
}

/// A press on a link (`openLink`): a link to an item of a local document opens it (zoomed in on the item); any
/// other opens in the browser.
pub fn open(url: &str) {
    if let Some(u) = parse_item_url(url) {
        if store::peek(|s| s.files.contains(&u.doc)) {
            go(&u.doc, &u.zoom);
            return;
        }
    }
    drop(open_url(url.to_owned()));
}

/// Open document `doc` zoomed in on `zoom` (the web's route), nothing edited.
pub fn go(doc: &str, zoom: &str) {
    store::edit(|s| {
        s.switch(doc);
        if s.tree().is_attached(zoom) {
            s.view.zoom = zoom.to_owned();
        }
        s.view.filter = None;
        s.editing = None;
        s.block = None;
        s.caret_rev += 1;
        s.reflow();
    });
}

// ---- suggestions ------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Tag,
    Link,
}

/// One suggestion (`Suggestion`): its label with the matched ranges (UTF-16), the line under it, the count.
#[derive(Debug, Clone, PartialEq)]
struct Hit {
    label: String,
    ranges: Vec<(usize, usize)>,
    sub: Option<String>,
    aside: Option<String>,
    /// A tag: the tag; a link: the candidate's index.
    pick: Pick,
}

#[derive(Debug, Clone, PartialEq)]
enum Pick {
    Tag(String),
    Link(Candidate),
}

/// The suggestions while a `#` / `@` or a `[[` is being typed (`AcState`).
#[derive(Clone, Default, PartialEq)]
struct Ac {
    open: Option<Kind>,
    id: NodeId,
    note: bool,
    /// Where the `#` / `@` or the `[[` starts, and the caret at the last input (the token ends there; UTF-16).
    start: usize,
    end: usize,
    /// The field's length at the last input: a caret moving past `end` without typing closes the list.
    len: usize,
    hits: Vec<Hit>,
    active: usize,
    /// The tags or the items, taken once when the list opens.
    tags: Option<Rc<Vec<(String, usize)>>>,
    links: Option<Rc<Vec<Candidate>>>,
}

fn ac() -> Signal<Ac> {
    global::<Ac>()
}

fn is_ws(c: Option<u16>) -> bool {
    c.is_none_or(|c| char::from_u32(u32::from(c)).is_some_and(char::is_whitespace))
}

/// Whether the suggestions show (tracked).
pub fn is_open() -> bool {
    ac().with(|a| a.open.is_some() && !a.hits.is_empty())
}

pub fn close() {
    if ac().with_untracked(|a| a.open.is_some()) {
        ac().set(Ac::default());
    }
}

/// Where the caret ends after `old` became `new` (the end of what changed).
fn caret_after(old: &[u16], new: &[u16]) -> usize {
    let prefix = old.iter().zip(new.iter()).take_while(|(a, b)| a == b).count();
    let suffix = old.iter().rev().zip(new.iter().rev()).take_while(|(a, b)| a == b).count();
    new.len() - suffix.min(old.len().min(new.len()) - prefix)
}

/// The input being edited, and where its caret is drawn in it (left, top, bottom): the list goes under the caret.
/// Where the caret of the input being edited is on the screen: its left, top and bottom (the list goes under it).
#[derive(Clone, Copy, Default, PartialEq)]
struct Caret(Option<(f32, f32, f32)>);

/// The edited input's caret moved to `b` (in the screen's coordinates, `CaretEvent::page_bounds`).
pub fn caret_at(b: Bounds<Points>) {
    if b.size.height.0 > 0.0 {
        global::<Caret>().set(Caret(Some((b.origin.x.0, b.origin.y.0, b.origin.y.0 + b.size.height.0))));
    }
}

/// The input being edited: until its caret is reported (or where the platform reports none), the list goes under
/// the input itself, where it is on the screen.
#[derive(Clone, Copy, Default)]
struct Field(Option<ElementRef>, u64);

impl PartialEq for Field {
    fn eq(&self, other: &Self) -> bool {
        self.1 == other.1
    }
}

/// A row's (or a note's) input appeared: its caret is not known yet.
pub fn register(input: ElementRef) {
    let n = global::<Field>().with_untracked(|f| f.1) + 1;
    global::<Field>().set(Field(Some(input), n));
    global::<Caret>().set(Caret(None));
    global::<Below>().set(Below(None));
}

/// The input's left, top and bottom on the screen, measured while the list shows and no caret is known.
#[derive(Clone, Copy, Default, PartialEq)]
struct Below(Option<(f32, f32, f32)>);

fn measure_field() {
    let Some(field) = global::<Field>().with_untracked(|f| f.0) else { return };
    spawn(async move {
        if let Some(m) = field.measure().await {
            let (x, y) = (m.page.x.0, m.page.y.0);
            global::<Below>().set(Below(Some((x, y, y + m.frame.size.height.0))));
        }
    });
}

/// Where the list goes: under the caret, or under the input.
fn anchor() -> (f32, f32, f32) {
    global::<Caret>().with(|c| c.0).or_else(|| global::<Below>().with(|b| b.0)).unwrap_or_default()
}

/// Typing in the row's input (`onTextInput`): `old` became `new` in `id`'s content (or note). A `#` or `@` after a
/// space starts the tag suggestions, `[[` after a space the item suggestions; while they show, the token typed
/// narrows them.
pub fn typed(id: &str, note: bool, old: &str, new: &str) {
    let (a, b): (Vec<u16>, Vec<u16>) = (old.encode_utf16().collect(), new.encode_utf16().collect());
    let c = caret_after(&a, &b);
    let cur = ac().get_untracked();
    if cur.open.is_some() && (cur.id != id || cur.note != note) {
        close();
    }
    if ac().with_untracked(|a| a.open.is_some()) {
        ac().update(|a| {
            a.end = c;
            a.len = b.len();
        });
        update(new, c);
    } else {
        let at = |k: usize| c.checked_sub(k).map(|i| b[i]);
        let ch = at(1);
        let kind = if (ch == Some(u16::from(b'#')) || ch == Some(u16::from(b'@'))) && is_ws(at(2)) {
            Some((Kind::Tag, c - 1))
        } else if ch == Some(u16::from(b'[')) && at(2) == Some(u16::from(b'[')) && is_ws(at(3)) {
            Some((Kind::Link, c - 2))
        } else {
            None
        };
        let Some((kind, start)) = kind else { return };
        ac().set(Ac { open: Some(kind), id: id.to_owned(), note, start, end: c, len: b.len(), ..Ac::default() });
        update(new, c);
    }
}

/// The input's caret moved (`onSelectionChange`): before the token, or past it without typing, the list closes.
pub fn caret_moved(id: &str, note: bool, at: u32) {
    let a = ac().get_untracked();
    if a.open.is_none() {
        return;
    }
    let at = at as usize;
    let min = if a.open == Some(Kind::Tag) { a.start + 1 } else { a.start + 2 };
    let len = store::peek(|s| {
        let n = s.tree().get(id);
        text16::len(if note { item::note(n) } else { item::content(n) })
    });
    if a.id != id || a.note != note || at < min || (at > a.end && len == a.len) {
        close();
    }
}

/// The list for the token from `start` to the caret `c` in `t` (`updateAc`); closed when it no longer is one.
fn update(t: &str, c: usize) {
    let a = ac().get_untracked();
    let Some(kind) = a.open else { return };
    let u: Vec<u16> = t.encode_utf16().collect();
    let slice = |x: usize, y: usize| String::from_utf16_lossy(&u[x.min(u.len())..y.min(u.len())]);
    match kind {
        Kind::Tag => {
            let q = slice(a.start, c);
            let body_ok = q.chars().skip(1).all(|ch| !ch.is_whitespace());
            let head_ok = q.starts_with('#') || q.starts_with('@');
            if c <= a.start || !head_ok || !body_ok || !is_ws(a.start.checked_sub(1).map(|i| u[i])) {
                close();
                return;
            }
            let tags = a.tags.clone().unwrap_or_else(|| Rc::new(tag_source()));
            let prefix = q.chars().next().unwrap_or('#');
            let body = q[prefix.len_utf8()..].to_lowercase();
            let pool: Vec<&(String, usize)> = tags
                .iter()
                .filter(|(tag, _)| tag.starts_with(prefix) && tag.to_lowercase() != q.to_lowercase())
                .collect();
            let rest_of = |tag: &str| tag[prefix.len_utf8()..].to_owned();
            let starts: Vec<&(String, usize)> =
                pool.iter().copied().filter(|(tag, _)| rest_of(tag).to_lowercase().starts_with(&body)).collect();
            let others: Vec<&(String, usize)> = pool.iter().copied().filter(|x| !starts.contains(x)).collect();
            let rest: Vec<&(String, usize)> = if body.is_empty() {
                Vec::new()
            } else {
                fuzzy_filter(&body, &others, |x| &x.0[prefix.len_utf8()..]).into_iter().map(|(x, _)| *x).collect()
            };
            let blen = text16::len(&body);
            let hits = starts
                .into_iter()
                .chain(rest)
                .take(MAX_TAGS)
                .map(|(tag, n)| Hit {
                    label: tag.clone(),
                    ranges: if blen > 0 && rest_of(tag).to_lowercase().starts_with(&body) { vec![(1, 1 + blen)] } else { Vec::new() },
                    sub: None,
                    aside: Some(n.to_string()),
                    pick: Pick::Tag(tag.clone()),
                })
                .collect();
            ac().update(|a| {
                a.tags = Some(tags);
                a.hits = hits;
                a.active = 0;
            });
        }
        Kind::Link => {
            let q = slice(a.start + 2, c);
            if c < a.start + 2 || slice(a.start, a.start + 2) != "[[" || q.contains(['\n', ']']) {
                close();
                return;
            }
            let links = a.links.clone().unwrap_or_else(|| {
                Rc::new(store::peek(|s| candidates(s).into_iter().filter(|x| x.id != a.id).collect()))
            });
            let plain: Vec<String> = links.iter().map(|x| inline_to_plain(&x.text, None)).collect();
            let ranked: Vec<(usize, Vec<(usize, usize)>)> = if q.trim().is_empty() {
                (0..links.len()).map(|i| (i, Vec::new())).collect()
            } else {
                let idx: Vec<(usize, String)> = plain.iter().cloned().enumerate().collect();
                fuzzy_filter(&q, &idx, |x| x.1.as_str())
                    .into_iter()
                    .map(|(x, m)| (x.0, m.ranges.iter().map(|r| (r[0], r[1])).collect()))
                    .collect()
            };
            let hits = ranked
                .into_iter()
                .take(MAX_LINKS)
                .map(|(i, ranges)| Hit {
                    label: plain[i].clone(),
                    ranges,
                    sub: Some(links[i].path.clone()),
                    aside: None,
                    pick: Pick::Link(links[i].clone()),
                })
                .collect();
            ac().update(|a| {
                a.links = Some(links);
                a.hits = hits;
                a.active = 0;
            });
        }
    }
}

/// The tags to suggest (`tagSource`): the open document's, or every document's with the
/// `global_tag_autocomplete` pref, most used first.
fn tag_source() -> Vec<(String, usize)> {
    let global = find::pref("global_tag_autocomplete");
    store::peek(|s| {
        let docs = if global { s.files.clone() } else { vec![s.doc.clone()] };
        s.tags_in(&docs)
    })
}

/// Up / Down while the list shows: the suggestion above or below (around the ends).
fn step(d: isize) {
    ac().update(|a| {
        let n = a.hits.len() as isize;
        if n > 0 {
            a.active = ((a.active as isize + d + n) % n) as usize;
        }
    });
}

/// Return / Tab, or a press on a suggestion (`pick`): the token replaced with the tag (and a space), or with a
/// link to the item, its title selected.
fn pick(i: usize) {
    let a = ac().get_untracked();
    close();
    let Some(hit) = a.hits.get(i) else { return };
    let text = store::peek(|s| {
        let n = s.tree().get(&a.id);
        if a.note { item::note(n) } else { item::content(n) }.to_owned()
    });
    let u: Vec<u16> = text.encode_utf16().collect();
    let edited = store::peek(|s| s.editing.as_ref().filter(|c| c.id == a.id && c.note == a.note).map(|c| c.at.max(c.end) as usize));
    let end = a.end.max(edited.unwrap_or(0)).min(u.len());
    match &hit.pick {
        Pick::Tag(tag) => {
            let spaced = u.get(end) == Some(&u16::from(b' '));
            let ins = if spaced { tag.clone() } else { format!("{tag} ") };
            let at = (a.start + text16::len(tag) + 1) as u32;
            crate::commands::replace_range(&a.id, a.note, a.start as u32, end as u32, &ins, at, at);
        }
        Pick::Link(it) => {
            let plain = inline_to_plain(&it.text, None);
            let squeezed = plain.split_whitespace().collect::<Vec<_>>().join(" ");
            let title: String = if squeezed.is_empty() { "Untitled".to_owned() } else { squeezed };
            let title: String = title.chars().flat_map(|c| if matches!(c, '[' | ']' | '\\') { vec!['\\', c] } else { vec![c] }).collect();
            let md = format!("[{title}]({})", item_url(&it.doc, &it.id));
            let s = (a.start + 1) as u32;
            crate::commands::replace_range(&a.id, a.note, a.start as u32, end as u32, &md, s, s + text16::len(&title) as u32);
        }
    }
}

/// The suggestions over the screen while they show (`.ac-popup`), below the caret (above it when there is more
/// room there); their keys go before the input's.
pub fn popup() -> impl IntoElement {
    // editing moved to another field, or stopped: the list closes
    let editing = computed(|| store::read(|s| s.editing.as_ref().map(|c| (c.id.clone(), c.note))));
    effect(move || {
        let e = editing.get();
        untracked(|| {
            let a = ac().get_untracked();
            if a.open.is_some() && e != Some((a.id.clone(), a.note)) {
                close();
            }
        });
    });
    // no caret known: the input is measured once the list shows (and again for a new input)
    effect(move || {
        let open = is_open();
        let reg = global::<Field>().with(|f| f.1);
        if open && reg > 0 && global::<Caret>().with_untracked(|c| c.0.is_none()) {
            untracked(measure_field);
        }
    });
    when(is_open, || {
        let opts = KeyOptions { when: When::Always, ..Default::default() };
        on_key_with("down", opts, |_| step(1));
        on_key_with("up", opts, |_| step(-1));
        on_key_with("enter", opts, |_| pick(ac().with_untracked(|a| a.active)));
        on_key_with("tab", opts, |_| pick(ac().with_untracked(|a| a.active)));
        on_key_with("escape", opts, |_| close());
        let size = signal((0.0_f32, 0.0_f32));
        let phone = style::phone();
        let item_y = if phone { num("space.step6") } else { num("space.ac_item_y") };
        let fs = style::efs();
        let line = fs * num("type.ui_line");
        let tiny = style::ui_sized("type.ui_tiny");
        let rows = dynamic(move || {
            let (hits, active) = ac().with(|a| (a.hits.clone(), a.active));
            view()
                .children(hits.into_iter().enumerate().map(|(i, h)| {
                    let mut label: Vec<AnyElement> = Vec::new();
                    let mut at = 0;
                    for &(s, e) in &h.ranges {
                        if s > at {
                            label.push(text(text16::slice(&h.label, at, s).to_owned()).into_element());
                        }
                        label.push(
                            text(text16::slice(&h.label, s, e).to_owned())
                                .font_weight(FontWeight(650))
                                .text_color(color::accent())
                                .into_element(),
                        );
                        at = e;
                    }
                    if at < text16::len(&h.label) {
                        label.push(text(text16::slice_from(&h.label, at).to_owned()).into_element());
                    }
                    let mut main = view().flex_row().items_end().gap(px(num("space.step8"))).child(
                        text("")
                            .flex_1()
                            .font_family(style::font())
                            .text_size(px(fs))
                            .line_height(LineHeight::Points(px(line)))
                            .text_color(color::fg())
                            .number_of_lines(1)
                            .children(label),
                    );
                    if let Some(n) = &h.aside {
                        main = main.child(ui_text(n.clone(), tiny).text_color(color::muted()).tabular_nums());
                    }
                    let mut row = nsh_base::button()
                        .accessibility_label(Some(h.label.clone().into()))
                        .selected(i == active)
                        .flex_col()
                        .items_stretch()
                        .py(px(item_y))
                        .px(px(num("space.step8")))
                        .on_press(move || pick(i))
                        .child(main);
                    if i == active {
                        row = row.bg(color::sel());
                    }
                    if let Some(sub) = &h.sub {
                        row = row.child(ui_text(sub.clone(), tiny).text_color(color::muted()).number_of_lines(1));
                    }
                    // the test id names a wrapper: a button takes none
                    view().test_id("suggestion").child(row).into_element()
                }))
                .into_element()
        });
        view()
            .test_id("suggestions")
            .absolute()
            .left(move || {
                let (w, _) = size.get();
                let x = anchor().0;
                px(x.min(window().width.0 - w - 4.0).max(4.0))
            })
            .top(move || {
                let (_, h) = size.get();
                let (_, top, bottom) = anchor();
                let gap = 4.0;
                let below = window().height.0 - bottom - gap - 4.0;
                let above = top - gap - 4.0;
                let down = h <= below || below >= above;
                px(if down { bottom + gap } else { (top - gap - h).max(4.0) })
            })
            .min_w(px(num("space.ac_min")))
            .max_w(move || px(num("space.ac_max").min(window().width.0 - num("space.step6"))))
            .py(px(num("space.step4")))
            .bg(color::bg())
            .border(px(num("space.border")))
            .border_color(color::chip_border())
            .rounded(px(num("radius.menu")))
            .shadow([BoxShadow {
                offset_y: px(num("space.step6")),
                blur: px(num("space.shadow_popup")),
                color: rgba(style::look().color("color.shadow_menu")).into(),
                ..Default::default()
            }])
            .on_layout(move |b: &Bounds<Points>| size.set((b.size.width.0, b.size.height.0)))
            .child(rows)
    })
}

// ---- backlinks ----------------------------------------------------------------------------------------------

/// Whether backlinks show (the `show_backlinks` pref, off by default).
pub fn backlinks_on() -> bool {
    find::pref("show_backlinks")
}

/// How many items link to each item of the open document (tracked), while backlinks show.
pub fn counts() -> std::collections::HashMap<NodeId, usize> {
    if !backlinks_on() {
        return Default::default();
    }
    store::read(|s| {
        let trees: Vec<DocTree<'_>> = s.files.iter().filter_map(|d| s.tree_of(d).map(|tree| DocTree { doc: d, tree })).collect();
        backlink_counts(&trees, &s.doc)
    })
}

fn backlinks_of(id: &str) -> Vec<(String, NodeId)> {
    store::read(|s| {
        let trees: Vec<DocTree<'_>> = s.files.iter().filter_map(|d| s.tree_of(d).map(|tree| DocTree { doc: d, tree })).collect();
        find_backlinks(&trees, &s.doc, id).into_iter().map(|b| (b.doc, b.id)).collect()
    })
}

/// A row of the items linking somewhere (`refRow`): the document and the path above the text; a press opens it.
fn ref_row(doc: &str, id: &str) -> AnyElement {
    let (crumb, src) = store::peek(|s| {
        let t = s.tree_of(doc);
        let crumb = t.map(|t| find::breadcrumb(t, id, ROOT)).unwrap_or_default();
        let title = s.title(doc);
        let path = if crumb.is_empty() { title } else { format!("{title} › {crumb}") };
        (path, item::content(t.and_then(|t| t.get(id))).to_owned())
    });
    find::ref_row(doc, id, crumb, &src, "", false, &[]).into_element()
}

/// Under the document, with the `show_backlinks` pref: the items that link to the zoomed item (`.backlinks`).
pub fn backlinks() -> impl IntoElement {
    dynamic(|| {
        if !backlinks_on() {
            return view().into_element();
        }
        let zoom = store::read(|s| s.view.zoom.clone());
        let refs = backlinks_of(&zoom);
        if refs.is_empty() {
            return view().into_element();
        }
        let (pad_l, pad_r) = style::doc_pad();
        view()
            .test_id("backlinks")
            .ml(px(pad_l))
            .mr(px(pad_r))
            .border_t(px(num("space.border")))
            .border_color(color::line())
            .pt(px(num("space.step7")))
            .child(
                ui_text("This item is referenced in:", style::ui_sized("type.ui_small"))
                    .text_color(color::muted())
                    .mb(px(num("space.step4"))),
            )
            .children(refs.iter().map(|(d, id)| ref_row(d, id)))
            .into_element()
    })
}

/// The count of the items linking to a row's item (`.o-backlinks`), left of its bullet; a press lists them.
pub fn badge(id: &str, n: usize, row_h: f32, left: f32) -> impl IntoElement {
    let h = num("space.backlinks_h");
    let id = id.to_owned();
    let button = nsh_base::button()
        .accessibility_label(Some(format!("{n} backlinks").into()))
        .min_w(px(h))
        .h(px(h))
        .px(px(num("space.step4")))
        .rounded(px(num("radius.backlinks")))
        .bg(color::chip())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .items_center()
        .justify_center()
        .on_press(move || references(&id))
        .child(ui_text(n.to_string(), (num("type.ui_micro"), num("space.step9"))).text_color(color::muted()));
    view()
        .test_id("backlink-count")
        .absolute()
        .left(px(left - num("space.backlinks_x")))
        .top(px((row_h - h) / 2.0 + style::pad()))
        .child(button)
}

/// The items linking to item `id` in a dialog (`showReferences`); a press on one opens it.
pub fn references(id: &str) {
    let id = id.to_owned();
    dialog::show(move || {
        let refs = untracked(|| backlinks_of(&id));
        let body: AnyElement = if refs.is_empty() {
            dialog::label("No other item links to this one yet. Link to it with [[ in any item, or \"Get link\".")
                .into_element()
        } else {
            view()
                .children(refs.iter().map(|(d, i)| {
                    let (d, i) = (d.clone(), i.clone());
                    view().on_press(dialog::close).child(ref_row(&d, &i)).into_element()
                }))
                .into_element()
        };
        dialog::modal("references", "References", body, vec![Button { label: "Close", primary: true, run: None }])
    });
}
