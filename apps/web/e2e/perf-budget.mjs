// Performance budgets: editing latency in large documents must not regress. Exits non-zero over budget.
// Each edit is timed with the style recalc + layout it causes (median of 15). Needs the snapshot server (snapshot.sh).
//
//   node apps/web/e2e/perf-budget.mjs           20k items, flat and grouped
//   N=50000 node apps/web/e2e/perf-budget.mjs   bigger (budgets scale with N/20000)
import { chromium } from 'playwright-core';

const exe = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const N = Number(process.env.N ?? 20000);
const scale = Math.max(1, N / 20000);
// ms at 20k items, about 3x what this machine measures, so that only real regressions trip them
const BUDGET = { load: 6000, type: 3, down: 30, up: 30, enter: 45, threadingOff: { down: 30 } };
// shapes: one flat list (the worst case for a single container) and 40 parents of 500 children (chunked
// children blocks, where the bullet-threading selectors have the most to match)
const SHAPES = {
  flat: (i) => '    '.repeat(i % 3),
  grouped: (i) => (i % 500 === 0 ? '' : '    '),
};

const browser = await chromium.launch({ executablePath: exe });
const over = [];
for (const [shape, indent] of Object.entries(SHAPES)) {
  const page = await (await browser.newContext({ viewport: { width: 1200, height: 800 } })).newPage();
  page.on('pageerror', (e) => over.push(`${shape}: page error ${e}`));
  await page.goto(process.env.URL ?? 'http://localhost:5174/');
  await page.waitForSelector('.o-editor');
  await page.click('.pane-head button[title^="New"]');
  await page.click('text=New document');
  await page.keyboard.type('Perf');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(200);
  await page.click('.o-editor', { position: { x: 300, y: 120 } });
  const lines = [];
  for (let i = 0; i < N; i++) lines.push(`${indent(i)}Item ${i} with **bold** #tag${i % 50} and \`code\` !(2026-11-${String(1 + (i % 28)).padStart(2, '0')})`);
  await page.evaluate((text) => {
    const dt = new DataTransfer();
    dt.setData('text/plain', text);
    document.activeElement.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  }, lines.join('\n'));
  await page.waitForTimeout(1500);
  const t0 = Date.now();
  await page.reload();
  await page.waitForFunction((n) => document.querySelectorAll('.o-node').length >= n, N, { timeout: 120000 });
  const load = Date.now() - t0;
  const mid = `Item ${Math.floor(N / 2)} with`;
  await page.evaluate((t) => [...document.querySelectorAll('.o-content')].find((e) => e.textContent.startsWith(t))?.scrollIntoView({ block: 'center' }), mid);
  await page.click(`.o-content >> text=${mid}`);
  await page.keyboard.press('End');
  const measure = () => page.evaluate(() => {
    if (!document.activeElement?.isContentEditable) throw new Error('perf: no item is being edited');
    const m = (fn) => {
      const t = [];
      for (let i = 0; i < 15; i++) {
        const a = performance.now();
        fn();
        document.body.getBoundingClientRect();
        t.push(performance.now() - a);
      }
      return +t.sort((p, q) => p - q)[7].toFixed(1);
    };
    const key = (k) => () => document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: k, code: k, bubbles: true, cancelable: true }));
    return { type: m(() => document.execCommand('insertText', false, 'x')), down: m(key('ArrowDown')), up: m(key('ArrowUp')), enter: m(key('Enter')) };
  });
  const on = { load, ...(await measure()) };
  await page.evaluate(() => document.querySelector('.o-editor').classList.remove('o-thread'));
  const off = await measure();
  console.log(`${shape.padEnd(8)} ${N} items: ${JSON.stringify(on)}  threading off: ${JSON.stringify(off)}`);
  for (const k of ['load', 'type', 'down', 'up', 'enter']) {
    if (on[k] > BUDGET[k] * scale) over.push(`${shape}: ${k} ${on[k]}ms > ${BUDGET[k] * scale}ms`);
  }
  if (off.down > BUDGET.threadingOff.down * scale) over.push(`${shape}: down without threading ${off.down}ms > ${BUDGET.threadingOff.down * scale}ms`);
  await page.close();
}
await browser.close();
for (const o of over) console.log(`OVER BUDGET ${o}`);
console.log(over.length ? 'FAILED' : 'within budget');
process.exit(over.length ? 1 : 0);
