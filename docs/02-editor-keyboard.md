# 02 — Editor behavior & keyboard (behavioral spec)

Source: static reading of the Dynalist desktop web bundle (`www/assets/js/main.min.js`, `www/index.html`).
Nothing was executed. This is a clean-room description of *behavior*; identifiers are cited only as
lookup hints. "Mod" = Cmd on macOS, Ctrl elsewhere (the bundle's `ctrl` modifier renders as ⌘ on Mac).
"Control" = the literal Control key even on Mac.

---

## 0. Selection model (drives every key handler)

There are exactly four selection states. Every key handler branches on them.

| State | Meaning | Visual |
|---|---|---|
| **None** | Nothing focused | — |
| **Content caret/range** | Caret or text range inside an item's main text (raw markdown shown) | item in edit mode |
| **Note caret/range** | Caret or text range inside an item's note (raw) | note in edit mode |
| **Multi-item** | One or more whole items selected (block selection). Stores `from` (anchor) and `to` (focus) items plus the ordered list of selected items | items highlighted, no caret |

Multi-item selection is always a set of **siblings under one parent** (computed by "select between": find the
deepest common ancestor of anchor and focus, then take the contiguous, visible children of that ancestor that
contain them). If anchor is an ancestor of focus, the selection is just the anchor. The zoomed-in root can never be
multi-selected. Ctrl/Cmd+click is the only way to get a non-contiguous set (it's re-sorted into document order).

The current **zoom root** ("current root") is rendered as a title above the list; it is editable like an item but
several rules special-case it (see below).

---

## 1. Default keyboard shortcut table

All named shortcuts are user-customizable (stored as an override map in the `shortcut_override` preference)
except **Select all (Mod+A)**. Many commands ship **unbound** (marked "—") and are only reachable via the
settings UI / menus. The shortcut help panel (Mod+/) groups them into the categories below, in this order.

### 1.1 Basics
| Default key | Command id | Action |
|---|---|---|
| Tab | doc-indent | Indent |
| Shift+Tab | doc-unindent | Unindent |
| Mod+. | doc-collapse | Expand/collapse current item(s) |
| Mod+] | doc-zoom-in | Zoom into current item |
| Mod+[ | doc-zoom-out | Zoom out one level |
| Mod+Shift+Backspace | doc-delete-left | Delete item(s) (focus goes to previous item) |

### 1.2 General
| Default key | Command id | Action |
|---|---|---|
| Mod+/ and Mod+Shift+/ | app-toggle-shortcut-help | Toggle shortcut help (allowed in read-only) |
| Mod+, | app-open-settings | Open settings |
| Mod+Shift+F | app-toggle-file-pane | Toggle file pane |
| Mod+Shift+B | app-toggle-bookmark-pane | Toggle bookmark pane |
| — | app-toggle-tag-pane | Toggle tag pane |
| — | app-toggle-show-checked | Toggle checked items (global) |
| — | doc-toggle-show-checked | Toggle checked items (this doc) |
| — | app-toggle-show-notes | Toggle notes (global) |
| — | doc-toggle-show-notes | Toggle notes (this doc) |
| Mod+S | app-save | Save / force sync |
| — | doc-toggle-spellcheck | Toggle spell checker |
| Mod+Shift+I | app-inbox-capture | Capture to inbox (Pro) |
| — | doc-switch-to-mind-map | Switch to mind map (Pro) |
| — | app-toggle-bookmark-current-state | Bookmark current view |

### 1.3 Working with lists
| Default key | Command id | Action |
|---|---|---|
| Mod+Shift+Delete | doc-delete-right | Delete item(s) (focus goes to next item) |
| Shift+Enter | doc-goto-notes | Move between item text and its note |
| Mod+Enter | doc-toggle-checked | Toggle completed |
| — | doc-uncheck-children | Uncheck item and all descendants |
| Mod+Shift+. | doc-collapse-all | Expand/collapse item(s) **and all descendants** |
| — | doc-toggle-siblings | Expand/collapse all siblings |
| Mod+Shift+C | doc-toggle-checklist | Toggle checkbox on item(s) |
| — | doc-toggle-children-checkbox | Toggle checkbox on children |
| Mod+Shift+X | doc-toggle-ordered-list | Toggle numbered list (applies to the item's children) |
| — | doc-duplicate | Duplicate item(s) (inserted right after the last selected) |
| — | doc-insert-template | Insert template |
| Shift+Up | doc-select-up | Extend block selection upward |
| Shift+Down | doc-select-down | Extend block selection downward |
| Mod+Shift+A | doc-select-all-force | Select every item under current root (allowed read-only) |
| Mod+Z | doc-undo | Undo |
| Mod+Y and Mod+Shift+Z | doc-redo | Redo |
| — | doc-copy-link | Copy link to current item |
| — | doc-delete-checked | Delete completed items under selection |
| — | doc-show-node-references | Show backlinks to item |
| — | doc-sort-title-asc / -desc | Sort children A–Z / Z–A |
| — | doc-sort-date-desc / -asc | Sort children by date |
| — | doc-expand-to-level-1..4 | Expand selection to N levels deep (deeper levels collapsed) |

### 1.4 Navigating & moving
| Default key | Command id | Action |
|---|---|---|
| Mod+Shift+M | doc-move-node | "Move item to…" picker |
| Mod+Up | doc-swap-up | Move item(s) up |
| Mod+Down | doc-swap-down | Move item(s) down |
| Mod+Shift+[ | doc-zoom-prev-sibling | Zoom into previous sibling of current root |
| Mod+Shift+] | doc-zoom-next-sibling | Zoom into next sibling of current root |
| — | app-goto-inbox | Go to inbox location |
| — | doc-goto-parent | Caret to end of parent (zooms out if parent is the zoom root's parent) |
| — | doc-goto-prev-sibling / next-sibling | Caret to end of previous/next sibling |
| Mod+Home | doc-goto-first | Caret to start of first visible item |
| Mod+End | doc-goto-last | Caret to end of last visible item |
| — | doc-goto-item-start / -end | Caret to start/end of current text (item or note) |

### 1.5 Search
| Default key | Command id | Action |
|---|---|---|
| Mod+F | doc-search | Toggle in-document search bar |
| Mod+Enter (in search bar) | doc-search-everywhere | Search all documents |
| Shift+Enter (in search bar) | doc-flat-search | Flat (list) search |
| Mod+O | app-go-anywhere | File finder |
| Mod+Shift+O | app-goto-item | Item finder (Pro) |
| — | doc-open-search-and-replace | Search & replace |

### 1.6 Formatting
| Default key | Command id | Action |
|---|---|---|
| Mod+B | doc-bold | Toggle `**` |
| Mod+I | doc-italics | Toggle `__` |
| Mod+` | doc-insert-code | Toggle `` ` `` |
| — | doc-format-code-block | Toggle ```` ``` ```` |
| Mod+K | doc-format-link | Wrap as link |
| — | doc-strike-through | Toggle `~~` |
| — | doc-highlight | Toggle `==` |
| — | doc-insert-latex | Toggle `$$` |
| Mod+Shift+Enter | doc-insert-new-line | Insert a literal newline inside the item text |
| Mod+Shift+H | doc-cycle-heading | Cycle heading none→H1→H2→H3→none |
| — | doc-heading-1/2/3, doc-clear-heading | Set heading |
| Mod+Shift+L | doc-cycle-color-label | Cycle color label 0→1…6→0 |
| — | doc-color-label-1..6, doc-clear-color-label | Set color label (1 red, 2 orange, 3 yellow, 4 green, 5 blue, 6 purple) |
| — | doc-insert-current-date | Insert `!(YYYY-MM-DD)` for today |
| — | doc-insert-current-time | Insert `!(YYYY-MM-DD HH:mm)` for now |

### 1.7 Hard-wired (non-customizable) keys
| Key | Context | Action |
|---|---|---|
| Enter / Backspace / Delete / Arrows | editor | see §2 |
| Mod+A | editor | Progressive select-all (see §4) |
| Esc | editor | see §2.10 |
| Mod+U | editor | Swallowed (no underline) |
| Control+P / Control+N | macOS only | Same as Up / Down |
| Tab | in search bar | Jump caret to first item |
| Enter / Esc | search bar | run / exit search |

### 1.8 Alternate preset & layout overrides
- **"Workflowy shortcuts" preference** swaps in: Zoom in = Alt+Right, Zoom out = Alt+Left,
  Collapse = Control+Up, Move up/down = Shift+Alt+Up / Shift+Alt+Down.
- **German keyboard layout**: Zoom in/out become Alt+Right / Alt+Left (brackets are awkward there).
- Resolution order: user override → keyboard-layout override → Workflowy preset (if on) → default.

### 1.9 In-text "trigger" sequences (from the help panel)
| Typed | Opens |
|---|---|
| `#` or `@` (at start or after whitespace) | Tag autocomplete (top 8 matches; picking inserts the tag + a space) |
| `!` (at start or after whitespace) | Date picker (date, optional time, optional range, optional recurrence) |
| `[[` (at start or after whitespace) | Item-link autocomplete across all docs; picking inserts `[Item title](absolute item URL)` with the title text selected |

---

## 2. Edit-mode key behavior (exact rules)

### 2.1 Enter (in item text)
Ignored while an autocomplete/date popup is open (popup consumes it).

1. **Empty-item outdent rule**: if the item text is empty AND the item is not the doc root AND its parent is not the
   current zoom root AND it has no *visible* next sibling → **unindent** instead of creating an item. (Pressing Enter
   repeatedly on a trailing blank bullet walks it outward, like most outliners.)
2. Otherwise **split**:
   - **Caret at end of text** (collapsed selection at/after last char): create a new empty item.
     - Placed as the **first child** if the item is the zoom root, or the item has visible (expanded) children,
       or has no parent.
     - Otherwise placed as the **next sibling**.
     - The new item copies the **checkbox** flag (not the checked state). Caret goes to new item.
   - **Caret in middle / range selected**: text before the selection start → a **new item inserted *above***
     (same parent, same index); text after the selection end stays in the **original item** (which keeps its id,
     note, children, collapse state, heading, etc.). Selected text is dropped. New item copies checkbox flag.
     Caret goes to start of the original (lower) item — except when the caret was at position 0, in which case
     the caret is placed in the new empty item above.
   - **Zoom root title, caret mid-text**: head stays in the title, tail becomes the title's new first child; caret
     goes to that child.
3. In **note**: Enter inserts a literal newline (notes are multi-line).
4. In **multi-item selection**: deletes the selected items and puts a single fresh empty item where the first one
   was; caret there.

### 2.2 Shift+Enter (doc-goto-notes)
- From item text → open/focus the note. Special case: if a text **range** is selected and the note is empty, the
  selected text is **moved out of the item into the note**.
- From note → caret to end of item text.
- Mod+Shift+Enter inserts a newline *inside the item text* instead.

### 2.3 Backspace
- **Multi-item**: delete the selected items (focus → end of previous visible item, else start of next).
- **Caret at start of item text** → "combine with before":
  1. If the item has **no children** and there's a visible previous item (could be the deepest last descendant of
     the previous sibling) → merge into that previous item.
  2. Else if it has a previous sibling → merge with it (see merge rule).
  3. Else if its parent isn't the zoom root → merge into parent.
- **Merge rule**: resulting text = previous text + current text; caret placed at the join point.
  Which item *survives*: the one with children. If neither has children, the previous item survives. **If both have
  children, nothing happens** (merge refused). Metadata (checkbox, heading, color, …): if only one of the two has
  non-empty text, that one's metadata is kept; otherwise metadata is merged.
- **Caret at start of an empty note**: caret jumps to end of the item text (note collapses when empty & blurred).
- Otherwise native character delete.

### 2.4 Delete (forward)
- Multi-item: delete selected items (focus → start of next item, else end of previous).
- Caret at end of item text: merge the next visible item into this one (same merge rule; caret stays).

### 2.5 Tab / Shift+Tab
- **Tab (item text or multi-select)**: move the item(s) to become the **last children of the previous visible
  sibling**; that sibling is auto-expanded. No previous sibling → no-op. In a **note**, Tab inserts a literal tab
  character.
- **Shift+Tab**: move item(s) to be siblings immediately **after their parent**. If that would take them outside the
  current zoom, the view zooms out to the grandparent first. Preference **"unindent in place"** (off by default): the
  former following siblings are re-parented as children of the last unindented item (Workflowy-style), so visual
  order is preserved. Caret/selection preserved.

### 2.6 Up / Down
- **Item text**: native movement within wrapped/multi-line text. Only when the caret is on the **first visual line**
  (Up) or **last visual line** (Down) does it jump to the previous/next *visible* item, keeping the caret's
  **horizontal pixel x-position** (caret at offset 0 stays at 0). Up from the zoom-root title does nothing; Down
  from the title only enters its children.
- **Note**: Up from first line → item text of same item; Down from last line → next visible item (x-position adjusted
  ±30px for indent changes).
- **Multi-item**: Up → caret to start of item before the first selected; Down → caret to start of the item after the
  last selected (skipping its descendants).
- Read-only docs: arrows move a single-item block selection instead.

### 2.7 Left / Right at boundaries
- Left at start of item text → end of previous visible item. Left at start of note → end of own item text.
- Right at end of item text → start of next visible item.
- Multi-item: Left → caret at start of first selected; Right → caret at end of last visible descendant of last
  selected.

### 2.8 Mod+Up / Mod+Down (move)
- Up: if there's a visible previous sibling → move before it. Else (first child) → if parent's previous sibling exists
  and is **expanded**, become its **last child**; otherwise become the sibling **just before the parent**. Never
  escapes the zoom root.
- Down: symmetric (after next sibling; or first child of parent's next sibling if expanded; else just after parent).
- Works on the whole multi-selection as a block. View scrolls to keep selection visible.

### 2.9 Collapse / expand
- Mod+. toggles the current item (or every selected item: if all are already collapsed → expand all, else collapse
  all). If the caret was inside a descendant that would be hidden, the caret moves to the collapsed item.
- Mod+Shift+. does the same but recursively for all descendants. On the zoom-root title it applies to the root's
  children.
- Clicking the expand arrow toggles; clicking a bullet toggles collapse (default) or zooms (preference "use bullet to
  zoom"; always zooms on mobile). **Double-click on bullet** (within 200 ms) expands/collapses all children.

### 2.10 Esc
- Multi-item → leave block selection (back to DOM caret if any).
- If a tag/link/date popup is open → only closes the popup.
- Editing → remember the view state (caret position), then blur (no selection).
- Not editing + searching → exit search. Not editing + not searching → restore the remembered caret (Esc toggles
  focus in/out of the editor).
- App-level Esc also closes finders, modals and context menus.

### 2.11 Zoom & breadcrumbs
- Mod+] zooms into the item holding the caret (URL changes to `#z=<itemId>`; zoom level remembered per document).
  Mod+[ zooms to the parent of the current root; after zooming out the caret lands at the end of the item you were
  zoomed into.
- Mod+Shift+[ / ] zooms to previous/next sibling of the current root.
- When zoomed, a breadcrumb bar shows every ancestor (rendered formatted text, separated by " > "), each a link that
  zooms there (Mod-click opens normally/new tab). Hidden at document root.
- Zoom root is displayed as an editable title; deleting/moving an item that contains the zoom root zooms out first.

### 2.12 Toggle checked (Mod+Enter)
Toggles completion for the item(s): if all are checked → uncheck all, else check all. Afterwards the caret moves to
the next visible non-descendant item. Recurring dates (`!(… | 1w)`) on Pro: checking advances the date instead of
completing, and (default preference "new_instance") inserts a completed copy above.

---

## 3. Undo / redo grouping
- Every edit is a **transaction** of primitive actions (attach/move, detach, meta-edit) plus the selection before and
  after; undo reverts actions in reverse and restores the "before" selection, redo re-applies and restores "after".
- **Typing coalescing**: a transaction that changes a single field of a single item by exactly ±1 character carries
  merge info (direction = insert vs delete, field = text vs note). It is merged into the previous undo entry if: the
  previous entry is < 1000 ms old, has the same direction & field, both are single meta-edits on the same item. The
  merged entry's timestamp refreshes, so continuous typing becomes one undo step; a pause > 1 s, switching between
  typing and deleting, or moving to another item starts a new step. Paste / multi-char changes are always their own
  step.
- Any new edit clears the redo stack. Undo entries expire after **48 hours**.
- Structural ops (split, merge, indent, move, delete, paste) are each one undo step.

---

## 4. Multi-item (block) selection

How to enter it:
| Gesture | Result |
|---|---|
| Shift+Up/Down while editing | Only when the caret is on the first line (Up) / last line (Down) or at text start/end; selects the current item. Otherwise native text-extend. |
| Shift+Up/Down in block mode | Moves the focus end to previous/next visible item *at the same level* (skips descendants), selection recomputed with "select between" |
| Mod+A while editing | 1st press: native select all text of the item. 2nd press (whole text already selected): select the item as a block. Further presses: if not all siblings selected → select all siblings; if all siblings selected → select the parent; mixed levels → normalize to common-ancestor siblings |
| Mod+Shift+A | Select all top-level items under current root |
| Mouse drag from one item's text to another | Drag-to-select (block selection between start and hovered item; auto-scrolls near edges) |
| Shift+click | Block-select between current item (or anchor) and clicked item |
| Mod+click | Toggle clicked item in/out of the block selection (non-contiguous allowed) |
| Shift+click below the last item | Select from current item to the last item |
| Right-click bullet / menu icon | Context menu; selects that item (or keeps the multi-selection if it was inside) |

What works on a block selection: Tab/Shift+Tab, Mod+Up/Down, Backspace/Delete/Mod+Shift+Backspace (delete),
Enter (replace with one blank item), Mod+Enter (check), Mod+./Mod+Shift+. (collapse), all formatting shortcuts
(see §6.3), heading/color/checkbox/numbered toggles, duplicate, move-to, copy/cut/paste, drag. Arrows/Esc exit it.

---

## 5. Clipboard

### 5.1 Copy / cut
- **Text range** inside an item: plain text of the selection; also an HTML rendering and a single-outline OPML
  fragment. Cut removes the text.
- **Block selection**: writes three flavors:
  - `text/plain`: one line per visible item (collapsed subtrees omitted, hidden-by-search items skipped), notes as
    following lines, numbered-list children prefixed `N. `. Note: the bundle exports this **without indentation**
    (indent option off for clipboard); a clone may prefer 4-space indentation per level (that is what the
    *export* dialog offers).
  - `text/html`: nested `<ul>/<ol><li>`, headings as `<hN>`, checked items wrapped in `<del>`, notes after `<br>`.
  - `text/opml`: full-fidelity OPML (text, `_note`, checkbox, complete, heading, colorLabel, listStyle, collapsed).
  - Cut = copy + delete items.

### 5.2 Paste (priority order)
Pastes are debounced (a second paste within 30 ms is ignored). If the caret is in a **note** or in the **zoom root
title**, everything is inserted as literal text (CRLF normalized). Otherwise:
1. Clipboard has `text/opml`, or plain text that *is* an OPML document → paste as structured items (lossless).
2. Clipboard has `text/html` → convert HTML to indented markdown text, then paste as text (below). Conversion:
   lists → one tab per nesting level, `<li>`/block elements → new lines, `<b>/<strong>` → `**`, `<i>/<em>` → `__`,
   `<a>` → `[text](href)` (bare URL if text == href), `<img>` → `![alt](src)`, `<br>` → newline, scripts/styles
   dropped, whitespace collapsed (except `<pre>`). MS Word list paragraphs use their `mso-list levelN` (or heading
   level) as indent depth. A lone pasted `<img>` with an image file → upload instead (Pro).
3. Plain text → **indent detection**:
   - Tabs are converted to 4 spaces; CR/CRLF → LF.
   - The **minimum leading-space count** across non-blank lines is stripped from every line (so pasted blocks that
     are uniformly indented don't nest).
   - Line 1 is spliced into the current item at the caret (text-before-caret + line1). The text after the caret is
     appended to the **last** pasted line.
   - Each subsequent line becomes a new item. Using a stack of (parent, indent): if a line's leading-space count is
     greater than the stack top's → it becomes the first child of the previous line's item; if smaller → pop until
     ≤, then insert as next sibling at that level. Blank lines are skipped (except the last line).
   - Caret ends at the end of the pasted text in the last item (before the re-appended tail).
   - When pasting structured items (OPML), the first item merges into the current item likewise, its children are
     added under the current item, subsequent roots become following siblings, collapsed flags are restored.
4. Files only (no text) → upload as attachment (Pro); inserts ` ![name](placeholder) ` / ` [name](placeholder) `
   then swaps in the final URL.
- Pasting over a block selection first replaces the selection with one blank item, then pastes into it.

---

## 6. Inline formatting syntax

### 6.1 Rendered vs raw
- Each item (and note) has two layers: a **rendered** view (formatted HTML) and a **raw editable** field containing the
  exact markdown source. The rendered view is shown whenever the field is not focused; on focus the raw markdown
  is shown and edited. Formatting is **never** live-WYSIWYG while editing.
- Clicking rendered text enters edit mode and places the caret at the **source offset corresponding to the clicked
  character** (hit-testing maps rendered position → token start + offset). LaTeX is "select-whole": clicking a
  formula selects its entire `$$…$$` source. Clicking a link, tag, date, image link opens/acts instead of editing
  (those tokens are "disabled" for caret placement).
- Click tag → adds `#tag` to the doc search filter (click again removes it; Alt+click deletes the tag from the text).
  Click date → opens date picker. Click internal link → navigates within app.
- Notes can be shown fully, as a one-line teaser, or hidden (preference); an empty note is hidden unless being edited.

### 6.2 Grammar (tokenizer-based, left-to-right, with a context stack)
Tokens split on: `#` `@` `$$` `__` `~~` `==` `**` `![` `[` `]` `!(` `(` `)` newline, space, `,` `;` `.` `?` `!`
`` ` `` `/` `'` `\` `:` and any `scheme://`.

| Syntax | Renders as (CSS class) | Rules |
|---|---|---|
| `**text**` | bold (`node-bold`) | Same delimiter opens/closes; nesting other formats inside allowed |
| `__text__` | italic (`node-italics`) | as above (note: underscores, not single `*`) |
| `~~text~~` | strikethrough (`node-strike-through`) | as above |
| `==text==` | highlight (`node-highlight`) | as above |
| `` `code` `` | inline code (`node-inline-code`) | contents literal; only `` ` `` closes; backslash escapes inside |
| ```` ```lang\n…``` ```` | code block (`node-multiline-code`) with syntax highlighting + Copy button | if first line is only lowercase letters it's the language; a newline right after the closing fence is swallowed |
| `$$latex$$` | KaTeX inline (`node-latex`) | contents literal; on render error shows source |
| `[text](url)` | link (`node-link`), opens in new tab | `[` must be followed immediately by `](`; balanced `[]` inside text and `()` inside URL; empty text → "Link" |
| `![alt](url)` | image link (`node-inline-image`), hover shows preview; with "display images inline" pref → actual `<img>` | empty alt → "Image" |
| bare `scheme://…` | auto-link | ends at whitespace/newline or an unbalanced `)` |
| `#tag`, `@tag` | tag pill (`node-tag`) | must be at start or after whitespace, and next char must not be space/EOL; ends at space, newline, `**`, `__`, `` ` ``, `'`, `,`, `?`, `!`, `;`, `/`, or an unbalanced `)`; `:` and `.` end it only if followed by whitespace/EOL; balanced `()` allowed inside; **all-digit tags (`#123`) are not tags** |
| `!(YYYY-MM-DD)` / `!(YYYY-MM-DD HH:mm)` | date pill (`node-time`; `is-overdue` if past, `is-recurring`) | optional range `!(start - end)`; optional recurrence `| Nd/Nw/Nm/Ny`, weekly may list ISO weekdays (`1w135` = Mon/Wed/Fri), prefix `~` = repeat from completion date; optional timezone; unparsable → plain text. Displayed relative ("Today", "Tomorrow", "In 3 days", "Yesterday"…), else "ddd, MMMM Do YYYY" + time (12h/24h pref or custom format) |
| `\x` | literal x | escapable chars: `\ ~ $ = \` * _ { } [ ] ( ) # + - . !` (the backslash is hidden when rendered) |
| Unclosed delimiter | literal text | the opener is emitted as plain text and parsing resumes after it |

URL normalization for links/images: keep if it has a `scheme:`; `C:\…` or `/abs/path` → `file://…`; otherwise
prefix `http://`.

### 6.3 Formatting commands (Mod+B, Mod+I, Mod+`, strike, highlight, LaTeX, code block)
On a text selection, with delimiter D:
1. If the selection is immediately surrounded by D (D just outside both ends) → remove those D's.
2. Else if the selected text itself starts and ends with D → strip them.
3. Else wrap: leading/trailing single space is excluded from the wrap; empty selection inserts `DD` with the caret
   between.
The resulting selection covers the same inner text. On a **block selection**, toggles D around the *entire text* of
every selected visible item (and visible descendants of expanded items): if any item is not already wrapped, wrap all
unwrapped ones; otherwise unwrap all.

Mod+K: wraps selection as `[selection]()` and places the caret inside `()`; with no selection inserts `[]()` with caret
inside `[]`.

### 6.4 Item links / references
- `[[` autocomplete inserts a standard markdown link whose URL is the absolute item URL
  (`…/d/<docId>#z=<itemId>`). Internal links get class `node-link-internal` and navigate in-app.
- If such a link's text is empty or equals the URL, the **target item's current title is displayed live** (updates
  when the target is renamed); links to items show a zoom affordance.
- Items show a backlink counter badge when referenced (preference); "Show all references" lists backlinks.

### 6.5 Item-level formatting (metadata, not markdown)
Heading 0–3, color label 0–6, checkbox flag, checked flag, numbered-list flag (applies to children's bullets), note.
Stored separately from the text and preserved by split (on the lower/original item) and merge (see §2.3).

---

## 7. Drag and drop (mouse)
- Drag starts by pressing on a **bullet** and moving > 3 px (touch: long-press 500 ms, cancelled by > 5 px movement;
  phone vibrates). If the bullet belongs to an item in the current block selection, the whole selection is dragged.
- A floating indicator follows the pointer: "Move N items" (or "Copy N items" while holding Mod).
- Drop target is computed by hit-testing the visible tree under the pointer:
  - Pointer in upper half of an item row → insert **before** it.
  - Lower half → insert **after** it; if that item is expanded with children → insert as its **first child**.
  - Pointer horizontally far enough right (beyond row-left + 100 px + 20 % of the remaining width + 30 px) over a
    childless/collapsed item → insert **inside** it, at the bottom (default) or top (preference "move item send to
    position").
  - Positions that would be no-ops (adjacent to itself) or inside the dragged subtree are ignored.
  - A divider line shows the target (top/bottom/inner).
- Releasing with Mod held **copies** (clones) instead of moving. Esc-free; release outside a valid target cancels.
- Auto-scroll when the pointer is within 50 px of the top/bottom (speed ramps 10→30 px per 50 ms).
- Dragging onto a document in the left file pane moves items to that document; dragging a document from the file
  pane into the outline offers to convert that document into an item (with confirmation).
- Dropping OS files onto the document uploads them (one file at a time, Pro).

---

## 8. Other mouse behaviors
- Clicking empty space in an item row (not text) → caret at start of that item.
- Clicking below the last item → caret at end of the last item (creates/ensures an editable item in empty docs).
- Bullet hover shows created/last-edited timestamps as a tooltip; bullet `href` is the item's zoom URL.
- Hovering an item reveals menu / zoom / expand icons; context menu offers collapse/expand(all), zoom, indent/unindent,
  move, template, add note, checkbox, numbered list, heading, color, link, references, export, sort, delete.

---

## 9. Relevant preferences (defaults)
| Preference | Default | Effect |
|---|---|---|
| use_bullet_to_zoom | off | bullet click zooms instead of collapsing |
| unindent_in_place | off | following siblings become children on Shift+Tab |
| move_item_sent_to_position | bottom | where "drop inside"/move-to puts items |
| auto_focus_first_item | off | focus first item after zoom/open |
| workflowy_shortcuts | off | alternate shortcut preset (§1.8) |
| keyboard_layout | english | layout-specific overrides |
| show_notes | show (show/teaser/hide) | note display |
| display_images_inline | off | render `![]()` as images |
| time_format | 12h | date rendering |
| recurrence_check_behavior | new_instance | recurring completion |
| global_tag_autocomplete | off | tag suggestions from all docs vs this doc |
| spellcheck | off | |
