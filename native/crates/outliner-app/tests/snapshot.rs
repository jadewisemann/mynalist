//! The structural snapshot: the fixtures of `outliner_app::fixture` (the demo's first screen, the inline showcase,
//! the item looks), each as a desktop and a phone, at rest and with a row being edited, rendered headless and
//! dumped as text: the render model (top bar, title, rows with their depth, bullet center, text left, top, height
//! and styled spans; the phone toolbar), then every view on screen. The dumps are compared byte for byte with the
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

fn start(doc: &str, editing: bool, device: Device) -> (Headless, Styles) {
    // the dates' local zone (every snapshot sets the same)
    std::env::set_var("TZ", "UTC");
    let edited = editing.then(|| outliner_app::fixture::edited_item(doc));
    let mut config = EngineConfig::new(outliner_app::fixture::root(doc, edited));
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

/// The render model area 6 shares with the web app.
fn model(out: &mut String, app: &Headless, tree: &Tree, styles: &HashMap<NodeId, ViewStyle>) {
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

fn snapshot(doc: &str, editing: bool, device: Device) -> String {
    let (app, styles) = start(doc, editing, device);
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
                let got = snapshot(doc, editing, device);
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
    assert!(
        failed.is_empty(),
        "snapshots differ: {failed:?}; if the change is meant, run `UPDATE_SNAPSHOTS=1 cargo test --test snapshot` \
         and say why in the commit"
    );
}

/// The web app's stylesheet.
fn web_css() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../apps/web/src/styles.css");
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

/// The value of declaration `prop` in the first rule whose selector starts with `selector` (`:root` for the light
/// theme's custom properties).
fn decl(css: &str, selector: &str, prop: &str) -> String {
    let at = css.find(&format!("\n{selector}")).unwrap_or_else(|| panic!("no rule `{selector}` in styles.css"));
    let body = &css[at..];
    let body = &body[body.find('{').unwrap() + 1..body.find('}').unwrap()];
    let key = format!("{prop}:");
    let v = body
        .split(';')
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

/// Where the web app and the snapshot overlap (sizes, paddings, colors of `apps/web/src/styles.css`), the goldens
/// hold the web's values: the snapshot pins the web's look, not only what the native app draws today.
#[test]
fn goldens_hold_the_web_values() {
    let css = web_css();
    let var = |name: &str| decl(&css, ":root", name);
    let efs = num(&var("--efs"));
    let line = (efs * num(&var("--lh"))).round();
    let pad = num(&var("--pad"));
    let fira = "Fira Sans, system-ui, sans-serif";
    let mono = "Fira Mono, ui-monospace, monospace";
    let em = |sel: &str, prop: &str| num(&decl(&css, sel, prop));
    let golden = |name: &str| std::fs::read_to_string(golden_dir().join(name)).unwrap();
    let mut missing = Vec::new();
    let mut want = |file: &str, what: &str, text: String| {
        if !golden(file).contains(&text) {
            missing.push(format!("{file}: {what}: `{text}`"));
        }
    };

    // type and colors of the content (`.o-editor`, `.o-content`, `--fg`), a row's pitch (`--pad`)
    let (fg, row) = (var("--fg"), "inline-desktop-rest.txt");
    want(row, "text", format!("\"Bold\" {fira} {}/{} w700 {fg}", len(efs), len(line)));
    want(row, "row pitch", format!("row depth=0 top=110 height={}", len(line + 2.0 * pad)));
    // `.o-title .o-content`
    let title = efs * em(".o-title .o-content", "font-size");
    let title_line = title * num(&decl(&css, ".o-title .o-content", "line-height"));
    want(row, "title", format!("\"Inline\" {fira} {}/{} w400 {fg}", len(title), len(title_line)));
    // `.o-code`
    let code = efs * em(".o-code", "font-size");
    let code_look =
        format!("{mono} {}/{} w400 {} bg={} r=4", len(code), len(line), var("--code-fg"), var("--code-bg"));
    want(row, "code", format!("\"let x = 1;\" {code_look}"));
    // `.o-tag`, `.o-link`, `.o-image`
    want(row, "tag", format!("\"#tag\" {fira} {}/{} w400 {} underline", len(efs), len(line), var("--tag")));
    want(row, "link", format!("\"link\" {fira} {}/{} w400 {fg} underline", len(efs), len(line)));
    want(row, "image", format!("\"a picture\" {fira} {}/{} w400 {} underline", len(efs), len(line), var("--accent")));
    // `.o-highlight, .o-mark`
    want(row, "highlight", format!("bg={} r=2", rgba_hex(&decl(&css, ".o-highlight, .o-mark", "background"))));
    // `.o-note`
    let note = efs * em(".o-note {", "font-size");
    let note_line = note * num(&decl(&css, ".o-note {", "line-height"));
    want(row, "note", format!("{fira} {}/{} w400 {}", len(note), len(note_line), var("--note")));
    // `.o-checked`
    want(row, "checked", format!("w400 {} strike", var("--check")));
    // `.o-codeblock`: .85em in the mono font, the text's color
    let block = efs * em(".o-codeblock", "font-size");
    want(row, "code block", format!("{mono} {}/", len(block)));
    want(row, "code block color", format!("console.log(a);\" {mono} {}/18.5 w400 {fg}", len(block)));

    // `.o-h1` .. `.o-h3`, labels `--cl1` .. `--cl6`, bullets, checkboxes, guides
    let looks = "looks-desktop-rest.txt";
    for (h, name) in [(1, "one"), (2, "two"), (3, "three")] {
        let sel = format!(".o-h{h} > .o-row .o-content");
        let size = efs * em(&sel, "font-size");
        let lh = size * num(&decl(&css, &sel, "line-height"));
        want(looks, "heading", format!("\"Heading {name}\" {fira} {}/{} w700 {fg}", len(size), len(lh)));
    }
    for k in 1..=6 {
        want(looks, "color label", format!("label={}", var(&format!("--cl{k}"))));
    }
    let muted = var("--muted");
    want(looks, "bullet", format!("dot=6 bg={muted} r=3"));
    want(looks, "folded bullet", format!("dot=14 bg={muted} r=7 border=4 {}", var("--faint")));
    want(looks, "checkbox", format!("16x16 bg={} r=3 border=1.5 {}", var("--bg"), var("--check")));
    want(looks, "checked checkbox", format!("16x16 bg={muted} r=3 border=1.5 {muted}"));
    want(looks, "guide", format!("1x30 bg={}", var("--guide")));
    // the document's padding (`.doc-wrap`), the indent (`--kid-pad` less `.o-kids` margin), `.o-body` gap
    let doc_left = num(decl(&css, ".doc-wrap {", "padding").split(' ').nth(3).unwrap());
    let indent = num(&decl(&css, ".o-editor { --kid-pad", "--kid-pad")) - 9.0;
    let gap = num(&decl(&css, ".o-body", "padding-left"));
    want(looks, "text left", format!("text={}", len(doc_left + gap)));
    want(looks, "indent", format!("text={}", len(doc_left + indent + gap)));
    want(looks, "bullet center", format!("bullet={}", len(doc_left - 9.0)));

    // `.topbar`: 44px on desktops, 46px on phones; `.mobile-bar`: 41px with a top border
    let line_c = var("--line");
    let bar = len(num(&decl(&css, ".topbar {", "height")));
    want(looks, "top bar", format!("view#topbar 0,0 1100x{bar} bg={} border-b=1 {line_c}", var("--bg")));
    want("looks-phone-rest.txt", "phone top bar", "view#topbar 0,0 390x46".into());
    want("looks-phone-edit.txt", "phone toolbar", format!("390x41 bg=#ffffff border-t=1 {line_c}"));
    want("inline-phone-rest.txt", "phone text left", "text=36".into());

    assert!(missing.is_empty(), "the goldens do not hold these values of styles.css:\n{}", missing.join("\n"));
}
