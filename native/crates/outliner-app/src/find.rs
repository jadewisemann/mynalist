//! Search, as the web app's (`openSearch` / `runSearch` in `apps/web/src/main.ts`, `apps/web/src/ui/results.ts`):
//! the search box above the document filters its items as the query is typed (`search::compile_query`,
//! `search_tree`; the matches and their ancestors are the view's filter) and marks the matched words in the
//! matching items; Escape or its close button ends it and puts the caret back. While its query is typed, the
//! search tips offer the flat search (the matches of the document as a list) and the search in all documents,
//! which show the results screen instead of the document. The search and replace dialog (`replace.ts`) is here
//! too.

use std::collections::HashSet;
use std::rc::Rc;

use nsh_core::keymap::When;
use nsh_core::prelude::*;
use nsh_core::LineHeight;
use outliner_core::commands::{sort_comparator, SortMode, SORT_LABELS};
use outliner_core::search::{compile_query, search_tree};
use outliner_core::{item, refs, NodeId, Tree, ROOT};

use crate::dialog::{self, Button};
use crate::icons::icon;
use crate::sidebar::ui_text;
use crate::store::{self, Caret, Mark};
use crate::style::{self, color, num};

/// The results screens (`ResultsState`): the flat search lists the matches under the zoom root of the document
/// that was open; the global one those of every document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Flat,
    Global,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Results {
    pub mode: Mode,
    pub q: String,
    pub sort: Option<SortMode>,
    pub doc: String,
    pub zoom: NodeId,
}

/// The search: the box, its query, what the rows mark, and the results screen while one shows.
#[derive(Clone, Default, PartialEq)]
struct Find {
    /// The search box shows.
    open: bool,
    query: String,
    /// The search input has the focus (the search tips show only then).
    focused: bool,
    /// Bumped to give the search input the focus and select its text.
    focus: u64,
    /// The query's words to mark (`Matcher::terms`), in the items that match (`hits`; not in their ancestors).
    terms: Vec<String>,
    hits: Rc<HashSet<NodeId>>,
    /// Where the caret was when the box opened: closing it puts it back.
    pre: Option<Mark>,
    results: Option<Results>,
}

fn find() -> Signal<Find> {
    global::<Find>()
}

/// Prefs set for this run, over the ones kept on the device (a fixture's).
#[derive(Clone, Default, PartialEq)]
struct Prefs(Vec<(String, bool)>);

/// Set a pref for this run (a fixture's state), whatever the device keeps.
pub fn set_pref(key: &str, on: bool) {
    global::<Prefs>().update(|p| {
        p.0.retain(|(k, _)| k != key);
        p.0.push((key.to_owned(), on));
    });
}

/// An on/off setting (`settings::FLAGS`: the web's name and default), or this run's value for it.
pub fn pref(key: &str) -> bool {
    if let Some(on) = global::<Prefs>().with(|p| p.0.iter().find(|(k, _)| k == key).map(|(_, v)| *v)) {
        return on;
    }
    crate::settings::get().flag(key)
}

/// The words to mark in item `id`'s text (tracked): the query's, while the item matches it.
pub fn highlight(id: &str) -> Vec<String> {
    find().with(|f| if f.hits.contains(id) { f.terms.clone() } else { Vec::new() })
}

/// Whether the search box shows (tracked).
pub fn is_open() -> bool {
    find().with(|f| f.open)
}

/// Where the selection is now (the caret or the selected items).
fn selection() -> Option<Mark> {
    store::peek(|s| match (&s.block, &s.editing) {
        (Some(b), _) => Some(Mark::Block(b.clone())),
        (None, Some(c)) => Some(Mark::Caret(c.clone())),
        _ => None,
    })
}

/// The top bar's search button, Mod+F (`openSearch`): the box shows with the focus on its query (`q` when
/// given, searched at once); the results screen gives way to the document.
pub fn open(q: Option<&str>) {
    let pre = selection();
    find().update(|f| {
        if !f.open || !f.focused {
            f.pre = pre;
        }
        if let Some(r) = f.results.take() {
            if q.is_none() && r.mode == Mode::Flat {
                f.query = r.q;
            }
        }
        f.open = true;
        if let Some(q) = q {
            f.query = q.to_owned();
        }
        f.focus += 1;
    });
    // the input does not keep the caret's place while the box has the focus
    store::edit(|s| {
        s.editing = None;
        s.block = None;
        s.caret_rev += 1;
    });
    run(false);
}

/// Escape in the box, its close button (`closeSearch`): no query, every item shows again, the caret back where
/// it was.
pub fn close() {
    let pre = find().with_untracked(|f| f.pre.clone());
    find().update(|f| {
        f.open = false;
        f.query.clear();
        f.focused = false;
        f.pre = None;
    });
    run(false);
    crate::commands::restore(pre);
}

/// The query typed: the box's input reports it.
fn typed(q: &str) {
    find().update(|f| f.query = q.to_owned());
    run(true);
}

/// Filter the open document by the query (`runSearch`): the matches and their ancestors show; with no query,
/// everything. `typing`: the last word may still grow, so it matches as a prefix.
fn run(typing: bool) {
    let q = find().with_untracked(|f| f.query.clone());
    let (terms, hits) = store::edit(|s| {
        let out = if q.trim().is_empty() {
            s.view.filter = None;
            (Vec::new(), HashSet::new())
        } else {
            // untrimmed: a trailing space ends the last word, which is then no longer a prefix
            let m = compile_query(&q, store::now(), typing);
            let res = search_tree(s.tree(), &m, Some(&s.view.zoom), Some(s.view.show_checked));
            s.view.filter = Some(res.visible);
            (m.terms, res.matches.into_iter().collect())
        };
        s.reflow();
        out
    });
    find().update(|f| {
        f.terms = terms;
        f.hits = Rc::new(hits);
    });
}

/// Shift+Return in the box, the tips' "Flat search": the matches of the document as a list.
fn flat() {
    let q = find().with_untracked(|f| f.query.trim().to_owned());
    if q.is_empty() {
        return;
    }
    show_results(Mode::Flat, q);
}

/// Mod+Return in the box, the tips' "Search in all documents".
fn global_search() {
    let q = find().with_untracked(|f| f.query.trim().to_owned());
    show_results(Mode::Global, q);
}

fn show_results(mode: Mode, q: String) {
    let (doc, zoom) = store::peek(|s| (s.doc.clone(), s.view.zoom.clone()));
    find().update(|f| {
        f.open = false;
        f.focused = false;
        f.query.clear();
        f.results = Some(Results { mode, q, sort: None, doc, zoom });
    });
    run(false);
    store::edit(|s| {
        s.editing = None;
        s.block = None;
        s.caret_rev += 1;
    });
}

/// "Go back [Esc]" on the results screen: the document again, the flat search's query in the box.
fn back() {
    let r = find().with_untracked(|f| f.results.clone());
    let Some(r) = r else { return };
    find().update(|f| f.results = None);
    if r.mode == Mode::Flat {
        find().update(|f| {
            f.open = true;
            f.query = r.q;
        });
        run(false);
    }
}

/// A press on a result, a backlink (`revealItem`): its document opened, the search ended, the item edited at its
/// start; the zoom stays when the item is under it.
pub fn reveal(doc: &str, id: &str) {
    find().update(|f| {
        f.open = false;
        f.focused = false;
        f.query.clear();
        f.results = None;
        f.pre = None;
    });
    run(false);
    store::edit(|s| {
        let same = s.doc == doc;
        s.switch(doc);
        let Some(t) = s.tree_of(doc) else { return };
        if !t.is_attached(id) {
            return;
        }
        if !(same && t.ancestors(id).contains(&s.view.zoom)) {
            s.view.zoom = ROOT.to_owned();
        }
        s.view.filter = None;
        s.reflow();
        s.block = None;
        s.editing = Some(Caret::text(id, 0));
        s.caret_rev += 1;
    });
}

/// Tab in the box (`goto-first`): the first row edited, at its start.
fn goto_first() {
    if let Some(first) = store::peek(|s| s.rows.first().map(|r| r.id.clone())) {
        crate::commands::start_editing(&first, Some(0));
    }
}

/// The document's place: the search box over the document (`.doc-tools`), the outline or the results screen,
/// and the search tips over them.
pub fn doc_area() -> impl IntoElement {
    // another document opened: the search of the one before ends (the web's route has no query then)
    let doc = computed(|| store::read(|s| s.doc.clone()));
    let seen = std::cell::RefCell::new(doc.get_untracked());
    effect(move || {
        let now = doc.get();
        if now == *seen.borrow() {
            return;
        }
        *seen.borrow_mut() = now;
        if find().with_untracked(|f| f.open) {
            untracked(|| {
                find().update(|f| {
                    f.open = false;
                    f.query.clear();
                    f.focused = false;
                    f.pre = None;
                });
                run(false);
            });
        }
    });
    let results = computed(|| find().with(|f| f.results.is_some()));
    view()
        .flex_1()
        .child(when(is_open, search_box))
        .child(dynamic(move || {
            if results.get() {
                results_view().into_element()
            } else {
                crate::outline::outline().into_element()
            }
        }))
        .child(when(|| find().with(|f| f.open && f.focused && !f.query.trim().is_empty()) && pref("show_search_tips"), tips))
}

/// The search box (`.search-box` in `.doc-tools`): a search icon, the query, the bookmark button (drawn: no
/// bookmarks yet) and the close button; as wide as the document on phones.
fn search_box() -> impl IntoElement {
    let field = element_ref();
    let phone = style::phone();
    let opts = KeyOptions { when: When::Always, ..Default::default() };
    let focused = || find().with_untracked(|f| f.focused);
    on_key_with("escape", opts, move |_| {
        if focused() {
            close();
        }
    });
    on_key_with("enter", opts, move |_| {
        if focused() {
            run(false);
        }
    });
    on_key_with("shift+enter", opts, move |_| {
        if focused() {
            flat();
        }
    });
    on_key_with("mod+enter", opts, move |_| {
        if focused() {
            global_search();
        }
    });
    on_key_with("tab", opts, move |_| {
        if focused() {
            goto_first();
        }
    });
    // the focus, and the query selected, when the box is asked for again
    let seen = std::cell::Cell::new(0_u64);
    effect(move || {
        let rev = find().with(|f| f.focus);
        if rev != seen.get() {
            seen.set(rev);
            field.focus();
            let len = find().with_untracked(|f| crate::commands::utf16_len(&f.query));
            field.set_selection(0, len);
        }
    });
    let size = style::efs();
    let button = |name: &'static str, label: &'static str, run: fn()| {
        nsh_base::button()
            .h(px(num("space.topbar_tool")))
            .min_w(px(num("space.topbar_tool_min")))
            .px(px(num("space.topbar_tool_pad")))
            .rounded(px(num("radius.topbar_tool")))
            .items_center()
            .justify_center()
            .text_color(color::topbar_fg())
            .accessibility_label(Some(label.into()))
            .on_press(run)
            .child(icon(name, num("space.topbar_icon")))
    };
    let input = nsh_base::input()
        .element_ref(&field)
        .test_id("search-input")
        .flex_1()
        .value(|| find().with(|f| f.query.clone()))
        .placeholder("Search… (is:completed, #tag, edited:3d)")
        .auto_focus(true)
        .auto_correct(false)
        .p_0()
        .border(px(0.0))
        .bg(AdaptiveColor::TRANSPARENT)
        .font_family(style::font())
        .text_size(px(size))
        .line_height(LineHeight::Points(px(size * num("type.ui_line"))))
        .text_color(color::fg())
        .on_focus(|| find().update(|f| f.focused = true))
        .on_blur(|| find().update(|f| f.focused = false))
        .on_change(|t: &str| typed(t));
    let bar = view()
        .test_id("search-box")
        .flex_row()
        .items_center()
        .gap(px(num("space.step4")))
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.block")))
        .pl(px(num("space.step6")))
        .pr(px(num("space.step4")))
        .h(px(num("space.step12")))
        .child(view().text_color(color::muted()).child(icon("search", num("space.topbar_icon"))))
        .child(input)
        .child(button("star", "Bookmark this search", || {}))
        .child(button("close", "Close search", close));
    let bar = if phone {
        bar.flex_1()
    } else {
        bar.w(|| px(num("space.search_box").min(window().width.0 * num("space.search_vw"))))
            .mr(px(num("space.search_box_right")))
    };
    view()
        .test_id("doc-tools")
        .flex_row()
        .justify_end()
        .pt(px(num("space.step5")))
        .px(px(num("space.step8")))
        .child(bar)
}

/// The search tips (`.search-tips`) under the box while its query is typed: the flat search, the search in all
/// documents, and the operators.
fn tips() -> impl IntoElement {
    let (size, line) = style::ui_sized("type.ui_small");
    let tiny = style::ui_sized("type.ui_tiny");
    let tip = |name: &'static str, keys: &'static str, run: fn()| {
        nsh_base::button()
            .accessibility_label(Some(name.into()))
            .flex_row()
            .justify_between()
            .py(px(num("space.ac_item_y")))
            .px(px(num("space.step8")))
            .active(|s| s.bg(color::hover()))
            .on_press(run)
            .child(ui_text(name, (size, line)).text_color(color::fg()))
            .child(ui_text(keys, tiny).text_color(color::muted()))
    };
    view()
        .test_id("search-tips")
        .absolute()
        .top(px(num("space.search_tips_top")))
        .right(px(num("space.search_tips_right")))
        .w(|| px(num("space.search_box").min(window().width.0 * num("space.search_vw"))))
        .bg(color::bg())
        .border(px(num("space.border")))
        .border_color(color::chip_border())
        .rounded(px(num("radius.menu")))
        .py(px(num("space.step4")))
        .shadow([BoxShadow {
            offset_y: px(num("space.step5")),
            blur: px(num("space.step11")),
            color: rgba(style::look().color("color.shadow_tips")).into(),
            ..Default::default()
        }])
        .child(tip("Flat search", "Shift+Enter", flat))
        .child(tip("Search in all documents", "Ctrl+Enter", global_search))
        .child(
            ui_text(
                "Operators: is:completed, -is:completed, #tag, \"exact phrase\", edited:4d, created:1w, has:date, due:today, parent:text…",
                tiny,
            )
            .text_color(color::muted())
            .pt(px(num("space.step5")))
            .px(px(num("space.step8")))
            .pb(px(num("space.step4")))
            .mt(px(num("space.step4")))
            .border_t(px(num("space.border")))
            .border_color(color::line()),
        )
}

// ---- the results screen ----------------------------------------------------------------------------------

/// `breadcrumb`: the texts of an item's ancestors below `stop`, each cut at 40 characters, joined by ` › `.
pub fn breadcrumb(t: &Tree, id: &str, stop: &str) -> String {
    let anc = t.ancestors(id);
    let from = anc.iter().position(|a| a == stop).map_or(0, |i| i + 1);
    anc[from..]
        .iter()
        .map(|a| item::content(t.get(a)).chars().take(40).collect::<String>())
        .collect::<Vec<_>>()
        .join(" › ")
}

/// One document's results.
struct Group {
    doc: String,
    title: String,
    ids: Vec<NodeId>,
}

/// The results of `r`: the groups that have some, the matches shown, and those the checked filter hid.
fn gather(s: &store::State, r: &Results) -> (Vec<Group>, usize, usize) {
    let m = compile_query(&r.q, store::now(), false);
    let docs: Vec<String> = match r.mode {
        Mode::Global => s.files.clone(),
        Mode::Flat => vec![r.doc.clone()],
    };
    let (mut total, mut filtered) = (0, 0);
    let mut groups = Vec::new();
    for d in docs {
        let Some(t) = s.tree_of(&d) else { continue };
        let scope = if r.mode == Mode::Flat { r.zoom.as_str() } else { ROOT };
        // the native app has no per-document "show checked" yet: the open document's view, else shown
        let show_checked = if d == s.doc { s.view.show_checked } else { true };
        let shown = search_tree(t, &m, Some(scope), Some(show_checked)).matches;
        let all = if m.empty { shown.len() } else { search_tree(t, &m, Some(scope), Some(true)).matches.len() };
        filtered += all - shown.len();
        if shown.is_empty() {
            continue;
        }
        let mut ids = shown;
        if let Some(cmp) = r.sort.and_then(|mode| sort_comparator(t, mode)) {
            ids.sort_by(|a, b| cmp(a, b));
        }
        total += ids.len();
        groups.push(Group { title: s.title(&d), doc: d, ids });
    }
    (groups, total, filtered)
}

fn set_sort(sort: Option<SortMode>) {
    find().update(|f| {
        if let Some(r) = &mut f.results {
            r.sort = sort;
        }
    });
}

/// The results screen (`ResultsView`): its head (the global one has its own query), the count, the sort and the
/// way back, then the matches (each under its ancestors' path, with its note), in cards per document for the
/// global search.
fn results_view() -> impl IntoElement {
    on_key("escape", |_| back());
    let r = computed(|| find().with(|f| f.results.clone()));
    let (pad_l, pad_r) = style::doc_pad();
    let head = dynamic(move || {
        let Some(r) = r.get() else { return view().into_element() };
        let size = num("type.results_title");
        let title = match r.mode {
            Mode::Flat => format!("Search results for \"{}\" in \"{}\"", r.q, store::peek(|s| s.title(&r.doc))),
            Mode::Global => "Search everywhere".to_owned(),
        };
        let h2 = ui_text(title, (size, size * num("type.ui_line")))
            .text_color(color::fg())
            .font_weight(FontWeight(400))
            .mb(px(num("space.step7")));
        let mut v = view().test_id("results-head").child(h2);
        if r.mode == Mode::Global {
            let (big, _) = style::ui_sized("type.ui_large");
            v = v.child(
                dialog::input("results-input", "Search everywhere…")
                    .value(r.q.clone())
                    .text_size(px(big))
                    .py(px(num("space.step6")))
                    .px(px(num("space.step7")))
                    .mb(px(num("space.step6")))
                    .on_submit(|q: &str| {
                        let q = q.to_owned();
                        find().update(|f| {
                            if let Some(r) = &mut f.results {
                                r.q = q;
                            }
                        })
                    })
                    .on_change(|q: &str| {
                        let q = q.to_owned();
                        find().update(|f| {
                            if let Some(r) = &mut f.results {
                                r.q = q;
                            }
                        })
                    }),
            );
        }
        v.into_element()
    });
    let body = dynamic(move || {
        let Some(r) = r.get() else { return view().into_element() };
        let (groups, total, filtered) = store::read(|s| gather(s, &r));
        let mut count = if total > 0 { format!("Found {total} match{}:", if total == 1 { "" } else { "es" }) } else { String::new() };
        if r.mode == Mode::Flat && filtered > 0 && total > 0 {
            count += &format!(" ({filtered} hidden by your filter settings)");
        }
        let bar = view()
            .test_id("results-bar")
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(num("space.step7")))
            .mb(px(num("space.step7")))
            .child(dialog::label(count).text_color(color::muted()))
            .child(view().flex_1())
            .child(dialog::label("Sort by").text_color(color::muted()))
            .child(sort_choice(r.sort))
            .child(dialog::btn("Go back [Esc]", false, back));
        let terms = compile_query(&r.q, store::now(), false).terms;
        let scope = if r.mode == Mode::Flat { r.zoom.clone() } else { ROOT.to_owned() };
        let list: AnyElement = if total == 0 {
            let msg = if filtered > 0 {
                format!(
                    "No results found. {filtered} match{} been filtered out by your filter settings. Append \"is:completed\" to your search to include checked items.",
                    if filtered == 1 { " has" } else { "es have" }
                )
            } else if !r.q.is_empty() {
                "No results found.".to_owned()
            } else {
                "Type to search all documents.".to_owned()
            };
            dialog::label(msg).text_color(color::muted()).my(px(num("space.step12"))).test_id("results-empty").into_element()
        } else if r.mode == Mode::Flat {
            let g = &groups[0];
            view()
                .children(g.ids.iter().map(|id| result_row(&g.doc, id, &scope, &terms).into_element()))
                .into_element()
        } else {
            view()
                .children(groups.iter().map(|g| {
                    view()
                        .test_id("results-card")
                        .border(px(num("space.border")))
                        .border_color(color::line())
                        .rounded(px(num("radius.menu")))
                        .py(px(num("space.step5")))
                        .px(px(num("space.step7")))
                        .mb(px(num("space.step8")))
                        .child({
                            let (s, l) = style::ui_sized("type.ui_head");
                            ui_text(g.title.clone(), (s, l))
                                .text_color(color::fg())
                                .font_weight(FontWeight(600))
                                .pt(px(num("space.step4")))
                                .pb(px(num("space.step5")))
                                .mb(px(num("space.step4")))
                                .border_b(px(num("space.border")))
                                .border_color(color::line())
                        })
                        .children(g.ids.iter().map(|id| result_row(&g.doc, id, &scope, &terms).into_element()))
                        .into_element()
                }))
                .into_element()
        };
        view().child(bar).child(list).into_element()
    });
    scroll()
        .test_id("results")
        .flex_1()
        .bg(color::bg())
        .child(view().pt(px(num("space.doc_top"))).pl(px(pad_l)).pr(px(pad_r)).child(head).child(body))
}

/// The sort of the results (the web's `<select>`, its options in view as the settings' choices are).
fn sort_choice(on: Option<SortMode>) -> impl IntoElement {
    let opts: Vec<(Option<SortMode>, &'static str)> = std::iter::once((None, "None"))
        .chain(SORT_LABELS.iter().filter(|(m, _)| *m != SortMode::Reverse).map(|(m, l)| (Some(*m), *l)))
        .collect();
    let (size, line) = style::ui_sized("type.ui_small");
    view().flex_row().flex_wrap().gap(px(num("space.step2"))).children(opts.into_iter().map(move |(m, l)| {
        let chosen = m == on;
        nsh_base::button()
            .accessibility_label(Some(format!("Sort by: {l}").into()))
            .selected(chosen)
            .px(px(num("space.step4")))
            .py(px(num("space.step2")))
            .border(px(num("space.border")))
            .border_color(if chosen { color::accent() } else { color::chip_border() })
            .bg(if chosen { color::sel() } else { color::bg() })
            .rounded(px(num("radius.code")))
            .on_press(move || set_sort(m))
            .child(ui_text(l, (size, line)).text_color(color::fg()))
            .into_element()
    }))
}

/// A result (`.res-item`): its path above it, its text with the query's words marked, its note; a press opens it.
pub fn result_row(doc: &str, id: &str, stop: &str, terms: &[String]) -> impl IntoElement {
    let (crumb, src, note, checked) = store::peek(|s| {
        let t = s.tree_of(doc);
        let n = t.and_then(|t| t.get(id));
        (
            t.map(|t| breadcrumb(t, id, stop)).unwrap_or_default(),
            item::content(n).to_owned(),
            item::note(n).to_owned(),
            item::is_checked(n),
        )
    });
    ref_row(doc, id, crumb, &src, &note, checked, terms)
}

/// A row of results or references: the path (`.res-crumb`), the text (`.res-text`), the note (`.res-note`).
pub fn ref_row(doc: &str, id: &str, crumb: String, src: &str, note: &str, checked: bool, terms: &[String]) -> impl IntoElement {
    let (big, big_line) = style::ui_sized("type.ui_large");
    let fg = if checked { color::check() } else { color::fg() };
    let look = crate::inline::Look::new(big, store::now()).colored(fg, checked).marking(terms, src);
    let mut text_el = text("")
        .font_family(style::font())
        .text_size(px(big))
        .line_height(LineHeight::Points(px(big_line)))
        .text_color(fg)
        .children(crate::inline::spans(&outliner_core::inline::parse_inline(src), &look));
    if checked {
        text_el = text_el.line_through();
    }
    let (doc_s, id_s) = (doc.to_owned(), id.to_owned());
    let mut row = nsh_base::button()
        .accessibility_label(Some(outliner_core::inline::inline_to_plain(src, None).into()))
        .flex_col()
        .items_start()
        .py(px(num("space.step5")))
        .px(px(num("space.step6")))
        .rounded(px(num("radius.block")))
        .active(|s| s.bg(color::hover()))
        .on_press(move || reveal(&doc_s, &id_s));
    if !crumb.is_empty() {
        row = row.child(
            ui_text(crumb, style::ui_sized("type.ui_tiny")).text_color(color::muted()).mb(px(num("space.step1"))),
        );
    }
    row = row.child(text_el);
    if !note.is_empty() {
        let (s, l) = style::ui_sized("type.ui_small");
        let look = crate::inline::Look::new(s, store::now()).colored(color::note(), false).marking(terms, note);
        row = row.child(
            text("")
                .font_family(style::font())
                .text_size(px(s))
                .line_height(LineHeight::Points(px(l)))
                .text_color(color::note())
                .children(crate::inline::spans(&outliner_core::inline::parse_inline(note), &look)),
        );
    }
    // the test id names a wrapper: a button takes none
    view().test_id("result").child(row)
}

// ---- search and replace --------------------------------------------------------------------------------

#[derive(Clone, Default, PartialEq)]
struct Replace {
    find: String,
    with: String,
    match_case: bool,
}

fn replace_state() -> Signal<Replace> {
    global::<Replace>()
}

/// The search and replace dialog (`searchReplaceDialog`): Find (the search box's query to start), Match case,
/// Replace with, how many matches there are under the zoom root; "Replace all" replaces them in one undoable
/// change.
pub fn replace_dialog() {
    let q = find().with_untracked(|f| f.query.trim().to_owned());
    replace_state().set(Replace { find: q, ..Default::default() });
    dialog::show(|| {
        let count = dynamic(|| {
            let r = replace_state().get();
            let msg = if r.find.is_empty() {
                String::new()
            } else {
                let (n, root) = store::read(|s| {
                    (refs::count_matches(s.tree(), &s.view.zoom, &r.find, r.match_case), s.view.zoom == ROOT)
                });
                format!("{n} match{} in this {}", if n == 1 { "" } else { "es" }, if root { "document" } else { "item" })
            };
            dialog::small(msg).test_id("replace-count").min_h(px(style::ui_sized("type.ui_small").1)).into_element()
        });
        let body = view()
            .child(dialog::field(
                "Find",
                dialog::input("replace-find", "Find")
                    .value(replace_state().with_untracked(|r| r.find.clone()))
                    .auto_focus(true)
                    .on_change(|t: &str| {
                        let t = t.to_owned();
                        replace_state().update(|r| r.find = t)
                    }),
            ))
            .child(dialog::check(
                "Match case",
                || replace_state().with(|r| r.match_case),
                || replace_state().update(|r| r.match_case = !r.match_case),
            ))
            .child(dialog::field(
                "Replace with",
                dialog::input("replace-with", "Replace with").on_change(|t: &str| {
                    let t = t.to_owned();
                    replace_state().update(|r| r.with = t)
                }),
            ))
            .child(count);
        dialog::modal(
            "replace",
            "Search and replace",
            body,
            vec![
                Button::close("Cancel"),
                Button::new("Replace all", true, || {
                    let r = replace_state().get_untracked();
                    let n = crate::commands::replace_all(&r.find, &r.with, r.match_case);
                    crate::toast::show(if n > 0 {
                        format!("Replaced {n} occurrence{}", if n == 1 { "" } else { "s" })
                    } else {
                        "Nothing to replace".to_owned()
                    });
                    true
                }),
            ],
        )
    });
}

/// A press on a tag (`toggleSearchTerm`): the tag joins the search's words, or leaves them (the search ending
/// when none is left).
pub fn toggle_term(tag: &str) {
    let q = find().with_untracked(|f| f.query.clone());
    let terms: Vec<&str> = q.split_whitespace().collect();
    let has = terms.contains(&tag);
    let next = if has {
        terms.iter().filter(|t| **t != tag).copied().collect::<Vec<_>>().join(" ")
    } else {
        terms.iter().copied().chain(std::iter::once(tag)).collect::<Vec<_>>().join(" ")
    };
    open(Some(&next));
    if has && next.is_empty() {
        close();
    }
}

/// The menu's "Search everywhere": the global search with no query yet.
pub fn search_everywhere() {
    show_results(Mode::Global, String::new());
}
