// App adapters: the same abstract operations against Dynalist (live demo, black-box) and our build.
// Each adapter: fresh(page) → empty doc with an empty first item focused; item(page, i) → locator of
// the i-th visible item's text (-1 = title); state(page) → normalized state (see state.mjs).

import { existsSync } from 'node:fs';

const LOCAL_CHROME = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
/** Chromium to drive: $CHROMIUM_PATH, this machine's copy, or playwright-core's own (`npx playwright-core install chromium`). */
export const EXE = process.env.CHROMIUM_PATH || (existsSync(LOCAL_CHROME) ? LOCAL_CHROME : undefined);
export const OURS_URL = process.env.OURS_URL ?? 'http://localhost:5174/';
export const DYN_URL = 'https://dynalist.io/demo';

/** Normalized state extraction for Dynalist, from its DOM (raw item text lives in `.Node-content`). */
function dynState() {
  const root = document.querySelector('.Node.is-currentRoot');
  const items = [];
  const els = [];
  const txt = (el) => (el ? el.textContent.replace(/\n$/, '') : '');
  const walk = (node, d) => {
    const kids = node.querySelector(':scope > .Node-children');
    if (!kids) return;
    for (const outer of kids.children) {
      const n = outer.matches('.Node') ? outer : outer.querySelector(':scope > .Node');
      if (!n || getComputedStyle(outer).display === 'none' || getComputedStyle(n).display === 'none') continue;
      const self = n.firstElementChild;
      items.push({
        d,
        c: txt(self.querySelector('.Node-content')),
        n: txt(self.querySelector('.Node-note')),
        ck: n.classList.contains('is-checked'),
        cb: n.classList.contains('has-checkbox'),
        h: +(self.className.match(/is-heading(\d)/)?.[1] ?? 0),
        num: self.classList.contains('is-numbered'),
        col: self.classList.contains('is-collapsed') && self.classList.contains('is-parent'),
      });
      els.push(n);
      if (!self.classList.contains('is-collapsed')) walk(n, d + 1);
    }
  };
  if (root) walk(root, 0);
  const zoomed = /[#&]z=/.test(location.hash);
  const a = document.activeElement;
  let focus = null;
  if (a && a.matches('.Node-content, .Node-note')) {
    const node = a.closest('.Node');
    const i = node === root ? -1 : els.indexOf(node);
    const sel = getSelection();
    const off = (n, o) => {
      if (!n || !a.contains(n)) return null;
      const r = document.createRange();
      r.setStart(a, 0);
      r.setEnd(n, o);
      return r.toString().length;
    };
    const max = txt(a).length;
    const x = Math.min(off(sel.anchorNode, sel.anchorOffset) ?? 0, max);
    const y = Math.min(off(sel.focusNode, sel.focusOffset) ?? 0, max);
    focus = { i: i < 0 && node !== root ? -2 : i, f: a.matches('.Node-note') ? 'n' : 'c', s: Math.min(x, y), e: Math.max(x, y) };
  }
  const selected = els.map((n, i) => (n.classList.contains('is-selected') ? i : -1)).filter((i) => i >= 0);
  let title = root ? txt(root.querySelector(':scope > .Node-self .Node-content')) : '';
  if (title === 'Untitled') {
    title = ''; // Dynalist's empty-title placeholder (its text is in the DOM, so a caret in it reads offset 8)
    if (focus?.i === -1) focus = { ...focus, s: 0, e: 0 };
  }
  return { zoom: zoomed ? title : null, title, items, focus, selected };
}


/**
 * Rendered (not edited) item text as styled runs: [{ t: text, f: 'b,i,…', x?: extra }], per item.
 * `cls` maps an element to a flag (or null) — the only app-specific part.
 */
function renderRuns(rows, cls) {
  return rows.map((el) => {
    const runs = [];
    const walk = (n, flags, extra) => {
      for (const c of n.childNodes) {
        if (c.nodeType === 3) {
          const t = c.nodeValue.replace(/\n$/, '');
          if (!t) continue;
          const f = [...new Set(flags)].sort().join(',');
          const last = runs[runs.length - 1];
          if (last && last.f === f && JSON.stringify(last.x) === JSON.stringify(extra)) last.t += t;
          else runs.push(extra ? { t, f, x: extra } : { t, f });
        } else if (c.nodeType === 1) {
          const m = cls(c);
          if (m?.skip) continue;
          if (m?.atomic) {
            runs.push({ t: m.text ?? c.textContent.trim(), f: [...flags, m.flag].sort().join(','), x: m.x });
            continue;
          }
          walk(c, m?.flag ? [...flags, m.flag] : flags, m?.x ?? extra);
        }
      }
    };
    walk(el, [], undefined);
    return runs;
  });
}

export const dynalist = {
  name: 'dynalist',
  async fresh(page) {
    await page.goto(DYN_URL, { waitUntil: 'domcontentloaded', timeout: 60000 });
    await page.waitForSelector('.Node-self.is-contentRendered', { timeout: 60000 });
    // tutorial popup and help pane only get in the way of clicks/screenshots
    await page.addStyleTag({ content: '.TutorialPopup, [class*=Tutorial], .HelpPane, .help-pane { display: none !important }' }).catch(() => {});
    await page.evaluate(() => {
      for (const el of document.querySelectorAll('div')) {
        if (el.children.length < 6 && /^Hi there, this is Erica/.test(el.textContent.trim())) el.style.display = 'none';
      }
    });
    // empty the demo document; the select-all/delete pair occasionally races, so verify and retry
    for (let k = 0; k < 4; k++) {
      const left = await page.locator('.Node.is-currentRoot .Node-children .Node').count();
      if (!left) break;
      await page.locator('.Node-children .Node-renderedContent').first().click();
      await page.waitForTimeout(80);
      await page.keyboard.press('Control+Shift+A');
      await page.waitForTimeout(120);
      await page.keyboard.press('Backspace');
      await page.waitForTimeout(150);
    }
    await this.item(page, -1).click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(80);
  },
  item(page, i) {
    if (i < 0) return page.locator('.Node.is-currentRoot > .Node-self .Node-contentContainer').first();
    return page.locator('.Node.is-currentRoot .Node:not(.is-currentRoot)').filter({ visible: true }).nth(i).locator(':scope > .Node-self .Node-contentContainer');
  },
  state: (page) => page.evaluate(dynState),
  render: (page) =>
    page.evaluate(`(${renderRuns})(
      [...document.querySelectorAll('.Node.is-currentRoot .Node-children .Node-renderedContent')],
      (el) => {
        const c = el.classList;
        if (c.contains('node-bold')) return { flag: 'b' };
        if (c.contains('node-italics')) return { flag: 'i' };
        if (c.contains('node-strike-through')) return { flag: 's' };
        if (c.contains('node-highlight')) return { flag: 'h' };
        if (c.contains('node-inline-code')) return { flag: 'code' };
        if (c.contains('node-multiline-code-copy-btn')) return { skip: true };
        if (c.contains('node-multiline-code')) return { flag: 'codeblock' };
        if ([...c].some((x) => x === 'hljs' || x.startsWith('hljs-'))) return null; // syntax highlight spans
        if (c.contains('node-tag')) return { flag: 'tag', atomic: true };
        if (c.contains('node-time')) return { flag: 'date', atomic: true };
        if (c.contains('node-latex')) return { flag: 'latex', atomic: true, text: el.getAttribute('title') };
        if (c.contains('node-inline-image')) return { flag: 'image', atomic: true, x: el.getAttribute('href') };
        if (c.contains('node-link')) return { flag: 'link', x: el.getAttribute('href') };
        if (el.tagName === 'SPAN' && !el.className) return null;
        return { flag: '?' + (el.className || el.tagName.toLowerCase()) };
      })`),

  async clip(page) {
    const r = await page.locator('.Node.is-currentRoot').first().boundingBox();
    return { x: Math.max(0, r.x - 50), y: Math.max(0, r.y - 16), width: 640, height: Math.min(560, r.height + 40) };
  },
};

export const ours = {
  name: 'ours',
  async fresh(page) {
    await page.goto(OURS_URL);
    await page.waitForSelector('.o-editor');
    await page.click('.pane-head button');
    await page.click('text=New document');
    await page.waitForTimeout(80);
    await page.keyboard.type('Welcome to Dynalist'); // same title text as the demo doc
    await page.keyboard.press('Enter');
    await page.waitForTimeout(80);
    await this.item(page, -1).click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(80);
  },
  item(page, i) {
    if (i < 0) return page.locator('.o-title .o-content').first();
    return page.evaluate((i) => window.__qa().items[i]?.id, i).then((id) => page.locator(`.o-node[data-id="${id}"] > .o-row .o-content`));
  },
  render: (page) =>
    page.evaluate(`(${renderRuns})(
      [...document.querySelectorAll('.o-items .o-node > .o-row .o-content')],
      (el) => {
        const c = el.classList;
        if (c.contains('o-bold')) return { flag: 'b' };
        if (c.contains('o-italic')) return { flag: 'i' };
        if (c.contains('o-strike')) return { flag: 's' };
        if (c.contains('o-highlight')) return { flag: 'h' };
        if (c.contains('o-code')) return { flag: 'code' };
        if (c.contains('o-codeblock')) return { flag: 'codeblock' };
        if (c.contains('o-tag')) return { flag: 'tag', atomic: true };
        if (c.contains('o-date')) return { flag: 'date', atomic: true };
        if (c.contains('o-latex')) return { flag: 'latex', atomic: true, text: el.dataset.tex };
        if (c.contains('o-image') || c.contains('o-img')) return { flag: 'image', atomic: true, x: el.getAttribute('href') ?? el.getAttribute('src') };
        if (c.contains('o-link')) return { flag: 'link', x: el.getAttribute('href') };
        if (c.contains('o-t') || c.contains('o-esc') || c.contains('o-match')) return null;
        if (el.tagName === 'CODE' && el.parentElement.classList.contains('o-codeblock')) return null;
        return { flag: '?' + (el.className || el.tagName.toLowerCase()) };
      })`),
  async state(page) {
    const s = await page.evaluate(() => window.__qa());
    for (const it of s.items) delete it.id;
    return s;
  },
  async clip(page) {
    const r = await page.locator('.o-editor').first().boundingBox();
    const t = await page.locator('.o-title').first().boundingBox();
    return { x: Math.max(0, t.x - 50), y: Math.max(0, t.y - 16), width: 640, height: Math.min(560, r.y + r.height - t.y + 16) };
  },
};

export const APPS = { dynalist, ours };
