//! The inline showcase (`OUTLINER_DEMO=inline`), headless: every inline kind drawn as the web app draws it, the
//! note under its item and the numbers of a numbered list. Every test here opens the showcase.

use nsh_core::platform::{Idiom, Os};
use nsh_core::prelude::rgb;
use nsh_core::{Color, ResolvedTextStyle};
use nsh_engine::{EngineConfig, Root};
use nsh_testing::{Headless, LayoutNode};

fn start(os: Os) -> Headless {
    // every test of this file sets the same value
    std::env::set_var("OUTLINER_DEMO", "inline");
    let mut config = EngineConfig::new(Root::of::<outliner_app::App>());
    config.environment.os = os;
    if os == Os::MacOs {
        // a desktop window, as the snapshot's: the sidebar's panel shows beside the document
        config.environment.idiom = Idiom::Desktop;
        config.viewport = nsh_core::Size { width: nsh_core::Points(1100.0), height: nsh_core::Points(800.0) };
    }
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:Numbered list").unwrap();
    app
}

fn style(app: &Headless, selector: &str) -> ResolvedTextStyle {
    app.screen().span_style(selector).unwrap_or_else(|e| panic!("{selector}: {e}"))
}

fn hex(c: u32) -> Color {
    Color::from(rgb(c))
}

/// The laid-out text view reading `text` exactly (spare list cells, sized 0, left out).
fn text_node(app: &Headless, text: &str) -> LayoutNode {
    app.screen()
        .layout_nodes()
        .into_iter()
        .find(|n| n.bounds.size.height.0 > 0.0 && n.text.as_ref().is_some_and(|t| t.text.text.as_str() == text))
        .unwrap_or_else(|| panic!("no text {text:?}"))
}

#[test]
fn emphasis_code_and_escapes() {
    let app = start(Os::Ios);
    // the markup is gone: one text of spans
    assert!(app.screen().shows("Bold, italic, strike and highlight"));
    assert_eq!(style(&app, "text:Bold").font_weight, 700);
    assert!(style(&app, "text:italic").italic);
    assert!(style(&app, "text:strike").decoration.line_through);
    assert!(style(&app, "text:highlight").background.is_some());
    assert!(style(&app, "text:/^Second with mark$/ >> text:mark").background.is_some());
    assert!(!style(&app, "text:Second with").decoration.line_through);
    // `.o-code`: mono at 8/9 of the text, red
    let code = style(&app, "text:let x = 1;");
    assert!(code.font_family.as_ref().is_some_and(|f| format!("{f:?}").contains("Fira Mono")), "{code:?}");
    assert!((code.font_size.0 - 16.0).abs() < 0.01);
    assert_eq!(code.color, hex(0xc7254e));
    // an escape shows its character; LaTeX shows its source in mono (no math typesetting)
    assert!(app.screen().shows("Code let x = 1;, an escaped *star* and e^{i\\pi}+1=0"));
    assert!(style(&app, "text:e^{i").font_family.as_ref().is_some_and(|f| format!("{f:?}").contains("Fira Mono")));
}

#[test]
fn links_tags_and_images() {
    let app = start(Os::Ios);
    assert!(app.screen().shows("A link, https://example.org/docs and #tag @me"));
    // `.o-link`: the text's color, underlined; `.o-tag`: gray, underlined
    let link = style(&app, "text:link");
    assert!(link.decoration.underline);
    assert_eq!(link.color, hex(0x222222));
    assert!(style(&app, "text:https://example.org/docs").decoration.underline);
    for tag in ["#tag", "@me"] {
        let s = style(&app, &format!("text:/^A link/ >> text:{tag}"));
        assert!(s.decoration.underline);
        assert_eq!(s.color, hex(0x7a7a7a));
    }
    // `![alt](url)`: the alt text in the accent color (the web's default, images not inline)
    assert_eq!(style(&app, "text:a picture").color, hex(0x2f8cf0));
}

#[test]
fn date_chips() {
    let app = start(Os::Ios);
    // a chip is an inline view in its row's text, showing the label as the web does
    assert!(app.screen().shows("Due \u{fffc} and \u{fffc}"));
    assert!(app.screen().shows("Fri, January 5th 2024"));
    assert!(app.screen().shows("Sun, March 10th 2024, 3:30 pm - 5 pm"));
    assert!(app.screen().shows("Mon, January 8th 2024, every week on Mon, Wed, Fri"));
    // the label is 8/9 of the text, inside the row's line
    let label = text_node(&app, "Sun, March 10th 2024, 3:30 pm - 5 pm");
    let row = text_node(&app, "Due \u{fffc} and \u{fffc}");
    assert!(label.bounds.origin.y.0 >= row.bounds.origin.y.0, "{label:?}");
    assert!(
        label.bounds.origin.y.0 + label.bounds.size.height.0 <= row.bounds.origin.y.0 + row.bounds.size.height.0 + 1.0
    );
    assert!((style(&app, "text:Sun, March 10th").font_size.0 - 16.0).abs() < 0.01);
}

#[test]
fn code_blocks_stand_under_their_line() {
    let app = start(Os::Ios);
    let line = text_node(&app, "Code block:");
    let block = text_node(&app, "const a = 1;\nconsole.log(a);");
    // the block starts on the line after, in mono at .85em
    assert!(block.bounds.origin.y.0 >= line.bounds.origin.y.0 + line.bounds.size.height.0);
    let s = style(&app, "text:/^const a/ >> text:const");
    assert!(s.font_family.as_ref().is_some_and(|f| format!("{f:?}").contains("Fira Mono")));
    assert!((s.font_size.0 - 18.0 * 0.85).abs() < 0.01);
}

#[test]
fn notes_show_under_their_item() {
    let app = start(Os::Ios);
    let item = text_node(&app, "Item with a note");
    let note = text_node(&app, "A note under the item, with bold and a #tag.");
    // `.o-note`: under the content, at the same x, 15px gray on 18px lines, formatted
    assert!(note.bounds.origin.y.0 >= item.bounds.origin.y.0 + item.bounds.size.height.0 - 0.5);
    assert_eq!(note.bounds.origin.x, item.bounds.origin.x);
    let s = style(&app, "text:/^A note under/ >> text:A note");
    assert!((s.font_size.0 - 18.0 * 0.8333).abs() < 0.01);
    assert_eq!(s.color, hex(0x4e4e4e));
    assert_eq!(style(&app, "text:/^A note under/ >> text:bold").font_weight, 700);
}

#[test]
fn numbered_children_show_numbers_for_bullets() {
    let app = start(Os::Ios);
    for (n, child) in [("1.", "First"), ("2.", "Second with mark"), ("3.", "Third")] {
        let num = text_node(&app, n);
        let text = text_node(&app, child);
        // right-aligned in the bullet column, 6px before the 6px gap to the text
        let right = num.bounds.origin.x.0 + num.bounds.size.width.0;
        assert!((text.bounds.origin.x.0 - 12.0 - right).abs() < 1.0, "{n}: {right} vs {}", text.bounds.origin.x.0);
        assert!(num.bounds.origin.y.0 >= text.bounds.origin.y.0 - 0.5);
    }
    // the parent itself keeps its dot
    assert!(!app.screen().shows("4."));
}

#[test]
fn checked_rows_are_gray_and_struck() {
    let app = start(Os::Ios);
    let s = style(&app, "text:Checked with a date");
    assert!(s.decoration.line_through);
    assert_eq!(s.color, hex(0xa7a7a7));
}

#[test]
fn checking_a_recurring_item_moves_its_date_on() {
    let mut app = start(Os::MacOs);
    // to the row from the one below (a press in the middle of the row would land on its date chip)
    app.press("text:Item with a note").unwrap();
    app.wait_for("id:row-input").unwrap();
    app.press_key("up").unwrap();
    let source = app.screen().input_text("id:row-input").unwrap().unwrap_or_default().to_owned();
    assert_eq!(source, "Every week !(2024-01-08 | 1w135) and ![a picture](https://example.com/a.png)");
    app.press_key("mod+enter").unwrap();
    // the item moves to the next of Mon, Wed, Fri, and stays edited
    let source = app.screen().input_text("id:row-input").unwrap().unwrap_or_default().to_owned();
    assert_eq!(source, "Every week !(2024-01-10 | 1w135) and ![a picture](https://example.com/a.png)");
    // a checked copy keeps the old date above it
    app.press_key("down").unwrap();
    app.wait_for("text:Wed, January 10th 2024, every week on Mon, Wed, Fri").unwrap();
    assert!(app.screen().shows("Mon, January 8th 2024, every week on Mon, Wed, Fri"));
    let copy = style(&app, "text:/^Every week/ >> nth=0 >> text:Every week");
    assert!(copy.decoration.line_through);
    let item = style(&app, "text:/^Every week/ >> nth=1 >> text:Every week");
    assert!(!item.decoration.line_through);
}

