//! The structural snapshot: the fixtures of `outliner_app::fixture` (the demo's first screen, the inline showcase,
//! the item looks), each as a desktop and a phone, at rest and with a row being edited, rendered headless and
//! dumped as text: the render model (top bar, title, rows with their depth, bullet center, text left, top, height
//! and styled spans, the row being edited and the bullet thread's pieces; the phone toolbar), then every view on
//! screen. The dumps are compared byte for byte with the
//! golden files in `tests/snapshots/`; `UPDATE_SNAPSHOTS=1 cargo test --test snapshot` rewrites them.
//!
//! The output depends on nothing of the machine: a fixed clock and time zone, fixed ids, reduced motion, and
//! nsh-testing's approximate text engine (sizes from the characters, not from fonts).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use nsh_core::platform::{Idiom, Os};
use nsh_core::time::Clock;
use nsh_core::{AttributedText, Color, MountItem, NodeId, Points, ResolvedTextStyle, Size, ViewKind, ViewStyle};
use nsh_engine::EngineConfig;
use nsh_testing::{Headless, LayoutNode};

/// The fixed now: 2024-06-15 12:00 UTC.
const NOW_MS: u64 = 1_718_452_800_000;

#[derive(Clone, Copy)]
struct Device {
    name: &'static str,
    os: Os,
    width: f32,
    height: f32,
}

const DESKTOP: Device = Device { name: "desktop", os: Os::MacOs, width: 1100.0, height: 800.0 };
const PHONE: Device = Device { name: "phone", os: Os::Ios, width: 390.0, height: 844.0 };

type Styles = Arc<Mutex<HashMap<NodeId, ViewStyle>>>;

fn start(doc: &str, editing: bool, device: Device, appearance: &str) -> (Headless, Styles) {
    start_with(doc, editing, device, "", appearance)
}

/// [`start`] in `modes` (`group=mode` words).
fn start_with(doc: &str, editing: bool, device: Device, modes: &str, appearance: &str) -> (Headless, Styles) {
    // the dates' local zone (every snapshot sets the same)
    std::env::set_var("TZ", "UTC");
    let edited = editing.then(|| outliner_app::fixture::edited_item(doc));
    let mut config = EngineConfig::new(outliner_app::fixture::root_with(doc, edited, modes, appearance));
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_millis(NOW_MS));
    config.viewport = Size { width: Points(device.width), height: Points(device.height) };
    config.environment.os = device.os;
    config.environment.idiom = if device.os == Os::MacOs { Idiom::Desktop } else { Idiom::Phone };
    config.environment.reduce_motion = true;
    config.environment.pixel_ratio = 2.0;
    let mut app = Headless::start(config).unwrap();
    // the views' visual styles, which the screen model does not keep
    let styles = Styles::default();
    let seen = styles.clone();
    app.observe(move |_, batch| {
        let mut seen = seen.lock().unwrap();
        for item in &batch.items {
            match item {
                MountItem::ViewStyle { id, style } => {
                    seen.insert(*id, (**style).clone());
                }
                MountItem::Delete { id } => {
                    seen.remove(id);
                }
                _ => {}
            }
        }
    });
    app.wait_for("id:row").unwrap();
    if editing {
        app.wait_for("id:row-input").unwrap();
    }
    app.settle();
    (app, styles)
}

/// A length rounded to 0.5pt, written short (`12`, `12.5`).
fn len(v: f32) -> String {
    let r = (v * 2.0).round() / 2.0;
    let r = if r == 0.0 { 0.0 } else { r };
    if r.fract() == 0.0 { format!("{r:.0}") } else { format!("{r:.1}") }
}

fn color(c: Color) -> String {
    let Color(v) = c;
    if v & 0xff == 0xff { format!("#{:06x}", v >> 8) } else { format!("#{v:08x}") }
}

fn transparent(c: Color) -> bool {
    c.0 & 0xff == 0
}

fn span_style(s: &ResolvedTextStyle) -> String {
    let family = s.font_family.map_or_else(|| "system".to_owned(), |f| f.name().to_string());
    let (size, line) = (len(s.font_size.0), len(s.line_height.0));
    let mut out = format!("{family} {size}/{line} w{} {}", s.font_weight, color(s.color));
    if s.italic {
        out.push_str(" italic");
    }
    if s.decoration.underline {
        out.push_str(" underline");
    }
    if s.decoration.line_through {
        out.push_str(" strike");
    }
    if let Some(c) = s.decoration_color {
        let _ = write!(out, " decoration={}", color(c));
    }
    if s.letter_spacing.0 != 0.0 {
        let _ = write!(out, " spacing={}", len(s.letter_spacing.0));
    }
    if s.font_variant != Default::default() {
        let _ = write!(out, " variant={:?}", s.font_variant);
    }
    if let Some(bg) = &s.background {
        let _ = write!(out, " bg={} r={}", color(bg.color), len(bg.radius.0));
    }
    if s.hidden {
        out.push_str(" hidden");
    }
    out
}

/// A text's characters after `head`, then its spans one per line, under `pad`.
fn write_text(out: &mut String, pad: &str, head: &str, t: &AttributedText) {
    let _ = writeln!(out, "{pad}{head}{:?}", t.text);
    let units: Vec<u16> = t.text.encode_utf16().collect();
    for span in &t.spans {
        let part = String::from_utf16_lossy(&units[span.start as usize..span.end as usize]);
        let _ = writeln!(out, "{pad}  {:?} {}", part, span_style(&span.style));
    }
}

fn view_style(s: &ViewStyle) -> String {
    let mut out = String::new();
    if !transparent(s.background_color) {
        let _ = write!(out, " bg={}", color(s.background_color));
    }
    if s.border_radius.0 > 0.0 {
        let _ = write!(out, " r={}", len(s.border_radius.0));
    }
    let c = &s.corner_radius;
    for (name, r) in [("tl", c.top_left), ("tr", c.top_right), ("br", c.bottom_right), ("bl", c.bottom_left)] {
        if let Some(r) = r {
            let _ = write!(out, " r-{name}={}", len(r.0));
        }
    }
    let w = &s.borders.widths;
    let edges = [("t", w.top, 0), ("r", w.right, 1), ("b", w.bottom, 2), ("l", w.left, 3)];
    let c = s.borders.colors;
    if edges.iter().all(|e| e.1 == w.top) && c.iter().all(|k| *k == c[0]) {
        if w.top.0 > 0.0 {
            let _ = write!(out, " border={} {}", len(w.top.0), color(c[0]));
        }
    } else {
        for (name, width, k) in edges {
            if width.0 > 0.0 {
                let _ = write!(out, " border-{name}={} {}", len(width.0), color(c[k]));
            }
        }
    }
    if s.opacity < 1.0 {
        let _ = write!(out, " opacity={}", s.opacity);
    }
    out
}

fn kind(k: ViewKind) -> String {
    format!("{k:?}").to_lowercase()
}

/// The laid-out views with an area, and what they hold: a view of no area (a list's spare cell, an empty slot)
/// is left out with what it holds.
fn shown(app: &Headless) -> Vec<LayoutNode> {
    let mut gone: Vec<NodeId> = Vec::new();
    let mut out = Vec::new();
    for n in app.screen().layout_nodes() {
        if gone.contains(&n.parent) || n.bounds.size.width.0 <= 0.0 || n.bounds.size.height.0 <= 0.0 {
            gone.push(n.id);
            continue;
        }
        out.push(n);
    }
    out
}

struct Tree {
    nodes: Vec<LayoutNode>,
    kids: HashMap<NodeId, Vec<usize>>,
}

impl Tree {
    fn new(nodes: Vec<LayoutNode>) -> Tree {
        let mut kids: HashMap<NodeId, Vec<usize>> = HashMap::new();
        for (k, n) in nodes.iter().enumerate() {
            kids.entry(n.parent).or_default().push(k);
        }
        Tree { nodes, kids }
    }

    /// The views inside `id`, in tree order.
    fn inside(&self, id: NodeId) -> Vec<&LayoutNode> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = self.kids.get(&id).cloned().unwrap_or_default();
        stack.reverse();
        while let Some(k) = stack.pop() {
            let n = &self.nodes[k];
            out.push(n);
            let mut more = self.kids.get(&n.id).cloned().unwrap_or_default();
            more.reverse();
            stack.extend(more);
        }
        out
    }

    fn with_id<'a>(&'a self, id: &str) -> impl Iterator<Item = &'a LayoutNode> + 'a {
        let id = id.to_owned();
        self.nodes.iter().filter(move |n| n.test_id.as_deref() == Some(id.as_str()))
    }

    fn depth(&self, n: &LayoutNode) -> usize {
        let mut d = 0;
        let mut p = n.parent;
        while let Some(parent) = self.nodes.iter().find(|m| m.id == p) {
            d += 1;
            p = parent.parent;
        }
        d
    }
}

fn is(n: &LayoutNode, id: &str) -> bool {
    n.test_id.as_deref() == Some(id)
}

fn input_text(app: &Headless, n: &LayoutNode) -> String {
    match app.screen().input_text(&format!("view:{}", n.id)) {
        Ok(t) => format!("{:?}", t.unwrap_or_default()),
        Err(e) => format!("<{e}>"),
    }
}

/// The texts (and inputs) inside `id`, under `pad`.
fn write_texts(out: &mut String, app: &Headless, tree: &Tree, id: NodeId, pad: &str) {
    for n in tree.inside(id) {
        write_texts_of(out, app, n, pad);
    }
}

/// A piece of the bullet thread as the model writes it: a straight run (pieces at one x that touch, merged) or a
/// turn into a bullet. `x` is the center of the vertical line.
struct ThreadPiece {
    x: f32,
    top: f32,
    bottom: f32,
    w: f32,
    color: String,
    /// A turn: where it ends on the right, and its corner's radius.
    elbow: Option<(f32, f32)>,
}

impl ThreadPiece {
    fn write(&self, out: &mut String) {
        let (x, top, bottom, w) = (len(self.x), len(self.top), len(self.bottom), len(self.w));
        match self.elbow {
            None => {
                let _ = writeln!(out, "  thread x={x} top={top} bottom={bottom} w={w} {}", self.color);
            }
            Some((right, r)) => {
                let (right, r) = (len(right), len(r));
                let _ = writeln!(out, "  elbow x={x} top={top} bottom={bottom} right={right} w={w} r={r} {}", self.color);
            }
        }
    }
}

/// The bullet thread's views (`thread`, `elbow`), straight runs merged.
fn thread_pieces(tree: &Tree, styles: &HashMap<NodeId, ViewStyle>) -> Vec<ThreadPiece> {
    let mut runs: Vec<ThreadPiece> = Vec::new();
    for n in tree.with_id("thread") {
        let b = n.bounds;
        let color = styles.get(&n.id).map_or_else(String::new, |s| color(s.background_color));
        let (x, top, w) = (b.origin.x.0 + b.size.width.0 / 2.0, b.origin.y.0, b.size.width.0);
        let bottom = top + b.size.height.0;
        match runs.iter_mut().find(|r| (r.x - x).abs() < 0.25 && (r.bottom - top).abs() <= 0.5 && r.color == color) {
            Some(r) => r.bottom = bottom,
            None => runs.push(ThreadPiece { x, top, bottom, w, color, elbow: None }),
        }
    }
    for n in tree.with_id("elbow") {
        let b = n.bounds;
        let Some(s) = styles.get(&n.id) else { continue };
        let w = s.borders.widths.left.0;
        let r = s.corner_radius.bottom_left.map_or(s.border_radius.0, |r| r.0);
        runs.push(ThreadPiece {
            x: b.origin.x.0 + w / 2.0,
            top: b.origin.y.0,
            bottom: b.origin.y.0 + b.size.height.0,
            w,
            color: color(s.borders.colors[3]),
            elbow: Some((b.origin.x.0 + b.size.width.0, r)),
        });
    }
    runs.sort_by(|a, b| a.top.total_cmp(&b.top).then(a.x.total_cmp(&b.x)));
    runs
}

/// The render model area 6 shares with the web app.
fn model(out: &mut String, app: &Headless, tree: &Tree, styles: &HashMap<NodeId, ViewStyle>) {
    let thread = thread_pieces(tree, styles);
    let _ = writeln!(out, "[model]");
    for bar in tree.with_id("topbar") {
        let b = bar.bounds;
        let _ = writeln!(out, "topbar top={} height={}", len(b.origin.y.0), len(b.size.height.0));
    }
    for title in tree.with_id("title") {
        let b = title.bounds;
        let _ = writeln!(out, "title top={} height={}", len(b.origin.y.0), len(b.size.height.0));
        write_texts(out, app, tree, title.id, "  ");
    }
    for row in tree.with_id("row") {
        let b = row.bounds;
        let inside = tree.inside(row.id);
        let depth = inside.iter().filter(|n| is(n, "guide")).count();
        let _ = write!(out, "row depth={depth} top={} height={}", len(b.origin.y.0), len(b.size.height.0));
        let bullet = inside.iter().find(|n| is(n, "bullet"));
        if let Some(bullet) = bullet {
            let x = bullet.bounds.origin.x.0 + bullet.bounds.size.width.0 / 2.0;
            let _ = write!(out, " bullet={}", len(x));
            // a number in place of the dot, and where it ends
            match tree.inside(bullet.id).iter().find_map(|n| n.text.as_ref()) {
                Some(t) => {
                    let right = t.content.origin.x.0 + t.content.size.width.0;
                    let _ = write!(out, " number={:?} number_right={}", t.text.text, len(right));
                    if let Some(span) = t.text.spans.first() {
                        let _ = write!(out, " number_color={}", color(span.style.color));
                    }
                }
                // the dot: its size and look (a folded item's is ringed)
                None => {
                    let look = styles.get(&bullet.id).map(view_style).unwrap_or_default();
                    let _ = write!(out, " dot={}{look}", len(bullet.bounds.size.width.0));
                }
            }
        }
        if let Some(check) = inside.iter().find(|n| is(n, "checkbox")) {
            let _ = write!(out, " checkbox={}", len(check.bounds.origin.x.0));
        }
        let content = inside.iter().find(|n| is(n, "row-text") || is(n, "row-input"));
        if let Some(c) = content {
            let _ = write!(out, " text={}", len(c.bounds.origin.x.0));
            // a color label: the content's background
            if let Some(bg) = styles.get(&c.id).map(|s| s.background_color).filter(|c| !transparent(*c)) {
                let _ = write!(out, " label={}", color(bg));
            }
        }
        // the row being edited (`.o-current`)
        if inside.iter().any(|n| is(n, "row-input")) {
            let _ = write!(out, " current");
        }
        let _ = writeln!(out);
        if let Some(c) = content {
            if c.text.is_some() || c.kind == ViewKind::Input {
                // a one-paragraph content is the text view itself
                write_texts_of(out, app, c, "  ");
            }
            write_texts(out, app, tree, c.id, "  ");
        }
        for note in inside.iter().filter(|n| is(n, "note")) {
            let _ = writeln!(out, "  note top={}", len(note.bounds.origin.y.0));
            write_texts(out, app, tree, note.id, "    ");
        }
        // the thread's pieces that start in this row
        let (top, bottom) = (b.origin.y.0, b.origin.y.0 + b.size.height.0);
        for p in thread.iter().filter(|p| p.top >= top - 0.25 && p.top < bottom - 0.25) {
            p.write(out);
        }
    }
    for bar in tree.with_id("toolbar") {
        let b = bar.bounds;
        let _ = writeln!(out, "toolbar top={} height={}", len(b.origin.y.0), len(b.size.height.0));
    }
}

fn write_texts_of(out: &mut String, app: &Headless, n: &LayoutNode, pad: &str) {
    if let Some(t) = &n.text {
        let head = format!("text x={} top={} ", len(t.content.origin.x.0), len(t.content.origin.y.0));
        write_text(out, pad, &head, &t.text);
    } else if n.kind == ViewKind::Input {
        let _ = writeln!(out, "{pad}input x={} {}", len(n.bounds.origin.x.0), input_text(app, n));
    }
}

/// Every view on screen, indented by nesting.
fn views(out: &mut String, app: &Headless, tree: &Tree, styles: &HashMap<NodeId, ViewStyle>) {
    let _ = writeln!(out, "[views]");
    for n in &tree.nodes {
        let pad = "  ".repeat(tree.depth(n));
        let b = n.bounds;
        let _ = write!(out, "{pad}{}", kind(n.kind));
        if let Some(id) = &n.test_id {
            let _ = write!(out, "#{id}");
        }
        let _ = write!(
            out,
            " {},{} {}x{}",
            len(b.origin.x.0),
            len(b.origin.y.0),
            len(b.size.width.0),
            len(b.size.height.0)
        );
        if let Some(s) = styles.get(&n.id) {
            out.push_str(&view_style(s));
        }
        if n.clips {
            out.push_str(" clips");
        }
        if n.hidden {
            out.push_str(" hidden");
        }
        if let Some(s) = &n.scroll {
            let _ = write!(out, " scroll{}", if s.horizontal { "-x" } else { "" });
            if let Some(c) = s.content_size {
                let _ = write!(out, " {}x{}", len(c.width.0), len(c.height.0));
            }
        }
        let _ = writeln!(out);
        if let Some(t) = &n.text {
            write_text(out, &format!("{pad}  "), "", &t.text);
        } else if n.kind == ViewKind::Input {
            let _ = writeln!(out, "{pad}  {}", input_text(app, n));
        }
    }
}

fn snapshot(doc: &str, editing: bool, device: Device, appearance: &str) -> String {
    let (app, styles) = start(doc, editing, device, appearance);
    let tree = Tree::new(shown(&app));
    let mut out = String::new();
    let state = if editing { format!("editing {}", outliner_app::fixture::edited_item(doc)) } else { "at rest".into() };
    let _ = writeln!(
        out,
        "# {doc}, {} {}x{} ({:?}), {state}; clock 2024-06-15 12:00 UTC",
        device.name, device.width, device.height, device.os
    );
    let styles = styles.lock().unwrap();
    model(&mut out, &app, &tree, &styles);
    views(&mut out, &app, &tree, &styles);
    out
}

/// The user appearance both apps' models are checked with.
fn custom_toml() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../design/test/custom.toml")
}

/// The fixtures' documents as the web's model page reads them (`apps/web/test/model`): written next to the
/// goldens, and checked to be current as they are.
#[test]
fn fixture_items() {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    for doc in ["inline", "looks"] {
        let path = golden_dir().join(format!("{doc}.items.jsonl"));
        let got = outliner_app::fixture::items_json(doc);
        if update {
            std::fs::write(&path, &got).unwrap();
        } else {
            let want = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(want == got, "{} is stale: run with UPDATE_SNAPSHOTS=1", path.display());
        }
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

/// A line diff of `want` against `got`: the lines around each change, `-` wanted, `+` got.
fn diff(want: &str, got: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (want.lines().collect(), got.lines().collect());
    // longest common subsequence, for a diff that lines up
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut lines: Vec<(char, usize, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            lines.push((' ', i + 1, a[i]));
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            lines.push(('+', j + 1, b[j]));
            j += 1;
        } else {
            lines.push(('-', i + 1, a[i]));
            i += 1;
        }
    }
    let changed: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.0 != ' ').map(|(k, _)| k).collect();
    let mut out = String::new();
    let mut last = None;
    for (k, (mark, n, line)) in lines.iter().enumerate() {
        if !changed.iter().any(|&c| c.abs_diff(k) <= 2) {
            continue;
        }
        if last.is_some_and(|l| l + 1 != k) {
            out.push_str("...\n");
        }
        last = Some(k);
        let _ = writeln!(out, "{mark} {n:>4} {line}");
    }
    out
}

#[test]
fn structural_snapshots() {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    let dir = golden_dir();
    let mut failed = Vec::new();
    for doc in outliner_app::fixture::DOCS {
        for device in [DESKTOP, PHONE] {
            for editing in [false, true] {
                let name = format!("{doc}-{}-{}.txt", device.name, if editing { "edit" } else { "rest" });
                let got = snapshot(doc, editing, device, "");
                let path = dir.join(&name);
                if update {
                    std::fs::create_dir_all(&dir).unwrap();
                    std::fs::write(&path, &got).unwrap();
                    continue;
                }
                let want = std::fs::read_to_string(&path).unwrap_or_default();
                if want != got {
                    eprintln!("snapshot {name} differs (- golden, + now):\n{}", diff(&want, &got));
                    failed.push(name);
                }
            }
        }
    }
    // a user's appearance over the light theme (the same file the web's model is checked with, qa/model.mjs)
    let custom = std::fs::read_to_string(custom_toml()).unwrap();
    for device in [DESKTOP, PHONE] {
        let name = format!("looks-{}-custom.txt", device.name);
        let got = snapshot("looks", false, device, &custom);
        let path = dir.join(&name);
        if update {
            std::fs::write(&path, &got).unwrap();
            continue;
        }
        let want = std::fs::read_to_string(&path).unwrap_or_default();
        if want != got {
            eprintln!("snapshot {name} differs (- golden, + now):\n{}", diff(&want, &got));
            failed.push(name);
        }
    }
    assert!(
        failed.is_empty(),
        "snapshots differ: {failed:?}; if the change is meant, run `UPDATE_SNAPSHOTS=1 cargo test --test snapshot` \
         and say why in the commit"
    );
}

/// The web app's design tokens as custom properties (`apps/web/src/design/tokens.css`, generated from
/// `design/*.toml`: what `styles.css` uses).
fn web_css() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../apps/web/src/design/tokens.css");
    let css = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // without its comments
    let mut out = String::new();
    let mut rest = css.as_str();
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at..].find("*/").map_or("", |end| &rest[at + end + 2..]);
    }
    out + rest
}

/// The value of declaration `prop` in the first block that starts with `selector` (`:root` for the base,
/// `@media (max-width: 760px)` for phones).
fn decl(css: &str, selector: &str, prop: &str) -> String {
    let at = css.find(&format!("\n{selector}")).unwrap_or_else(|| panic!("no block `{selector}` in tokens.css"));
    let body = &css[at..];
    let body = &body[..body.find('}').unwrap()];
    let key = format!("{prop}:");
    let v = body
        .split(['{', ';'])
        .find_map(|d| d.trim().strip_prefix(&key))
        .unwrap_or_else(|| panic!("no `{prop}` in `{selector}`"));
    v.trim().to_owned()
}

/// A length or a number in a declaration: `18px` → 18, `.8889em` → 0.8889.
fn num(v: &str) -> f32 {
    v.trim_end_matches("px").trim_end_matches("em").parse().unwrap_or_else(|_| panic!("not a number: {v}"))
}

/// `rgba(255, 221, 0, .38)` as the snapshot writes it.
fn rgba_hex(v: &str) -> String {
    let inner = v.trim_start_matches("rgba(").trim_end_matches(')');
    let p: Vec<f32> = inner.split(',').map(|x| x.trim().parse().unwrap()).collect();
    format!("#{:02x}{:02x}{:02x}{:02x}", p[0] as u8, p[1] as u8, p[2] as u8, (p[3] * 255.0).round() as u8)
}

/// Where the web app and the snapshot overlap (sizes, paddings, colors of the design tokens the web's `styles.css`
/// uses), the goldens hold the web's values: the snapshot pins the web's look, not only what the native app draws.
#[test]
fn goldens_hold_the_web_values() {
    let css = web_css();
    let var = |name: &str| decl(&css, ":root", name);
    let phone = |name: &str| decl(&css, "@media (max-width: 760px)", name);
    let n = |name: &str| num(&var(name));
    let efs = n("--efs");
    let line = (efs * n("--lh")).round();
    let pad = n("--pad");
    let fira = var("--font");
    let mono = var("--mono");
    let golden = |name: &str| std::fs::read_to_string(golden_dir().join(name)).unwrap();
    let mut missing = Vec::new();
    let mut want = |file: &str, what: &str, text: String| {
        if !golden(file).contains(&text) {
            missing.push(format!("{file}: {what}: `{text}`"));
        }
    };

    // type and colors of the content (`.o-editor`, `.o-content`, `--fg`), a row's pitch (`--pad`)
    let (fg, row) = (var("--fg"), "inline-desktop-rest.txt");
    let bold = n("--type-bold-weight");
    want(row, "text", format!("\"Bold\" {fira} {}/{} w{bold} {fg}", len(efs), len(line)));
    want(row, "row pitch", format!("row depth=0 top=111 height={}", len(line + 2.0 * pad)));
    // `.o-title .o-content`
    let title = efs * n("--type-title");
    let title_line = title * n("--type-title-line");
    let tw = n("--type-title-weight");
    want(row, "title", format!("\"Inline\" {fira} {}/{} w{tw} {fg}", len(title), len(title_line)));
    // `.o-code`
    let code = efs * n("--type-code");
    let code_look = format!(
        "{mono} {}/{} w400 {} bg={} r={}",
        len(code),
        len(line),
        var("--code-fg"),
        var("--code-bg"),
        len(n("--radius-code"))
    );
    want(row, "code", format!("\"let x = 1;\" {code_look}"));
    // `.o-tag`, `.o-link`, `.o-image`
    want(row, "tag", format!("\"#tag\" {fira} {}/{} w400 {} underline", len(efs), len(line), var("--tag")));
    want(row, "link", format!("\"link\" {fira} {}/{} w400 {fg} underline", len(efs), len(line)));
    want(row, "image", format!("\"a picture\" {fira} {}/{} w400 {} underline", len(efs), len(line), var("--accent")));
    // `.o-highlight, .o-mark`
    want(row, "highlight", format!("bg={} r={}", rgba_hex(&var("--highlight")), len(n("--radius-mark"))));
    // `.o-note`
    let note = efs * n("--type-note");
    let note_line = note * n("--type-note-line");
    want(row, "note", format!("{fira} {}/{} w400 {}", len(note), len(note_line), var("--note")));
    // `.o-checked`
    want(row, "checked", format!("w400 {} strike", var("--check")));
    // `.o-codeblock`: in the mono font, the text's color
    let block = efs * n("--type-codeblock");
    let block_line = block * n("--lh");
    want(row, "code block color", format!("console.log(a);\" {mono} {}/{} w400 {fg}", len(block), len(block_line)));

    // `.o-h1` .. `.o-h3`, labels `--cl1` .. `--cl6`, bullets, checkboxes, guides
    let looks = "looks-desktop-rest.txt";
    let hw = n("--type-heading-weight");
    for (h, name) in [(1, "one"), (2, "two"), (3, "three")] {
        let size = efs * n(&format!("--type-h{h}"));
        let lh = size * n(&format!("--type-h{h}-line"));
        want(looks, "heading", format!("\"Heading {name}\" {fira} {}/{} w{hw} {fg}", len(size), len(lh)));
    }
    for k in 1..=6 {
        want(looks, "color label", format!("label={}", var(&format!("--cl{k}"))));
    }
    let muted = var("--muted");
    let dot = n("--space-dot");
    let ring = n("--space-fold-ring");
    want(looks, "bullet", format!("dot={} bg={muted} r={}", len(dot), len(dot / 2.0)));
    let folded = dot + 2.0 * ring;
    want(
        looks,
        "folded bullet",
        format!("dot={} bg={muted} r={} border={} {}", len(folded), len(folded / 2.0), len(ring), var("--faint")),
    );
    let check = len(n("--space-check"));
    let (cr, cb) = (len(n("--radius-check")), len(n("--space-check-border")));
    want(looks, "checkbox", format!("{check}x{check} bg={} r={cr} border={cb} {}", var("--bg"), var("--check")));
    want(looks, "checked checkbox", format!("{check}x{check} bg={muted} r={cr} border={cb} {muted}"));
    want(looks, "guide", format!("{}x{} bg={}", len(n("--space-guide-w")), len(line + 2.0 * pad), var("--guide")));
    // the document's padding (`.doc-wrap`), the indent (`--kid-pad` less the guide's offset), `.o-body` gap
    let doc_left = n("--space-doc-left");
    // the children block's guide line is its left border
    let indent = n("--kid-pad") - n("--space-guide") + n("--space-guide-w");
    let gap = n("--space-body-gap");
    want(looks, "text left", format!("text={}", len(doc_left + gap)));
    want(looks, "indent", format!("text={}", len(doc_left + indent + gap)));
    want(looks, "bullet center", format!("bullet={}", len(doc_left - n("--space-bullet") / 2.0)));

    // the bullet thread (`.o-thread`): its width and color, the bullets on it, its turn's radius
    let edit = "looks-desktop-edit.txt";
    let thread = var("--thread");
    let thread = match thread.strip_prefix("var(").and_then(|v| v.strip_suffix(')')) {
        Some(v) => var(v),
        None => thread,
    };
    let tw = len(n("--thread-w"));
    want(edit, "thread", format!(" w={tw} {thread}"));
    want(edit, "thread turn", format!(" w={tw} r={} {thread}", len(n("--radius-elbow"))));
    want(edit, "bullet on the thread", format!("dot={} bg={thread} r={}", len(dot), len(dot / 2.0)));

    // `.topbar`: 44px on desktops, 46px on phones; `.mobile-bar`: 41px with a top border
    let line_c = var("--line");
    let border = len(n("--space-border"));
    // the bar's height is above its bottom border (`.topbar` is content-box)
    let bar = len(n("--space-topbar") + n("--space-border"));
    want(looks, "top bar", format!("view#topbar 0,0 1100x{bar} bg={} border-b={border} {line_c}", var("--bg")));
    let phone_bar = len(num(&phone("--space-topbar")) + n("--space-border"));
    want("looks-phone-rest.txt", "phone top bar", format!("view#topbar 0,0 390x{phone_bar}"));
    let toolbar = len(n("--space-toolbar"));
    want("looks-phone-edit.txt", "phone toolbar", format!("390x{toolbar} bg={} border-t={border} {line_c}", var("--bg")));
    let phone_left = num(&phone("--space-doc-left")) + gap;
    want("inline-phone-rest.txt", "phone text left", format!("text={}", len(phone_left)));

    assert!(missing.is_empty(), "the goldens do not hold these values of the design tokens:\n{}", missing.join("\n"));
}

// ---- design/spec.json: the relations of the look, on the native app's model ----

/// The `[model]` of `doc` at rest in `modes` on `device`.
fn model_in(doc: &str, device: Device, modes: &str) -> String {
    let (app, styles) = start_with(doc, false, device, modes, "");
    let tree = Tree::new(shown(&app));
    let mut out = String::new();
    model(&mut out, &app, &tree, &styles.lock().unwrap());
    out
}

/// A text of the model: where it is, and its first span's size, line and color.
#[derive(Default, Debug)]
struct MText {
    top: f32,
    x: f32,
    first: Option<(f32, f32, String)>,
}

#[derive(Default, Debug)]
struct MRow {
    depth: f32,
    top: f32,
    height: f32,
    bullet: f32,
    dot_color: Option<String>,
    checkbox: Option<f32>,
    text: f32,
    label: Option<String>,
    texts: Vec<MText>,
    note: Option<(f32, Vec<MText>)>,
}

#[derive(Default, Debug)]
struct Model {
    topbar: f32,
    title: Vec<MText>,
    rows: Vec<MRow>,
}

fn attr(line: &str, key: &str) -> Option<String> {
    let at = line.find(&format!(" {key}="))?;
    Some(line[at + key.len() + 2..].split(' ').next()?.to_owned())
}

fn attr_num(line: &str, key: &str) -> Option<f32> {
    attr(line, key).and_then(|v| v.parse().ok())
}

/// The parts of a model the relations measure (qa/lib/model.mjs reads the same text on the web's side).
fn parse_model(text: &str) -> Model {
    let mut m = Model::default();
    // where texts go: 0 the title, 1 the last row, 2 the last row's note
    let mut into = 0;
    for line in text.lines() {
        let t = line.trim_start();
        let l = format!(" {t}");
        if t.starts_with("topbar ") {
            m.topbar = attr_num(&l, "height").unwrap();
        } else if t.starts_with("title ") {
            into = 0;
        } else if t.starts_with("row ") {
            m.rows.push(MRow {
                depth: attr_num(&l, "depth").unwrap(),
                top: attr_num(&l, "top").unwrap(),
                height: attr_num(&l, "height").unwrap(),
                bullet: attr_num(&l, "bullet").unwrap_or(0.0),
                dot_color: if t.contains(" dot=") { attr(&l, "bg") } else { None },
                checkbox: attr_num(&l, "checkbox"),
                text: attr_num(&l, "text").unwrap_or(0.0),
                label: attr(&l, "label"),
                ..MRow::default()
            });
            into = 1;
        } else if t.starts_with("note ") {
            m.rows.last_mut().unwrap().note = Some((attr_num(&l, "top").unwrap(), Vec::new()));
            into = 2;
        } else if t.starts_with("text ") || t.starts_with("input ") || t.starts_with('"') {
            let list = match into {
                0 => &mut m.title,
                1 => &mut m.rows.last_mut().unwrap().texts,
                _ => &mut m.rows.last_mut().unwrap().note.as_mut().unwrap().1,
            };
            if !t.starts_with('"') {
                list.push(MText { top: attr_num(&l, "top").unwrap_or(0.0), x: attr_num(&l, "x").unwrap(), first: None });
                continue;
            }
            let Some(last) = list.last_mut() else { continue };
            if last.first.is_none() {
                // `"text" family size/line wN #color ...`
                last.first = t.rsplit_once(" w").and_then(|(head, rest)| {
                    let (size, line) = head.rsplit(' ').next()?.split_once('/')?;
                    Some((size.parse().ok()?, line.parse().ok()?, rest.split(' ').nth(1)?.to_owned()))
                });
            }
        }
    }
    m
}

/// What the relations need of a fixture's visible items, in row order (`*.items.jsonl`).
#[derive(Debug, Default)]
struct Item {
    heading: i64,
    note: bool,
    checked: bool,
    checkbox: bool,
    label: i64,
    numbered: bool,
    code: bool,
    latex: bool,
    long: bool,
}

fn fixture_rows(doc: &str) -> Vec<Item> {
    let text = std::fs::read_to_string(golden_dir().join(format!("{doc}.items.jsonl"))).unwrap();
    let items: Vec<serde_json::Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    fn walk(items: &[serde_json::Value], id: &str, out: &mut Vec<Item>) {
        let numbered = items.iter().find(|x| x["id"] == id).is_some_and(|p| p["props"]["num"] == true);
        for k in items.iter().filter(|x| x["parent"] == id) {
            let p = &k["props"];
            let c = p["c"].as_str().unwrap_or("");
            out.push(Item {
                heading: p["h"].as_i64().unwrap_or(0),
                note: p["n"].as_str().is_some_and(|n| !n.trim().is_empty()),
                checked: p["ck"] == true,
                checkbox: p["cb"] == true,
                label: p["cl"].as_i64().unwrap_or(0),
                numbered,
                code: c.contains("```"),
                latex: c.contains("$$"),
                long: c.encode_utf16().count() > 24,
            });
            if k["collapsed"] != true {
                walk(items, k["id"].as_str().unwrap(), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&items, "root", &mut out);
    out
}

/// The row kinds of design/spec.json.
fn kind_of(name: &str, r: &Item) -> bool {
    match name {
        "plain" => r.heading == 0 && !r.note && !r.code && !r.checked && !r.checkbox && !r.latex && !r.long,
        "dot" => !r.numbered && !r.checkbox,
        "heading1" => r.heading == 1,
        "heading2" => r.heading == 2,
        "heading3" => r.heading == 3,
        "note" => r.note,
        "checkbox" => r.checkbox,
        "checked" => r.checked && r.heading == 0,
        "label" => r.label > 0,
        _ => panic!("unknown row kind {name}"),
    }
}

/// An expression over design entries: numbers, `table.key`, + - * / and parentheses.
fn evaluate(expr: &str, look: &outliner_app::appearance::Look) -> f32 {
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in expr.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            cur.push(c);
            continue;
        }
        if !cur.is_empty() {
            toks.push(std::mem::take(&mut cur));
        }
        if "+-*/()".contains(c) {
            toks.push(c.to_string());
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    struct P<'a> {
        toks: Vec<String>,
        i: usize,
        look: &'a outliner_app::appearance::Look,
    }
    impl P<'_> {
        fn peek(&self) -> &str {
            self.toks.get(self.i).map_or("", |s| s.as_str())
        }
        fn next(&mut self) -> String {
            self.i += 1;
            self.toks[self.i - 1].clone()
        }
        fn atom(&mut self) -> f32 {
            let t = self.next();
            match t.as_str() {
                "(" => {
                    let v = self.sum();
                    self.next();
                    v
                }
                "-" => -self.atom(),
                _ => t.parse().unwrap_or_else(|_| self.look.number(&t)),
            }
        }
        fn prod(&mut self) -> f32 {
            let mut v = self.atom();
            while matches!(self.peek(), "*" | "/") {
                v = if self.next() == "*" { v * self.atom() } else { v / self.atom() };
            }
            v
        }
        fn sum(&mut self) -> f32 {
            let mut v = self.prod();
            while matches!(self.peek(), "+" | "-") {
                v = if self.next() == "+" { v + self.prod() } else { v - self.prod() };
            }
            v
        }
    }
    P { toks, i: 0, look }.sum()
}

fn hex(c: u32) -> String {
    if c & 0xff == 0xff { format!("#{:06x}", c >> 8) } else { format!("#{c:08x}") }
}

/// The broken relations of `spec` in a model of `doc` in `modes` on `device` (qa/lib/model.mjs `checkSpec` on
/// the web's side).
fn check_spec(spec: &serde_json::Value, m: &Model, doc: &str, modes: &str, device: Device) -> Vec<String> {
    use outliner_app::appearance::{Looks, Modes};
    let looks = Looks::new(Modes::parse(modes).unwrap(), Default::default());
    let look = if device.name == "phone" { &looks.phone } else { &looks.desktop };
    let items = fixture_rows(doc);
    let mut out = Vec::new();
    for rel in spec["relations"].as_array().unwrap() {
        let name = rel["name"].as_str().unwrap();
        let measure = rel["measure"].as_str().unwrap();
        let expect = rel["expect"].as_str().unwrap();
        let tol = rel["tolerance"].as_f64().unwrap_or(0.0) as f32;
        let color_of = |item: &Item| {
            hex(look.color(&if expect == "label" { format!("label.{}", item.label) } else { expect.to_owned() }))
        };
        let mut check = |what: String, got: f32| {
            let want = evaluate(expect, look);
            if (got - want).abs() > tol {
                out.push(format!("{name} ({what}): {got}, want {want}"));
            }
        };
        let first_line = |t: &MText| t.first.as_ref().map_or(0.0, |f| f.1);
        match measure {
            "topbar.height" => check("top bar".into(), m.topbar),
            "title.text_left" => check("title".into(), m.title[0].x - m.rows[0].text),
            "title.to_first_row" => check("title".into(), m.rows[0].top - (m.title[0].top + first_line(&m.title[0]))),
            _ => {
                let mut colors = Vec::new();
                for (i, row) in m.rows.iter().enumerate() {
                    let Some(item) = items.get(i) else { continue };
                    match rel["rows"].as_str() {
                        Some("child") if i == 0 || row.depth != m.rows[i - 1].depth + 1.0 => continue,
                        Some("child") | None => {}
                        Some(k) if !kind_of(k, item) => continue,
                        Some(_) => {}
                    }
                    let w = format!("row {i}");
                    let first = &row.texts[0];
                    match measure {
                        "row.height" => check(w, row.height),
                        "row.text_top" => check(w, first.top - row.top),
                        "row.bullet_to_text" => {
                            if row.dot_color.is_some() {
                                check(w, row.text - row.bullet);
                            }
                        }
                        "row.indent" => check(w, row.text - m.rows[i - 1].text),
                        "note.gap" => {
                            if let Some((top, _)) = &row.note {
                                check(w, top - (first.top + first_line(first)));
                            }
                        }
                        "row.checkbox_to_text" => check(w, row.text - row.checkbox.unwrap()),
                        "span.size" => check(w, first.first.as_ref().unwrap().0),
                        "note.size" => {
                            if let Some((_, t)) = &row.note {
                                check(w, t[0].first.as_ref().unwrap().0);
                            }
                        }
                        "span.color" => colors.push((w, first.first.as_ref().unwrap().2.clone(), color_of(item))),
                        "row.dot_color" => {
                            if let Some(c) = &row.dot_color {
                                colors.push((w, c.clone(), color_of(item)));
                            }
                        }
                        "row.label" => colors.push((w, row.label.clone().unwrap_or_default(), color_of(item))),
                        _ => panic!("unknown measure {measure}"),
                    }
                }
                for (w, got, want) in colors {
                    if got != want {
                        out.push(format!("{name} ({w}): {got}, want {want}"));
                    }
                }
            }
        }
    }
    out
}

/// design/spec.json holds on the native app's model, in each of its modes, on a desktop and a phone (the web's
/// model is checked by qa/model.mjs).
#[test]
fn the_look_keeps_the_spec() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../design/spec.json");
    let spec: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut broken = Vec::new();
    for modes in spec["modes"].as_array().unwrap() {
        let words: Vec<String> =
            modes.as_object().unwrap().iter().map(|(g, m)| format!("{g}={}", m.as_str().unwrap())).collect();
        let modes = words.join(" ");
        for device in [DESKTOP, PHONE] {
            for doc in ["looks", "inline"] {
                let m = parse_model(&model_in(doc, device, &modes));
                for p in check_spec(&spec, &m, doc, &modes, device) {
                    broken.push(format!("{doc} {} [{modes}]: {p}", device.name));
                }
            }
        }
    }
    assert!(broken.is_empty(), "the native look breaks design/spec.json:\n{}", broken.join("\n"));
}
