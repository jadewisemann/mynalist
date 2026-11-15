//! The workspace, as the web's (`apps/web/src/store/workspace.ts`): the file tree (folders and documents) and the
//! bookmarks, kept as the special document `workspace` (its `files`, `settings` and `bookmarks` nodes); and the
//! documents kept on the device as the web keeps them (`apps/web/src/store/sync.ts`): each document's
//! [`DocSnapshot`] under `doc:<id>` in the app's storage (NSH storage, the web's IndexedDB `kv` store), saved
//! 300 ms after a change. There is no server yet: everything stays on the device. The store ([`crate::store`])
//! holds the documents' sessions; this module keeps its list of documents in the file tree's order.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::Duration;

use nsh_core::prelude::*;
use nsh_core::storage::Storage;
use outliner_core::{compact, item, new_id, DocSession, DocSnapshot, Op, Props, Tree, Tx, ROOT};
use serde_json::Value;

use crate::store::{self, Session};

/// The workspace document's id (`WS_DOC`).
pub const WS_DOC: &str = "workspace";
const FILES: &str = "files";
const SETTINGS: &str = "settings";
const BOOKMARKS: &str = "bookmarks";
/// The open document and the zoom root of each (the web's `localStorage` keys).
const LAST_DOC: &str = "outliner:lastDoc";
const ZOOMS: &str = "outliner:zooms";
/// How long after a change a document is saved (the web's `save` timer).
const SAVE_DELAY: Duration = Duration::from_millis(300);

/// The storage key of document `id` (the web's `DOC_KEY`).
pub fn doc_key(id: &str) -> String {
    format!("doc:{id}")
}

/// What a file entry is (`FileKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Folder,
    Doc,
}

/// A file entry with the entries in it (`FileEntry`).
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub archived: bool,
    pub collapsed: bool,
    pub children: Vec<Entry>,
}

/// A bookmark (`Bookmark`): a document, a zoomed item in it, or a search: in the document's search box, or the
/// results screen of a flat search (`flat`) or of every document (`global`), sorted as it was (`sort`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bookmark {
    pub id: String,
    pub title: String,
    pub doc: String,
    pub zoom: Option<String>,
    pub q: Option<String>,
    pub flat: bool,
    pub global: bool,
    pub sort: Option<String>,
}

struct Ws {
    session: DocSession<()>,
    /// The documents are kept on the device (the app's, not a look check's fixture).
    persist: bool,
    /// The documents (and [`WS_DOC`]) changed since they were saved.
    dirty: BTreeSet<String>,
    /// A save is due.
    due: bool,
}

#[derive(Clone)]
struct Shared(Rc<RefCell<Ws>>);

impl Default for Shared {
    fn default() -> Self {
        let mut session = DocSession::default();
        ensure_structure(&mut session);
        Shared(Rc::new(RefCell::new(Ws { session, persist: false, dirty: BTreeSet::new(), due: false })))
    }
}

/// The workspace's revision: bumped by every change.
#[derive(Clone, Copy, Default, PartialEq)]
struct Rev(u64);

fn shared() -> Rc<RefCell<Ws>> {
    global::<Shared>().get_untracked().0
}

/// Read the workspace tree; the reader runs again when it changes.
pub fn read<R>(f: impl FnOnce(&Tree) -> R) -> R {
    global::<Rev>().get();
    f(&shared().borrow().session.live)
}

fn peek<R>(f: impl FnOnce(&Tree) -> R) -> R {
    f(&shared().borrow().session.live)
}

/// The fixed top-level containers (`ensureStructure`: deterministic ids).
fn ensure_structure(session: &mut DocSession<()>) {
    let mut ops = Vec::new();
    let t = &session.live;
    let ts = store::now();
    let mk = |id: &str, after: Option<&str>| Op::Create {
        id: id.into(),
        parent: ROOT.into(),
        after: after.map(Into::into),
        props: props(&[("title", Value::String(id.into()))]),
        ts,
    };
    if t.get(FILES).is_none() {
        ops.push(mk(FILES, None));
    }
    if t.get(SETTINGS).is_none() {
        ops.push(mk(SETTINGS, Some(FILES)));
    }
    if t.get(BOOKMARKS).is_none() {
        ops.push(mk(BOOKMARKS, Some(SETTINGS)));
    }
    session.transact(ops, (), (), false);
}

fn props(kv: &[(&str, Value)]) -> Props {
    kv.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect()
}

fn text(t: &Tree, id: &str, key: &str) -> Option<String> {
    t.get(id).and_then(|n| n.props.get(key)).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned)
}

fn flag(t: &Tree, id: &str, key: &str) -> bool {
    t.get(id).and_then(|n| n.props.get(key)).is_some_and(|v| !outliner_core::tree::is_falsy(v))
}

/// Change the workspace (`run`): not undoable, saved, the store's list of documents kept in order.
fn run(f: impl FnOnce(&mut Tx)) {
    {
        let ws = shared();
        let mut ws = ws.borrow_mut();
        let (ops, inverse) = {
            let mut tx = Tx::new(&mut ws.session.live, store::now());
            f(&mut tx);
            (tx.ops, tx.inverse)
        };
        if ops.is_empty() {
            return;
        }
        ws.session.commit(ops, inverse, (), (), false);
    }
    global::<Rev>().update(|r| r.0 += 1);
    changed(WS_DOC);
    sync_files();
}

fn entry(t: &Tree, id: &str) -> Entry {
    let n = t.get(id);
    Entry {
        id: id.to_owned(),
        kind: if text(t, id, "kind").as_deref() == Some("folder") { Kind::Folder } else { Kind::Doc },
        title: text(t, id, "title").unwrap_or_else(|| "Untitled".into()),
        archived: flag(t, id, "archived"),
        collapsed: n.is_some_and(|n| n.collapsed),
        children: n.map(|n| n.children.iter().map(|c| entry(t, c)).collect()).unwrap_or_default(),
    }
}

/// The file tree (`files`; tracked).
pub fn files() -> Vec<Entry> {
    read(entries)
}

/// The file tree, untracked (in handlers).
pub fn files_now() -> Vec<Entry> {
    peek(entries)
}

fn entries(t: &Tree) -> Vec<Entry> {
    t.get(FILES).map(|f| f.children.iter().map(|c| entry(t, c)).collect()).unwrap_or_default()
}

/// The documents in the file tree, depth first (`allDocs`).
fn all_docs(t: &Tree) -> Vec<String> {
    fn walk(es: &[Entry], out: &mut Vec<String>) {
        for e in es {
            match e.kind {
                Kind::Doc => out.push(e.id.clone()),
                Kind::Folder => walk(&e.children, out),
            }
        }
    }
    let mut out = Vec::new();
    walk(&entries(t), &mut out);
    out
}

/// The folders, depth first, with their depth (for "Move to").
pub fn folders() -> Vec<(String, String, u16)> {
    fn walk(es: &[Entry], depth: u16, out: &mut Vec<(String, String, u16)>) {
        for e in es.iter().filter(|e| e.kind == Kind::Folder) {
            out.push((e.id.clone(), e.title.clone(), depth));
            walk(&e.children, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(&peek(entries), 0, &mut out);
    out
}

/// An entry by id, if it is in the file tree (`get`).
pub fn get(id: &str) -> Option<Entry> {
    peek(|t| (t.is_attached(id) && t.get(id).is_some()).then(|| entry(t, id)))
}

/// The folder an entry is in (`parentOf`; `None` at the top level).
pub fn parent_of(id: &str) -> Option<String> {
    peek(|t| t.get(id).and_then(|n| n.parent.clone())).filter(|p| p != FILES)
}

/// The folders an entry is in, from the top, as the finders show them (`allDocs`' `path`: "A / B"; "" at the top
/// level).
pub fn path_of(id: &str) -> String {
    peek(|t| {
        let names: Vec<String> = t
            .ancestors(id)
            .iter()
            .filter(|a| t.get(a).is_some_and(|n| n.parent.is_some()) && a.as_str() != FILES)
            .map(|a| text(t, a, "title").unwrap_or_else(|| "Untitled".into()))
            .collect();
        names.join(" / ")
    })
}

/// Its position among its siblings (`indexOf`).
pub fn index_of(id: &str) -> usize {
    peek(|t| t.index(id)).unwrap_or(0)
}

/// Whether `id` is `folder` or inside it.
pub fn is_within(id: &str, folder: &str) -> bool {
    id == folder || peek(|t| t.is_ancestor(folder, id))
}

/// A new entry last in `parent` (the top level for `None`), the folder opened (`create`); a document starts
/// with its name as its root's text, in an op as on the web. Its id.
pub fn create(kind: Kind, parent: Option<&str>, title: &str) -> String {
    let id = new_id();
    if kind == Kind::Doc {
        store::edit(|s| s.add(&id, store::named(title)));
        changed(&id);
    }
    let p = parent.unwrap_or(FILES).to_owned();
    let kind_name = if kind == Kind::Folder { "folder" } else { "doc" };
    let new = id.clone();
    run(move |tx| {
        let last = tx.tree.get(&p).and_then(|n| n.children.last().cloned());
        let entry = props(&[("kind", Value::String(kind_name.into())), ("title", Value::String(title.into()))]);
        tx.create_with_id(&p, last.as_deref(), entry, new);
        if p != FILES {
            tx.collapse(&p, false);
        }
    });
    id
}

/// Rename an entry (`rename`): a document's root text too.
pub fn rename(id: &str, title: &str) {
    run(|tx| {
        tx.set(id, props(&[("title", Value::String(title.into()))]));
    });
    if store::peek(|s| s.tree_of(id).is_some() && s.title(id) != title) {
        store::edit(|s| s.rename(id, title));
        changed(id);
    }
}

/// The open document's title was edited in place (`syncTitleFromDoc`): its entry takes the new name.
pub fn sync_title(id: &str, title: &str) {
    let title = if title.is_empty() { "Untitled" } else { title };
    if get(id).is_some_and(|e| e.title != title) {
        run(|tx| {
            tx.set(id, props(&[("title", Value::String(title.into()))]));
        });
    }
}

/// Move an entry into `parent` (the top level for `None`) at `index` among the others there (`move`).
pub fn move_to(id: &str, parent: Option<&str>, index: usize) {
    let p = parent.unwrap_or(FILES).to_owned();
    if p != FILES && is_within(&p, id) {
        return;
    }
    run(|tx| {
        let kids: Vec<String> = tx.tree.get(&p).map(|n| n.children.clone()).unwrap_or_default();
        let kids: Vec<&String> = kids.iter().filter(|c| *c != id).collect();
        let at = index.min(kids.len());
        let after = if at == 0 { None } else { Some(kids[at - 1].clone()) };
        tx.move_to(id, Some(&p), after.as_deref());
    });
}

/// Take an entry (a folder with what is in it) out of the file tree (`remove`). Its documents stay on the
/// device, as the web leaves them in its store.
pub fn remove(id: &str) {
    run(|tx| {
        tx.move_to(id, None, None);
    });
}

pub fn set_archived(id: &str, archived: bool) {
    run(|tx| {
        tx.set(id, props(&[("archived", Value::Bool(archived))]));
    });
}

/// A document's view option `key` (`docPref`: its entry's `v_<key>`, as JSON), tracked; `None` when not set.
pub fn doc_pref(id: &str, key: &str) -> Option<Value> {
    read(|t| pref_of(t, id, key))
}

/// [`doc_pref`], untracked.
pub fn doc_pref_now(id: &str, key: &str) -> Option<Value> {
    peek(|t| pref_of(t, id, key))
}

fn pref_of(t: &Tree, id: &str, key: &str) -> Option<Value> {
    let raw = t.get(id)?.props.get(&format!("v_{key}"))?.as_str()?;
    serde_json::from_str(raw).ok()
}

/// Sets a document's view option (`setDocPref`); `None` takes it away.
pub fn set_doc_pref(id: &str, key: &str, value: Option<Value>) {
    let v = value.map_or(Value::Null, |v| Value::String(v.to_string()));
    run(|tx| {
        tx.set(id, props(&[(&format!("v_{key}"), v)]));
    });
}

pub fn set_collapsed(id: &str, v: bool) {
    run(|tx| {
        tx.collapse(id, v);
    });
}

/// Fold every folder (`collapseAll`).
pub fn collapse_all() {
    fn walk(es: &[Entry], out: &mut Vec<String>) {
        for e in es.iter().filter(|e| e.kind == Kind::Folder) {
            if !e.collapsed {
                out.push(e.id.clone());
            }
            walk(&e.children, out);
        }
    }
    let mut open = Vec::new();
    walk(&peek(entries), &mut open);
    if !open.is_empty() {
        run(|tx| {
            for id in &open {
                tx.collapse(id, true);
            }
        });
    }
}

/// A copy of document `id` right after it, "<name> (copy)", its items and note copied (`copyDoc`); its id.
pub fn copy_doc(id: &str) -> Option<String> {
    let (ops, note) = store::peek(|s| {
        let t = s.tree_of(id)?;
        let mut ops = Vec::new();
        let mut ids = std::collections::HashMap::new();
        let ts = store::now();
        // depth first, each item after its sibling copied before it
        let mut stack: Vec<(String, String)> = t.root().children.iter().rev().map(|c| (c.clone(), ROOT.to_owned())).collect();
        let mut last: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        while let Some((old, parent)) = stack.pop() {
            let Some(n) = t.get(&old) else { continue };
            let new = new_id();
            ids.insert(old.clone(), new.clone());
            let after = last.insert(parent.clone(), new.clone());
            ops.push(Op::Create { id: new.clone(), parent, after, props: n.props.clone(), ts });
            if n.collapsed {
                ops.push(Op::Collapse { id: new.clone(), v: true });
            }
            stack.extend(n.children.iter().rev().map(|c| (c.clone(), new.clone())));
        }
        Some((ops, item::note(t.get(ROOT)).to_owned()))
    })?;
    let title = format!("{} (copy)", store::peek(|s| s.title(id)));
    let parent = parent_of(id);
    let copy = create(Kind::Doc, parent.as_deref(), &title);
    move_to(&copy, parent.as_deref(), index_of(id) + 1);
    store::edit(|s| {
        if let Some(session) = s.session_mut(&copy) {
            let mut ops = ops;
            if !note.is_empty() {
                ops.push(Op::Set { id: ROOT.into(), props: props(&[("n", Value::String(note))]), ts: store::now() });
            }
            session.transact(ops, None, None, false);
        }
        s.switch(&copy);
        s.reflow();
    });
    changed(&copy);
    Some(copy)
}

/// A new document `title` in `parent` holding `items` (an imported file), opened (`importOpml`); its id.
pub fn import(parent: Option<&str>, title: &str, items: &[outliner_core::commands::Portable]) -> String {
    let id = create(Kind::Doc, parent, title);
    store::edit(|s| s.import(&id, items));
    changed(&id);
    id
}

// ---- bookmarks -------------------------------------------------------------

/// The bookmarks in their order (`bookmarks`; tracked).
pub fn bookmarks() -> Vec<Bookmark> {
    read(bookmarks_of)
}

/// The bookmarks, untracked (in handlers).
pub fn bookmarks_now() -> Vec<Bookmark> {
    peek(bookmarks_of)
}

fn bookmarks_of(t: &Tree) -> Vec<Bookmark> {
    {
        t.get(BOOKMARKS)
            .map(|b| {
                b.children
                    .iter()
                    .map(|id| Bookmark {
                        id: id.clone(),
                        title: text(t, id, "title").unwrap_or_else(|| "Untitled".into()),
                        doc: text(t, id, "doc").unwrap_or_default(),
                        zoom: text(t, id, "zoom"),
                        q: text(t, id, "q"),
                        flat: flag(t, id, "flat"),
                        global: flag(t, id, "global"),
                        sort: text(t, id, "sort"),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A bookmark last in the list (`addBookmark`).
pub fn add_bookmark(title: &str, doc: &str, zoom: Option<&str>) {
    add_search_bookmark(title, doc, zoom, None);
}

/// [`add_bookmark`] of a search (`q`) in the document.
pub fn add_search_bookmark(title: &str, doc: &str, zoom: Option<&str>, q: Option<&str>) {
    let b = Bookmark {
        title: title.into(),
        doc: doc.into(),
        zoom: zoom.map(str::to_owned),
        q: q.map(str::to_owned),
        ..Default::default()
    };
    add(&b);
}

/// Adds bookmark `b` last (`addBookmark`; its id is made): the properties the web sets, none that are unset.
pub fn add(b: &Bookmark) {
    run(|tx| {
        let last = tx.tree.get(BOOKMARKS).and_then(|n| n.children.last().cloned());
        let mut p = props(&[("title", Value::String(b.title.clone())), ("doc", Value::String(b.doc.clone()))]);
        for (k, v) in [("zoom", &b.zoom), ("q", &b.q), ("sort", &b.sort)] {
            if let Some(v) = v {
                p.insert(k.into(), Value::String(v.clone()));
            }
        }
        for (k, on) in [("flat", b.flat), ("global", b.global)] {
            if on {
                p.insert(k.into(), Value::Bool(true));
            }
        }
        tx.create(BOOKMARKS, last.as_deref(), p);
    });
}

/// Move a bookmark to `index` among the others (`moveBookmark`).
pub fn move_bookmark(id: &str, index: usize) {
    move_within(BOOKMARKS, id, index);
}

fn move_within(parent: &str, id: &str, index: usize) {
    run(|tx| {
        let kids: Vec<String> = tx.tree.get(parent).map(|n| n.children.clone()).unwrap_or_default();
        let kids: Vec<&String> = kids.iter().filter(|c| *c != id).collect();
        let at = index.min(kids.len());
        let after = if at == 0 { None } else { Some(kids[at - 1].clone()) };
        tx.move_to(id, Some(parent), after.as_deref());
    });
}

pub fn rename_bookmark(id: &str, title: &str) {
    run(|tx| {
        tx.set(id, props(&[("title", Value::String(title.into()))]));
    });
}

pub fn remove_bookmark(id: &str) {
    run(|tx| {
        tx.move_to(id, None, None);
    });
}

// ---- the store's documents ---------------------------------------------------

/// Keep the store's documents as the file tree lists them: new ones added, those that left it closed (the one
/// listed after the open one opens, or the one before); with none left, a new "Untitled" one (the web shows an
/// empty pane then; the native app always has a document open).
fn sync_files() {
    let docs = peek(all_docs);
    let (files, open) = store::peek(|s| (s.files.clone(), s.doc.clone()));
    if files == docs {
        return;
    }
    if docs.is_empty() {
        create(Kind::Doc, None, "Untitled");
        return;
    }
    let next = if docs.contains(&open) {
        open.clone()
    } else {
        let at = files.iter().position(|f| *f == open).unwrap_or(0);
        files[at.min(files.len())..]
            .iter()
            .chain(files[..at.min(files.len())].iter().rev())
            .find(|f| docs.contains(f))
            .cloned()
            .unwrap_or_else(|| docs[0].clone())
    };
    let persist = shared().borrow().persist;
    store::edit(|s| {
        for id in &docs {
            if s.tree_of(id).is_none() {
                let session = if persist { load_doc(id) } else { None };
                s.add(id, session.unwrap_or_default());
            }
        }
        if s.tree_of(&next).is_some() {
            for id in files.iter().filter(|f| !docs.contains(f)) {
                s.remove(id, &next);
            }
        }
        s.set_files(docs);
    });
}

// ---- kept on the device -----------------------------------------------------

/// A document kept on the device, if there is one (and it reads).
fn load_doc(id: &str) -> Option<Session> {
    let raw = Storage::get_item(&doc_key(id))?;
    match serde_json::from_str::<DocSnapshot>(&raw) {
        Ok(snap) => Some(DocSession::new(Some(&snap))),
        Err(e) => {
            // kept as it is: never overwritten until the document changes here
            eprintln!("outliner: document {id} could not be read: {e}");
            None
        }
    }
}

/// Open the workspace and the documents kept on the device, and keep them from now on; the documents the device
/// has none of yet (a first start) come from `first`, in its order. The document open last opens again, in its
/// zoom.
pub fn start(first: impl FnOnce() -> Vec<(String, String, Session)>) {
    let kept = Storage::get_item(&doc_key(WS_DOC)).and_then(|raw| serde_json::from_str::<DocSnapshot>(&raw).ok());
    let fresh = kept.is_none();
    let mut session = DocSession::new(kept.as_ref());
    ensure_structure(&mut session);
    {
        let ws = shared();
        let mut ws = ws.borrow_mut();
        ws.session = session;
        ws.persist = true;
    }
    if fresh {
        let docs = first();
        let ws = shared();
        let mut ws = ws.borrow_mut();
        let ts = store::now();
        let mut after: Option<String> = None;
        let mut ops = Vec::new();
        for (id, title, _) in &docs {
            let entry = props(&[("kind", Value::String("doc".into())), ("title", Value::String(title.clone()))]);
            ops.push(Op::Create { id: id.clone(), parent: FILES.into(), after: after.replace(id.clone()), props: entry, ts });
        }
        ws.session.transact(ops, (), (), false);
        drop(ws);
        let ids: Vec<String> = docs.iter().map(|(id, _, _)| id.clone()).collect();
        store::open_all(docs.into_iter().map(|(id, _, s)| (id, s)).collect());
        for id in ids.iter().chain(std::iter::once(&WS_DOC.to_owned())) {
            changed(id);
        }
    } else {
        let ids = peek(all_docs);
        let mut docs: Vec<(String, Session)> =
            ids.iter().map(|id| (id.clone(), load_doc(id).unwrap_or_default())).collect();
        if docs.is_empty() {
            let id = new_id();
            docs.push((id, store::named("Untitled")));
        }
        let only_new = ids.is_empty().then(|| docs[0].0.clone());
        store::open_all(docs);
        if let Some(id) = only_new {
            // its entry, as a new document's
            run(|tx| {
                let entry =
                    props(&[("kind", Value::String("doc".into())), ("title", Value::String("Untitled".into()))]);
                tx.create_with_id(FILES, None, entry, id.clone());
            });
            changed(&id);
        }
    }
    // the document open last, in the zoom it had
    let zooms = zooms();
    let last = Storage::get_item(LAST_DOC);
    store::edit(|s| {
        for (doc, zoom) in &zooms {
            if let Some(v) = s.view_of(doc) {
                v.zoom = zoom.clone();
            }
        }
        if let Some(last) = last.filter(|d| s.files.contains(d)) {
            s.switch(&last);
        }
        s.reflow();
    });
    sync_files();
    // the open document is saved when it changes
    effect(|| {
        let doc = store::read(|s| s.doc.clone());
        changed(&doc);
    });
    // and all of it when the app goes to the background or quits
    effect(|| {
        if lifecycle() != nsh_core::platform::Lifecycle::Active {
            untracked(flush);
        }
    });
}

/// The zoom root of each document (`outliner:zooms`).
fn zooms() -> BTreeMap<String, String> {
    Storage::get_item(ZOOMS).and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or_default()
}

/// Document `id` (or [`WS_DOC`]) changed: saved soon, if the documents are kept.
pub fn changed(id: &str) {
    let ws = shared();
    let mut ws = ws.borrow_mut();
    if !ws.persist {
        return;
    }
    ws.dirty.insert(id.to_owned());
    if !ws.due {
        ws.due = true;
        // under no view's owner: a view going away (a row, an input) does not cancel the save
        Owner::detached().run(|| {
            drop(set_timeout(SAVE_DELAY, flush));
        });
    }
}

/// Save what changed now (`flushSaves`): the documents, the workspace, the open document and its zoom.
pub fn flush() {
    let dirty = {
        let ws = shared();
        let mut ws = ws.borrow_mut();
        ws.due = false;
        if !ws.persist {
            return;
        }
        std::mem::take(&mut ws.dirty)
    };
    // a document's name follows its root's text (`syncTitleFromDoc`)
    let renamed: Vec<(String, String)> = dirty
        .iter()
        .filter(|id| id.as_str() != WS_DOC)
        .filter_map(|id| {
            let now = store::peek(|s| s.tree_of(id).map(|_| s.title(id)))?;
            (peek(|t| t.get(id).is_some() && text(t, id, "title").unwrap_or_else(|| "Untitled".into()) != now))
                .then(|| (id.clone(), now))
        })
        .collect();
    let mut dirty = dirty;
    if !renamed.is_empty() {
        let ws = shared();
        let mut ws = ws.borrow_mut();
        let (ops, inverse) = {
            let mut tx = Tx::new(&mut ws.session.live, store::now());
            for (id, title) in &renamed {
                tx.set(id, props(&[("title", Value::String(title.clone()))]));
            }
            (tx.ops, tx.inverse)
        };
        ws.session.commit(ops, inverse, (), (), false);
        drop(ws);
        global::<Rev>().update(|r| r.0 += 1);
        dirty.insert(WS_DOC.to_owned());
    }
    for id in &dirty {
        let snap = if id == WS_DOC {
            Some(shared().borrow().session.snapshot())
        } else {
            store::peek(|s| s.session_of(id).map(DocSession::snapshot))
        };
        let Some(mut snap) = snap else { continue };
        snap.pending = compact(&snap.pending);
        match serde_json::to_string(&snap) {
            Ok(json) => {
                if let Err(e) = Storage::set_item(doc_key(id), json) {
                    eprintln!("outliner: document {id} could not be saved: {e:?}");
                }
            }
            Err(e) => eprintln!("outliner: document {id} could not be saved: {e}"),
        }
    }
    // the open document and its zoom, as the web keeps them
    let (doc, zoom) = store::peek(|s| (s.doc.clone(), s.view.zoom.clone()));
    if Storage::get_item(LAST_DOC).as_deref() != Some(doc.as_str()) {
        let _ = Storage::set_item(LAST_DOC, doc.clone());
    }
    let mut z = zooms();
    let before = z.clone();
    if zoom == ROOT {
        z.remove(&doc);
    } else {
        z.insert(doc, zoom);
    }
    if z != before {
        let _ = Storage::set_item(ZOOMS, serde_json::to_string(&z).unwrap_or_default());
    }
}

/// A folder of a fixture's file tree: its id, its name, whether it is folded, its documents (id, name).
pub type FolderSpec<'a> = (&'a str, &'a str, bool, &'a [(&'a str, &'a str)]);

/// A file tree of documents `docs` (id, name) at the top level, and folders after them (a fixture's workspace;
/// the documents must be in the store).
pub fn list(docs: &[(&str, &str)], folders: &[FolderSpec<'_>]) {
    run(|tx| {
        let mut after: Option<String> = None;
        let doc = |name: &str| props(&[("kind", Value::String("doc".into())), ("title", Value::String(name.into()))]);
        for (id, name) in docs {
            tx.create_with_id(FILES, after.as_deref(), doc(name), (*id).to_owned());
            after = Some((*id).to_owned());
        }
        for (id, name, folded, inside) in folders {
            let folder = props(&[("kind", Value::String("folder".into())), ("title", Value::String((*name).into()))]);
            tx.create_with_id(FILES, after.as_deref(), folder, (*id).to_owned());
            after = Some((*id).to_owned());
            let mut last: Option<String> = None;
            for (d, n) in inside.iter() {
                tx.create_with_id(id, last.as_deref(), doc(n), (*d).to_owned());
                last = Some((*d).to_owned());
            }
            if *folded {
                tx.collapse(id, true);
            }
        }
    });
}

/// A fresh workspace with no entries (a fixture's start).
pub fn reset() {
    let mut session = DocSession::default();
    ensure_structure(&mut session);
    let ws = shared();
    let mut ws = ws.borrow_mut();
    ws.session = session;
    ws.dirty.clear();
}
