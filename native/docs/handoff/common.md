# Native app: common handoff notes

Read this first. Area notes in this folder say what each piece of work is.

## Where things are

- `native/` is a Cargo workspace (Rust stable). The web app (`apps/web`, TS) and the server (`apps/server`) stay as
  they are; the native app (Android + macOS) syncs with the same server and must match the web app's behavior and look.
- `crates/outliner-core`: Rust port of `packages/core` (no UI deps). Done so far: tree, item, ids, session (undo,
  sync), history, tx, view, commands, text16 (UTF-16 offsets). Not yet: inline, dates, search, paste, export, refs,
  calendar.
- `crates/outliner-app`: the UI on NSH (Native Shell, https://github.com/longbridge/nsh, crates pinned `=0.5.0`).
  `store.rs` (open document, rows, editing caret, view), `commands.rs` (app commands over core commands),
  `outline.rs` (virtual list, rows, row input, key bindings), `toolbar.rs` (top bar, phone toolbar), `style.rs`
  (colors and metrics from `apps/web/src/styles.css`), `icons.rs` (web icons as inline SVG), `inline.rs` (a
  temporary subset parser, to be replaced by the core port).
- `crates/outliner-mac`: macOS binary, NSH's GPUI backend. Builds on Linux too (GPUI dev libs).
- CI: `.github/workflows/ci.yml` job `native-core` (fixtures + tests + clippy); `.github/workflows/native.yml` builds
  the macOS .app and the Android APK as artifacts on every push touching `native/`.
- `TASKS.md` section 4 tracks the native app; update it when you finish something.

## How we verify

- Core ports are checked against the TS code: a generator in `crates/outliner-core/fixtures/*.ts` (run with
  `node`, from the repo root) records what TS does on seeded random input into a JSON fixture (gitignored), and a
  test in `crates/outliner-core/tests/` replays it. Add a generator + test for every new port. Check the test catches
  bugs: copy the crate to a scratch dir and mutate it there, with its own `CARGO_TARGET_DIR` (sharing the target dir
  with the real crate mixes builds and gives false results). Never revert files in the repo with git to undo a mutation.
- App behavior: headless tests in `crates/outliner-app/tests/flows.rs` (`nsh-testing` 0.5.0: `press`, `type_text`,
  `submit`, `press_key`, `scroll`, `screen()`; no IME `compose` step in 0.5.0; no access to app state, so assert on
  what the screen shows). Set `config.environment.os = Os::MacOs` (+ `Idiom::Desktop`) for desktop keys.
- Look: the structural snapshot is how the look is checked; the offscreen GPUI render only makes pictures to look at.
  No screenshot comparison of any kind, no Xvfb / xwd (the user asked for deterministic checks). Values to match come
  from `apps/web/src/styles.css`.
  - Fixtures: `outliner_app::fixture::root(doc, editing)` opens the app on `demo` (first screen of the demo),
    `inline` (the inline showcase) or `looks` (headings, checked items, checkboxes, the six labels, a folded item,
    four levels), with `edited_item(doc)` being edited or not. Add a document there (and to `DOCS`) to cover more.
  - Structural snapshot: `crates/outliner-app/tests/snapshot.rs` renders each fixture as a desktop (`Os::MacOs`,
    1100x800) and a phone (`Os::Ios`, 390x844), at rest and editing, on a manual clock (2024-06-15 12:00), `TZ=UTC`,
    reduced motion and nsh-testing's approximate text engine (no fonts involved), and writes
    `tests/snapshots/<doc>-<device>-<rest|edit>.txt`: `[model]` (top bar, title, rows: depth, top, height, bullet
    center, dot or number, checkbox, text left, label, texts with their styled spans, notes; phone toolbar: the
    render model area 6 shares with the web) then `[views]` (every view with an area, in tree order: kind, test id,
    bounds rounded to 0.5pt, background / radius / borders from the mount batches, texts with spans, inputs).
    A mismatch prints a line diff per file. After a meant change: `UPDATE_SNAPSHOTS=1 cargo test -p outliner-app
    --test snapshot`, read the diff of the goldens, and say why they changed in the commit message. The test ids
    the model reads (`topbar`, `title`, `row`, `guide`, `bullet`, `checkbox`, `row-text`, `row-input`, `note`,
    `toolbar`) are in `outline.rs` / `toolbar.rs`; keep them when restructuring views. `goldens_hold_the_web_values`
    reads `styles.css` and checks the goldens hold its values (type, colors, pitch, indent, bars): when the web
    changes a value, both that test and the snapshot tell you.
  - Offscreen render: `cargo run -p outliner-mac --features shot --bin shot -- <dir>` (`src/bin/shot.rs`) draws the
    same fixtures with GPUI's test platform (`HeadlessAppContext` + the platform's headless renderer: Metal on
    macOS, wgpu on Linux) to PNGs at scale 2. No window, no X server; on Linux it needs a Vulkan driver (Mesa
    `mesa-vulkan-drivers`, lavapipe) and GPUI's dev libs. CI (`native.yml`, macOS) uploads them as the artifact
    `Outliner-shots`; nothing compares them. Phones are laid out with `OUTLINER_PHONE=1` there (the test platform
    is a desktop), and the clock is the system's (the fixtures' dates are fixed days, so labels do not move).
- Before committing: `cargo test` and `cargo clippy --all-targets -- -D warnings` in `native/`, `npm run typecheck`
  at the root if you touched TS.

## Gotchas found so far

- TS `Tree.toJSON()` shares the live node objects: `structuredClone` before mutating if you record a "before" state.
- Rust strings cannot hold a lone UTF-16 surrogate; generators must cut text at code point boundaries.
- Text offsets everywhere are UTF-16 units (`outliner_core::text16`).
- NSH virtual list recycles cells: anything made per row must be keyed by the item id (see `editing` in
  `outline.rs`), and spare cells sit out of view (size 0) holding their last row.
- NSH `on_key_with(.., When::Always, ..)` gets keys while a text input has focus (desktop only).
- Fonts: crate `assets/` are shipped by `nsh package` (Android); the GPUI binary passes them with `Root::font`
  (`outliner_app::FONTS`). Fira Sans has no Hangul: the platform falls back.
- NSH text spans have no padding; an inline view in a text becomes U+FFFC in the text (bad for copy/a11y).
- `nsh package` creates `crates/outliner-app/target` and `.nsh/`: both gitignored, never commit build output
  (check `git status` before `git add`).

## Conventions

- Commit on branch `ccr-b10d0d65-ax4wtp`, push with `git push -u origin ccr-b10d0d65-ax4wtp`. Pull before you start.
  Commit messages in English, as the existing ones (a subject line, a wrapped body saying what and why), ending with
  the attribution lines your session's system prompt gives.
- Code comments in English, plain and specific, as the existing code.
- Talk to the orchestration session (the one that started you) in Korean; keep other output short.
