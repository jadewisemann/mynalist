# 03 — Features & UI Layout (behavioral spec)

Clean-room description derived from static inspection of the Dynalist desktop web bundle (DOM template, stylesheets, and minified logic). Everything here is written in our own words. Pixel values are approximate guidance, not copied rules. "Pro" marks features the original gates behind a paid tier. A clone can ignore the gating.

---

## 1. Overall layout

```
+---------------------------------------------------------------------------+
| Top bar (~46px): [<-][->] history     ...  sync status | lock | bell | gear | ☰ |
+------+--------------------+-+---------------------------------------------+
| tab  | Pane header (26px) |s| Breadcrumb (Parent > Child > ...)           |
| strip|  title + toolbar   |p| [search box / view-options icons, top-right]|
| (20px|--------------------|l|                                             |
| vert.| pane content       |i|  DOCUMENT TITLE (zoomed root, ~28px)        |
| tabs)| (tree of folders/  |t|  note of root                               |
|      |  docs / bookmarks /|t|  • item                                     |
|      |  tags)             |e|  │ • child                                  |
|      |                    |r|  • item                                     |
+------+--------------------+-+---------------------------------------------+
|                                       status bar (word counts, optional)  |
+---------------------------------------------------------------------------+
```

### 1.1 Top bar
- Left: Back/Forward navigation buttons (desktop app). They walk the history of document, zoom, and search states (tooltips mention Alt+Left/Right).
- Right, in order:
  - Sync status label. Only one state shows at a time: spinner (syncing), "Saved", "Synced", "Sync now" (clickable), "Can't sync", "Can't save", "Sync disabled". Clicking it opens a **Sync status** modal that lists each document with its sync state and errors.
  - Lock/read-mode indicator (hidden unless read mode is on).
  - "What's new" bell. It opens a notifications popover with an empty state ("No more news to show").
  - Settings gear.
  - Main menu (hamburger).
- Main menu contents: user name and email (opens account settings), Upgrade, Tell a friend, Download the apps / Settings…, Upload manager / Help, Shortcuts & Formatting, Tutorial, Blog, About / Import from WorkFlowy…, Download backup / Logout.
- A banner across the top appears when a new app version is available, with a refresh link.

### 1.2 Left sidebar
- Desktop: a narrow vertical **tab strip** (~20px) with rotated labels for **My files**, **Bookmarks**, and **Tags** (Tags is Pro). Clicking a tab switches the active pane. The panes area sits to the right of the strip.
- A draggable **splitter** (~7px, with a 1px line) resizes the sidebar. When the sidebar gets narrow, pane header text is hidden.
- The sidebar can be toggled. On mobile it slides in (280px) over a dimmed overlay.
- Each pane has a header (~26px) with a title on the left and small icon buttons on the right. Pane content scrolls.
- Pane rows have a left indent of ~26px for the toggle and icon. Hovered or highlighted rows get a light-blue background.

**File pane ("My files")**
- Header toolbar:
  - "Create new file…" (new document / new folder)
  - File finder (Ctrl+O)
  - Inbox capture (Pro, Ctrl+Shift+I)
  - Collapse pane
- Shows a tree of folders and documents.
  - Folders have an expand/collapse toggler and a folder icon. Empty folders are marked as such.
  - Documents have a doc icon. Archived documents are dimmed to about 65% opacity and show a distinct archive icon.
- Each row shows a "…" menu trigger on hover. Right-click opens the same menu.
- Double-clicking a row renames it inline. The name is edited in place; an empty name falls back to "Untitled".
- Drag and drop rearranges the tree (reorder, or move into a folder).
- Dropping outline items from the editor onto the file pane:
  - Onto a folder or position in the tree: **converts those items into a new document**.
  - Onto an existing document: **moves the items into that document**.
- Dropping a document onto an item in the editor converts the document into an item. A confirmation says the document will be deleted and its content copied under the destination item.
- Right-click on empty pane space shows: New document, New folder, Import from OPML (file picker; only `.opml` files are accepted).
- Folder context menu: a file-details header (icon, name, path), then New document, New folder / Manage sharing… / Rename, Delete.
- Document context menu: a file-details header, then:
  - Manage sharing…
  - Export…
  - Add to bookmarks, Make a copy, Rename, Delete, Archive/Unarchive
  - Version history (Pro), Mind map view (Pro)
  - Google Calendar sync "Turn on/off" for this document
  - "Clean up newlines" (only shown if the advanced setting is enabled)
- Delete asks for confirmation ("Are you sure you want to delete the <folder|document> <name>?").
- Archived documents are not preloaded or synced in the background. Archiving keeps them in the tree but dims them.

**Bookmarks pane**
- Header shows a count against the limit, e.g. "(0/5)". Free accounts are limited to 5 bookmarks; reaching the limit opens an upsell modal.
- Toolbar: "Bookmark current state" and Collapse.
- A bookmark captures a *view state*: document, zoomed node, focused node, search query, flat-search flag, and sort order. So you can bookmark a zoomed sub-list, a search, or a flat search with a particular sort.
- Bookmark context menu: Rename (also by double-click), Delete (with an "Unbookmarking…" confirmation).
- A bookmark icon also appears next to the zoomed root title, and search screens have a bookmark icon in their header.

**Tag pane (Pro)**
- Two collapsible sections:
  - **Document tags**, with a total count: tags in the current document.
  - **All tags**, with a total count: tags across all documents.
- Each row shows the tag text and its "(count)". Tags that differ only by case are merged.
- A sort toggle switches between by-occurrence (default, descending count) and alphabetical.
- Clicking a document tag runs an in-document search for it. Clicking a global tag runs a global search.
- Document tags support Rename (double-click or context menu) and Delete (with confirmation). These rewrite every occurrence. For global tags, the context menu only shows a message that tag operations are document-only.

### 1.3 Document area
- **Breadcrumb**: a row above the title, shown only when zoomed into an item.
  - Lists every ancestor from the document root down to the parent, separated by " > ".
  - Each crumb is a link (truncated with an ellipsis at ~150px) and is rendered with inline formatting.
  - Clicking a crumb zooms to that item. Ctrl/Cmd-click opens it in a new window or tab.
- **Title**: the zoomed root (or the document root) is rendered as a large title (~28px). Its note is shown below the title in full.
- Root items get the full set of context-menu options (see §4). The title has no bullet. On hover, a menu icon appears to the left of the title, plus a bookmark icon.
- **Narrow layout** (setting, default on): the document column has a max width of ~820px and is centered. Otherwise the document fills the width.
- **Document border** (advanced setting): the app background turns light grey and the document becomes a white rounded card with a thin border.
- **Current item highlight** (setting): a faint grey full-width bar behind the line being edited.
- **Document tools**, at the top-right of the document:
  - Search icon. It expands into a ~300px search input with a clear (×) button and a bookmark-this-search icon.
  - View-options icon. It opens a "view filter" popover (§1.4).
- **Search tip popover**: shown under the search input while typing; can be disabled in settings. It offers:
  - "Flat search" (Shift+Enter)
  - "Search in all documents" (Ctrl+Enter)
  - A hint about operators such as `is:completed` and `edited:4d`, with a "Learn more" link.
- **Status bar** (optional "Show word count" setting): a bottom-right strip showing total words, words in items, words in notes, and item count.
- **Backlinks section**: shown below the document when the "Show backlinks" setting is on and the zoomed item is referenced elsewhere. It has the heading "This item is referenced in:" and lists the referencing items.
- **Empty states**:
  - No document open: shows "No document is open" with suggestions: file finder (Ctrl+O), "Create a new document", or open one from the file explorer.
  - Invalid link: "Cannot open document".
  - Loading or failure: a spinner, or "Failed to load document" with a Retry button.

### 1.4 View-options popover (per-document overrides)
Every row has a "Global (current global value)" choice plus explicit choices. Per-document values are stored with the document's user data.

| Option | Choices |
|---|---|
| Checked items | Global / Show / Hide |
| Notes | Global / Show / 1st line / Hide |
| Layout | List / Article / Mind map (Pro, read-only) |
| Text direction | Global / Left-to-right / Right-to-left |

- **Article layout**: bullets are hidden (they reappear on hover), guide lines are hidden, and items get more vertical padding (~10px), so the outline reads as paragraphs.
- **Mind map**: a read-only full-screen canvas.
  - Top bar: "Back to document", mind-map breadcrumbs, zoom in/out buttons.
  - Nodes are boxes that keep their color label (box fill and border), heading styling, checked styling (grey), and notes.
  - Each node has an expand/collapse toggle.
  - When the document hides checked items, checked items and their descendants are omitted.

---

## 2. Item (node) rendering

### 2.1 Anatomy of a row (left to right)
1. **Hover icons**, floating in the left gutter (~100px to the left of the bullet): a menu (☰) icon that opens the item menu, and a zoom icon. On the current root, a bookmark icon also appears. The zoom icon is hidden when "use bullet to zoom" is on.
2. **Backlink counter** (when Show backlinks is on and count > 0): a small 18px round badge with the count, just left of the bullet. Clicking it shows the references.
3. **Bullet**: a small dot glyph (~16–18px font size).
   - **Collapsed parent**: the dot is drawn with a surrounding halo/ring, darker and larger, to show hidden children.
   - **Hover over a parent's bullet** (when bullet-to-zoom is off): the bullet turns into a disclosure arrow (pointing down if expanded, right if collapsed), and clicking toggles collapse.
   - **Bullet-to-zoom on**: clicking the bullet zooms in. Hovering shows a round grey highlight behind the dot, and a separate expand/collapse arrow appears in the hover icons.
   - **Bullet tooltip**: shows when the item was created and last edited ("Created at …", "Last edited at …").
   - **Drag**: dragging the bullet moves the item (and its subtree).
4. **Numbered list**: when the parent has "Number children" on, each child's bullet is replaced by "1.", "2.", … (counter per sibling group, ~32px wide). The parent's hover arrow shows a numbered-list variant.
5. **Checkbox** (only if the item has a checkbox): a 16px rounded square. Checked shows a tick mark.
6. **Content**: one line of rich text. Display mode renders markdown; edit mode shows raw text. Long text wraps.
   - **Color label**: tints the background of the content area with a pastel fill and a small horizontal padding.
   - **Heading 1/2/3**: bold text, larger font (see §2.3).
7. **Note** (below the content, aligned with the content's left edge): smaller font (~15px at medium size). Display mode is one of:
   - show: full note
   - 1st line: clamped to one line
   - hide: the note is hidden
   Notes that contain the current search match are always shown. Shift+Enter moves between content and note.

### 2.2 Children and guide lines
- Children are indented ~20px of padding plus ~10px of margin under the parent's bullet.
- A thin 1px vertical **guide line** runs down the left edge of every child block, in a very light grey (theme dependent). It is hidden for the zoomed root's direct children, in article layout, and inside a selected block.
- RTL mode mirrors everything: padding on the right, guide line on the right.
- Collapsing animates the children height.

### 2.3 Font size / density matrix
| | small | medium (default) | large |
|---|---|---|---|
| Item text | 16px | 18px | 20px |
| H3 | 18px | ~20px | ~22px |
| H2 | 20px | ~22px | ~24px |
| H1 | 22px | ~24px | ~26px |
| Inline code / chips | 14px | 16px | 18px |
| Zoomed root title | ~28px for all sizes | | |

Vertical padding per row, by density: **Compact** 0, **Cozy** (default) 2px top and bottom, **Comfortable** 5px top and bottom, **Article view** 10px.

### 2.4 Checked / completed items
- The checked item's content and note turn grey (light themes use a light grey) and are struck through. Inline code inside a checked item inherits the grey color.
- When "Show checked items" is off (globally or per document), checked items **and everything under them** are hidden. Descendants of a checked item count as completed.
- With checked items hidden, checking an item off moves the cursor to the next visible item, and the item disappears.
- Searching with `is:completed` forces checked items to show.
- Checking a parent does **not** check or uncheck its children. "Uncheck all children" exists as a separate command.
- Toggle-check on a multi-selection: if any selected item is unchecked, all become checked. Otherwise all become unchecked.
- "Delete checked items" removes checked items in the current scope.
- The root item cannot be checked.
- **Recurring dates** (Pro): checking off an item whose first date has a recurrence works as follows:
  - The date in the text is advanced to the next occurrence. The span length is preserved for date ranges, and all-day stays all-day.
  - If "from completion" is set, the next occurrence is computed from today or now instead of from the due date.
  - Setting "Next date appears in new item" (default): a *checked clone* of the original item is inserted at the same position (a history record), and the original stays unchecked with the advanced date.
  - Setting "Existing date gets updated": only the date is advanced and the item stays unchecked.

### 2.5 Selection and multi-select
- Selected items (block selection by Shift+Up/Down, drag-select, or Ctrl+Shift+A) get a light-blue background over the whole subtree.
- Most commands apply to the whole selection.

### 2.6 Inline syntax rendered in items (from the in-app help table)
| Syntax | Result |
|---|---|
| `**bold**` | bold |
| `__italic__` | italic |
| `~~strike~~` | strikethrough |
| `==highlight==` | yellow highlight (semi-transparent yellow) |
| `` `code` `` | inline code chip (bordered grey pill) |
| ```` ```code``` ```` | multi-line code block with syntax highlighting and a copy button |
| `[text](url)` | link |
| `![alt](url)` | image. Shown as a chip that previews on hover, or inline if "Display images inline" is on |
| `$$E = mc^2$$` | LaTeX (rendered math) |
| `#tag` / `@tag` | tag chip, grey text; optional light-blue background with the "Tag background color" setting |
| `[[` | triggers the item-link picker, producing an internal link to another item |
| `!` (at start or after a space) | opens the date picker, producing a date chip |

Tag rules:
- A tag starts with `#` or `@` at the start of the text or after whitespace.
- It ends at whitespace or punctuation (comma, period, colon, `?`, `!`, `;`, `/`, unmatched `)`, and similar).
- Purely numeric tags (e.g. `#123`) are not tags.
- Tag autocomplete triggers on `#` or `@@`. It lists tags from the same document, or from all documents if "Auto-complete tags from all docs" is on.

### 2.7 Colors
Seven label values: 0 = none, 1 red, 2 orange, 3 yellow, 4 green, 5 blue, 6 purple. Fills are light pastels on light themes and deep, dark shades on dark themes:

| # | Name | Light theme fill | Dark theme fill |
|---|---|---|---|
| 1 | Red | `#f9d0c4` | `#651f0a` |
| 2 | Orange | `#fed8b1` | `#743b01` |
| 3 | Yellow | `#fef2c0` | `#6f5a02` |
| 4 | Green | `#c8e3c0` | `#2a4920` |
| 5 | Blue | `#c5def5` | `#10385d` |
| 6 | Purple | `#d4c5f9` | `#403a67` |

(These are plain color values, listed for parity.)
- "Toggle color label" (Ctrl+Shift+L) cycles 0 → 1 → … → 6 → 0.
- "Toggle heading" (Ctrl+Shift+H) cycles 0 → 1 → 2 → 3 → 0.
- Choosing a specific heading that is already applied to all selected items clears it.

---

## 3. Dates, reminders, calendar

- **Date syntax** stored in the text: `!(START[ - END][ | RECURRENCE])`.
  - START and END are `YYYY-MM-DD`, optionally followed by ` HH:mm`, optionally followed by a timezone offset (only when the "Encode timezone" setting is on).
  - A date without a time is *all-day*.
  - RECURRENCE is `[~]<n><unit>[weekdays]`:
    - unit is `d`, `w`, `m`, or `y`
    - the optional leading `~` means "repeat from completion"
    - weekdays are ISO digits 1–7 (Mon–Sun), used with `w` (e.g. `1w135` means every week on Mon, Wed, Fri)
    - a missing n means 1
- **Rendering**: a rounded chip.
  - Relative labels: "Today", "Yesterday", "Tomorrow", "In N days", "N days ago" (for fewer than 7 days); otherwise a long form like "Mon, January 3rd 2022".
  - A time is appended in 12h ("3 pm", "3:30 pm") or 24h ("15:30") format according to the time-format setting.
  - Ranges show "start - end". Same-day ranges show only the end time.
  - Custom display formats (Pro) can be set separately for dates with and without time. They are format strings, or the value "relative".
  - Recurrence is described in words, e.g. "every 2 weeks on Mon, Wed".
- **Overdue** (setting "Highlight overdue"): unchecked items with a past date get a translucent red chip with a red border. Checked items keep the neutral chip.
- **Date picker** popover, opened by typing `!`, clicking a date chip, or "Add date" on the mobile toolbar:
  - Month calendar. The first day of the week follows the setting (Sat/Sun/Mon; default Sunday).
  - Start time input (accepts typed times, plus "All day" and "Now"). Invalid input shows "Please enter a valid time format like 22:42".
  - "Create date range" toggle. It adds an end-time input; you click a start date, then an end date. A help tooltip explains this.
  - Repeat dropdown: Does not repeat, Daily, "Weekly on <weekday>", "Monthly on <Nth>", "Annually on <Month D>", Custom… (Pro).
  - "From completion" checkbox (Pro), with a tooltip.
  - Done (Enter) and Esc to cancel. A delete-date action is available on mobile.
  - "Insert current date" and "Insert current time" are separate commands.
- **Custom recurrence modal**: "Repeat every [N] [Day|Week|Month|Year]". For Week it adds "Repeat on" toggles for M T W T F S S. Buttons: Done, Cancel.
- **Google Calendar integration** (Settings → Pro tab):
  - Link or unlink a Google account.
  - Choose the calendar that synced dates go to ("Sync events to" dropdown, with Refresh calendars).
  - Shows how many documents are synced.
  - "Bulk toggle sync": enable or disable for all documents.
  - Each document has its own toggle in its context menu. Dated items in synced documents become calendar events.
- **Email to inbox**: a per-user inbox email address with a Copy button. "Trim incoming emails" setting: Always / Never / Only when content is left.

---

## 4. Item context menu (bullet menu / right-click)

Groups are shown in this order. Items are shown or hidden depending on context.

1. **Edit**
   - Collapse / Expand (Ctrl+.)
   - Collapse all / Expand all (Ctrl+Shift+.)
   - Expand to level ▸ (Level 1–4)
   - Collapse all siblings / Expand all siblings
   - Zoom in (Ctrl+])
   - Add note (Shift+Enter)
   - Delete (Ctrl+Shift+Backspace)
   - Delete checked items
   - Add to bookmarks
   - Sort ▸
     - Title A→Z / Z→A
     - Date new→old / old→new
     - Unchecked first / Checked first
     - Edited new→old / old→new
     - Created new→old / old→new
     - Reverse current
   - Search and replace…
2. **Indent**
   - Indent (Tab), Unindent (Shift+Tab)
   - Move to… (Ctrl+Shift+M): opens a "Move item" chooser that searches for a target item or document
   - Insert template… (Pro): opens a chooser to pick an item whose subtree is copied here
3. **Checkbox**
   - Add checkbox / Remove checkbox (Ctrl+Shift+C)
   - Add checkbox to children / Remove checkbox from children (recursive)
   - Check off / Uncheck (Ctrl+Enter)
   - Number children / Stop numbering children (Ctrl+Shift+X)
4. **Export**
   - Manage sharing… (root only)
   - Get link: a modal with the item's URL and a Done button
   - Show all references
   - Export…
   - Set as inbox
5. **Color labels**: a strip of seven "A" swatches (6 colors plus clear). The current label is outlined.
6. **Heading**: three swatches H1 H2 H3. The current level is outlined.

Visibility rules:
- Expand/collapse items appear only when relevant (has children, collapsed state).
- Indent is hidden for the first visible child. Unindent is hidden for direct children of the zoomed root.
- Check vs Uncheck, Add vs Remove checkbox, and Number vs Stop numbering show the applicable one.
- Multi-selection hides Zoom in, Add note, Add to bookmarks, and Get link.
- Read-only documents hide all editing groups.
- The root item shows only the edit, checkbox, and export groups.

**Node toolbar** (hover toolbar on desktop, bottom toolbar on mobile; customizable on mobile):
- Unindent, Indent, Zoom in, Delete, Check off, Edit note, Add date, Upload a file, Toggle checkbox, Set color label, Set heading level, Duplicate, Export item, Insert new line, Customize toolbar.
- Mobile extras: Move item up/down, Insert hash tag, Insert at tag, Make date range.

---

## 5. Search

### 5.1 Modes
| Mode | Trigger | Behavior |
|---|---|---|
| In-document filter | Ctrl+F, or the search icon | Filters the outline live as you type. Matching items stay visible together with their ancestor chain. Ancestors are force-expanded for the search and restored afterward. Matches are highlighted in semi-transparent yellow. Notes with a match are shown. The query is part of the URL state, so it can be bookmarked. |
| Flat search | Shift+Enter in the search box | Replaces the document with a flat list of matching items. Header: "Search results for "q" in "<doc>"". Each result has a grey ancestor breadcrumb. **Sort by** dropdown: None, Title A–Z / Z–A, Date new→old / old→new, Unchecked first, Checked first, Edited new→old / old→new, Created new→old / old→new. Summary: "Found N matches:". "Go back [Esc]". |
| Global search | Ctrl+Enter, or a global tag click | "Search everywhere" screen with its own search bar. Results are grouped per document in bordered cards with the document title. Summary as above. When all matches are hidden by the checked-items filter, it shows: "No results found. N matches have been filtered out by your filter settings. Append "is:completed" …". |
| File finder | Ctrl+O | Full-screen prompt: "Search any document or bookmark to jump to…". Fuzzy match. ↑↓ to move, Enter to open, Esc to close. |
| Item finder (Pro) | Ctrl+Shift+O | Same prompt style. Searches items across all documents. |
| Link picker | type `[[` | Inline suggestion list of items across documents, with breadcrumb context. Checked items are greyed. |

Fuzzy matching (finders and link picker):
- First try matching the query's word tokens in order. If that fails, match individual characters.
- Score penalties: number of separate match fragments, word-boundary misses, span spread, start offset, and target length. Best score first.

### 5.2 Query language
- Terms are split on spaces and **AND**ed.
- A bare `OR` (uppercase) starts a new alternative group; the groups are ORed.
- `-term` negates a term.
- `"exact phrase"` (or `-"…"`) is a case-sensitive, whole-word phrase match.
- Ordinary terms are case-insensitive substring matches on both content and note.
- Terms starting with `#` or `@` must match a whole word (boundaries are whitespace or punctuation).
- While typing, the last term is matched as a prefix.
- An operator followed by a space joins with the next word (`edited: 3d` is the same as `edited:3d`).
- `in:title` makes the following terms match content only. `in:note` makes them match notes only.

| Operator | Meaning |
|---|---|
| `is:completed` | Item is checked **or any ancestor is checked**. Also forces checked items to be visible. |
| `has:checkbox` / `is:checklist` | Item has a checkbox |
| `has:note` | Note is non-empty |
| `has:color` | Any color label |
| `color:red\|orange\|yellow\|green\|blue\|purple\|none` | Specific label |
| `is:heading` | Any heading |
| `heading:0-3` | Specific heading level |
| `has:children` | Has at least one child |
| `has:date` (alias `time:`) | Contains a date in content or note |
| `since:X` | Has a date whose range ends on or after X |
| `until:X` | Has a date that starts before the end of X |
| `within:X` | Date falls between now and X in the future (or between X and today if X is in the past). `within:today` means today. `within:0d` means from now to the end of today. |
| `edited:X` | Last edited within the past X (e.g. `edited:4d`) |
| `created:X` | Created within the past X |
| `parent:text` | Direct parent's content contains the text |
| `ancestor:text` | Any ancestor's content contains the text |

X may be:
- `now`, `today`, `yesterday`, or `tomorrow`
- a relative amount `<n>d` (days), `<n>w` (weeks), `<n>m` (months), or `<n>h` (hours); negative n means the past
- an absolute date (`YYYY-MM-DD` covers the whole day) or any parseable date and time

Relative values snap to local midnight.

### 5.3 Search and replace
A modal with Find, a "Match case" checkbox, and "Replace with". "Replace all" applies within the current scope.

---

## 6. Export / import / print / backup

- **Export modal** (for a document, an item, a selection, or a version-history item). Tabs:
  - **Formatted**: an HTML nested list. Uses `ul`, or `ol` when the parent is numbered. Bold, italic, and links become HTML. A note follows its item after a line break. Headings are wrapped in h1–h3. Checked items are wrapped in strikethrough. Downloaded as `.html`.
  - **Plain text**: one line per item. Indentation style (radio): Spaces (4 per level), Asterisks ("* " prefix), Dashes ("- " prefix), or No indentation. Numbered children get "1. ", "2. " prefixes. Note lines follow their item, indented to align with the text. Downloaded as `.txt`.
  - **OPML 2.0**:
    - Head: title, a flavor marker identifying the app, a source URL, owner name and email.
    - Each item is an `outline` element. Attributes: `text`, `_note`, `checkbox="true"`, `complete="true"`, `heading="1-3"`, `colorLabel="1-6"`, `listStyle` (dot or arabic), `collapsed="true"`.
    - Downloaded as `.opml`.
  - Option: "Export visible items only". It skips items hidden by collapse and checked filters, and also respects hidden notes.
  - Buttons: Copy to clipboard (shows "Copied!" feedback), Download as file.
  - There is no separate Markdown export. The inline text already *is* markdown-like, so plain text plus inline syntax serves that purpose.
- **Import**:
  - OPML file via the file-pane context menu. Each file becomes a new document named after the file. Standard OPML attributes are honored, including `note`, `checklist`, and `_complete` as aliases. Non-native OPML gets simple HTML (b, i, a) converted to markdown.
  - Pasting OPML or indented plain text into the editor creates a tree. Indentation depth defines nesting, and leading bullets are stripped.
  - Import from WorkFlowy (login form).
  - Drag and drop or paste of files uploads them as attachments (Pro, ≤50 MB per file, 1 GB per month). A link or image markdown is inserted for the upload.
- **Print**: the browser print path hides the header, sidebar, breadcrumb, menus, toolbars, search UI, and status bar, prints the document full-width on white at a ~10pt base size, and keeps colors exact.
- **Download backup** (main menu): a zip of all documents in plain text and OPML, plus the folder structure. Only server-synced data is included.
- Daily backup to Dropbox or Google Drive (Pro): link or unlink in settings.

---

## 7. Document / folder management

| Action | Where | Notes |
|---|---|---|
| New document / folder | Pane header button, empty-area context menu, folder context menu (creates inside), blank-slate link | New files are titled "Untitled" until renamed. They enter rename mode immediately. |
| Rename | Context menu, or double-click a row | Inline edit. Renaming the document also changes its root item title (they are the same thing). |
| Move | Drag within the file tree | Reorder, or nest into a folder |
| Make a copy | Document context menu | Duplicates the document |
| Duplicate item | Command or toolbar | Copies the item and its subtree right below it |
| Delete | Context menu | Confirmation modal. There is no trash UI in the bundle; recovery is via version history or backups. |
| Archive / Unarchive | Document context menu | Dimmed in the tree and not loaded in the background |
| Version history (Pro) | Document context menu | See below |
| Share | "Manage sharing…" | See below |
| Convert item → document | Drag items onto the file pane | |
| Convert document → item | Drag a document onto an item | Confirmation; the document is deleted |
| Set as inbox / Go to inbox / Capture to inbox | Item menu, command, Ctrl+Shift+I | Capture is a full-screen textarea with "Capture into my inbox…" and Add / Cancel buttons. The item goes to the start or end of the inbox location, per the setting. |

**Version history modal**
- Left column: list of versions (timestamps). The selected one is highlighted.
- Right side: a diff of that version. Added items have a translucent green background, removed items a translucent red background.
- Toggle: "Show version after change". When off, it shows the before state.
- Clicking an item in the diff opens the export panel for that item ("Back to version" returns), which is how content is restored.

**Sharing modal**
- If not shared: "Share this document/folder" button.
- Public link section: "View this document" and optional "Edit this document" checkboxes. Copy link. Link options: "Share as collapsed", "Display images inline", Theme (Light, Dark, Sepia, Sci-Fi, Sakura, Dark gold).
- Private collaborators: email field plus permission (View only / Can edit / Manage), Add button, and a list of current collaborators.
- "Stop sharing", with confirmation.
- A public viewer shows the read-only document, a "Copy to my Dynalist" button, and sign-up prompts.

---

## 8. Settings (complete list)

Settings modal tabs: **Preferences**, **Keymap**, **Dynalist Pro**.

The advanced section is revealed with "Show advanced settings". Preference values are synced per user and include custom CSS.

| Section | Setting (key) | Type / values | Default |
|---|---|---|---|
| Appearance | Theme (appearance.theme) | Auto-detect (follows OS dark mode: dark or light), Light, Dark, Sepia, Sci-Fi, Sakura, Dark gold, Christmas | auto-detect |
| | List density (display_density) | Comfortable / Cozy / Compact | cozy |
| | Highlight current item (ui.current_highlight) | bool | off |
| | Narrow document layout (center_doc) | bool | on |
| | Display images inline (display_images_inline) | bool | off |
| | Right-to-left (right_to_left) | bool | off |
| Viewing options | Show checked items (show_checked) | bool, per-doc override | show |
| | Notes (show_notes) | Show / 1st line / Hide, per-doc override | show |
| Font options | Font size (font_size) | Small / Medium / Large | medium |
| | Font (appearance.font) | Whitney, Whitney Light, Inter, Arial, Helvetica, Roboto (light), Lora, Calibri, Calibri Light, Consolas, System font | Whitney |
| | Eastern-Asian font (appearance.eastern_asian_font) | YaHei, KaiTi, Meiryo, MS PMincho, Hiragino Kaku Gothic, Hiragino Mincho, Malgun Gothic, Batang | YaHei |
| Input | Enable spell checker (editor.spellcheck) | bool | off |
| | Spell checker language (desktop) | en-US / de-DE | — |
| | Show word count (word_count) | bool | off |
| | Auto-complete tags from all docs (global_tag_autocomplete) | bool | off |
| | Keyboard layout (keyboard_layout) | English (US) / German. German remaps zoom in/out to Alt+Right/Left. | en |
| Date | Time format (time_format) | 12h / 24h | 12h |
| | First day of week (start_day) | Sat / Sun / Mon | Sunday |
| | Date & time display format (custom_date_display_format, Pro) | text | empty |
| | Date display format, no time (custom_date_display_format_no_time, Pro) | text | empty |
| | Encode timezone (show_timezone) | bool | off |
| | Highlight overdue (highlight_overdue) | bool | off |
| | Recurring date (recurrence_check_behavior, Pro) | Next date appears in new item / Existing date gets updated | new item |
| Internal links | Show backlinks (show_backlinks) | bool | off |
| Your inbox | Inbox destination (inbox_location) | picker ("Change…") | not set |
| | Move to position (inbox_move_position) | End / Start of inbox | end |
| Control | Use bullet point to zoom in (use_bullet_to_zoom) | bool | off |
| | Unindent items in-place (unindent_in_place) | bool | off |
| Mobile | Use WiFi only (only_use_wifi_to_sync); Use true black (use_true_black) | bool | off |
| Advanced | Tag background color (tag_bg) | bool | off |
| | Document border (doc_border) | bool | off |
| | Auto-focus on first item (auto_focus_first_item) | bool | off |
| | Show search tip popup (show_search_tips) | bool | on |
| | Font for dyslexia (dyslexia_font) | None / Open Dyslexic | none |
| | Move item to (move_item_sent_to_position) | End / Start of destination. Also used when dropping onto a collapsed parent. | end |
| | Clean up newlines option (show_cleanup_newlines_option) | bool | off |
| Pro tab | Plan status; Google Calendar (link, calendar, bulk toggle); Uploads (quota, "Allow others to access via link" (upload_permission), upload manager); Email to inbox (address, trim footer: on/off/only_nonempty); Daily backup (Dropbox / Google Drive); Offline mode (disables sync entirely); Custom CSS (custom_css, textarea) | | |
| Hidden / internal | export_visible_only, show_version_history_after (on), workflowy_shortcuts (off; alternative keymap), shortcut_override, mobile toolbar order | | |

**Keymap tab** (Pro to customize):
- Searchable list of every command, each with one or more bindings.
- Click a binding to record a new one. Bindings need at least one modifier (Ctrl, Shift, or Alt).
- "Reset to default shortcut".
- Some commands cannot be customized (e.g. Select all).

### 8.1 Themes (palette guidance)
| Theme | Background | Text | Guide line | Selection |
|---|---|---|---|---|
| Light (default) | white | near-black | very light grey | pale blue |
| Dark | near-black (#181818-ish) | light grey | dark grey | deep navy |
| Sepia | warm cream | dark brown | beige | light tan |
| Sci-Fi | near-black | cyan | teal-grey | dark teal |
| Sakura | very pale pink | dark magenta | pink-grey | light pink |
| Dark gold | near-black | pale gold | olive-grey | dark gold |
| Christmas | forest green with a red header and sidebar | off-white | dark gold | olive yellow |

Accent color across light themes is a bright azure (used for selected option borders and links). Bullets and header icons use a mid-grey.

---

## 9. Default keyboard shortcuts (Ctrl means Cmd on macOS)

| Group | Command | Default |
|---|---|---|
| Basics | Indent / Unindent | Tab / Shift+Tab |
| | Expand/collapse | Ctrl+. |
| | Zoom in / Zoom out | Ctrl+] / Ctrl+[ |
| | Delete item | Ctrl+Shift+Backspace |
| General | Toggle help (shortcuts & formatting side panel) | Ctrl+/ or Ctrl+Shift+/ |
| | Open settings | Ctrl+, |
| | Toggle file pane / bookmark pane | Ctrl+Shift+F / Ctrl+Shift+B |
| | Save | Ctrl+S |
| | Capture to inbox | Ctrl+Shift+I |
| | Toggle tag pane, toggle checked (global or doc), toggle notes (global or doc), toggle spellcheck, switch to mind map, bookmark current state | unbound |
| Lists | Delete from above | Ctrl+Shift+Delete |
| | Move between item & note | Shift+Enter |
| | Toggle checked | Ctrl+Enter |
| | Expand/collapse all | Ctrl+Shift+. |
| | Toggle checkbox | Ctrl+Shift+C |
| | Toggle numbered list | Ctrl+Shift+X |
| | Select this and previous / next | Shift+Up / Shift+Down |
| | Select everything | Ctrl+Shift+A |
| | Undo / Redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z |
| | Uncheck all children, siblings toggle, checkbox for children, duplicate, insert template, copy item link, delete checked, show references, sort title/date, expand to level 1–4 | unbound |
| Navigate | Move item | Ctrl+Shift+M |
| | Swap with previous / next | Ctrl+Up / Ctrl+Down |
| | Zoom into previous / next sibling | Ctrl+Shift+[ / Ctrl+Shift+] |
| | Go to first / last item | Ctrl+Home / Ctrl+End |
| | Go to inbox, parent, prev/next sibling, start/end of text | unbound |
| Search | Search in doc | Ctrl+F |
| | Search everywhere | Ctrl+Enter (in the search box) |
| | Flat search | Shift+Enter (in the search box) |
| | File finder / Item finder | Ctrl+O / Ctrl+Shift+O |
| | Search & replace | unbound |
| Format | Bold / Italic | Ctrl+B / Ctrl+I |
| | Inline code | Ctrl+` |
| | Link | Ctrl+K |
| | Insert new line in item | Ctrl+Shift+Enter |
| | Toggle heading | Ctrl+Shift+H |
| | Toggle color label | Ctrl+Shift+L |
| | Code block, strikethrough, highlight, LaTeX, H1/H2/H3/clear, color 1–6/clear, insert current date/time | unbound |

The WorkFlowy-style alternative keymap uses:
- Alt+Right / Alt+Left for zoom in / out
- Ctrl(control)+Up for collapse
- Shift+Alt+Up / Shift+Alt+Down to swap items

The **Shortcuts & Formatting** help panel slides in at the side. It has three tables: Triggers (`@@`/`#` tag autocomplete, `!` date, `[[` link), Formatting (see §2.6), and Shortcuts (grouped as above).

---

## 10. Miscellaneous UX
- **Onboarding**: an 8-step modal tutorial with short videos and captions, Next and page dots. It can be restarted from the main menu.
- **Toasts / dismissible messages**, for example:
  - "Link copied to your clipboard!"
  - "Successfully captured to your inbox."
  - "Made a mistake? You can undo it with Ctrl+Z."
  - tips such as "You can move an item by dragging its bullet point."
- **Generic error modal**: a title, a message, and an "Okay" button.
- **Upload manager**: a table of uploaded files (name, size, uploaded time).
- **Read/lock mode** (mobile primarily): makes the document non-editable. A lock icon in the header turns it off.
- **URL state**: document id, zoom node, focus node, search query, flat flag, sort, and mind-map flag are encoded in the URL. Back/forward walks these states.
