# Area 5: deterministic look checks (structural snapshot + offscreen render)

The user wants the UI checked deterministically, replacing the Xvfb screenshots (do not use Xvfb / xwd any more).
Two tools, both in CI. This comes right after area 1; areas 2-4 then use them as their "done when".

## 1. Structural snapshot

A headless NSH test (`nsh-testing` 0.5.0) that renders fixed documents and dumps what is on screen in a stable text
form, checked in as golden files and compared byte for byte:

- every view: kind, test id, bounds (rounded to 0.5pt), in tree order;
- every text and its spans: characters, and the resolved style (`screen().span_style(..)`: family, size, weight,
  italic, color, decoration, background);
- inputs: their text.

Fixtures: the demo document's first screen, one exercising every inline kind, headings, checked items, checkboxes,
color labels, notes and numbered lists; each as desktop (`Os::MacOs` + `Idiom::Desktop`, 1100x800) and phone
(`Os::Ios`/`Os::Android`, 390x844), at rest and with a row being edited. Fixed clock, fixed ids, reduced motion,
bundled fonts only, so the output does not depend on the machine.

A mismatch prints a readable diff. An env var (for example `UPDATE_SNAPSHOTS=1`) rewrites the goldens; the commit
that changes them must say why. Check that the web app's values are what the goldens say where they overlap
(sizes, paddings, colors from `apps/web/src/styles.css`), so the snapshot pins the web's look, not just today's.

## 2. Offscreen render (GPUI)

`nsh-platform-gpui` has a `shot` feature: GPUI's test platform (`TestAppContext`) renders a window's frame
offscreen, with no X server or window manager (see `crates/gpui/nsh-platform-gpui/src/tests.rs` in the NSH repo,
https://github.com/longbridge/nsh, for how its own tests use it). Use it to render the same fixtures to PNG at a fixed
size and scale:

- on the macOS CI runner (the platform the app ships on); find out whether it works on Linux too and say so;
- compare with checked-in reference PNGs with a small per-pixel tolerance and a max differing-pixel ratio, and
  upload the PNGs and a diff image as CI artifacts;
- references are updated the same way as the snapshots (env var, and the commit says why).

If the `shot` path cannot work from outside the NSH workspace at 0.5.0, stop and report what blocks it with the
evidence, instead of falling back to Xvfb.

## Done when

Both run in CI (the snapshot in `ci.yml` with the other Rust tests, the offscreen render in `native.yml` on macOS),
they fail on a deliberate one-pixel / one-point change and pass again when it is reverted, `common.md` describes how
to use and update them, TASKS.md updated, committed and pushed, working tree clean.
