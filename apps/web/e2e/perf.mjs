import { chromium } from 'playwright-core';
const exe = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const N = Number(process.env.N ?? 10000);
const browser = await chromium.launch({ executablePath: exe });
const page = await (await browser.newContext({ viewport: { width: 1200, height: 800 } })).newPage();
page.on('pageerror', (e) => console.log('pageerror', String(e)));
await page.goto('http://localhost:5173/');
await page.waitForSelector('.o-editor');
await page.click('.pane-head button'); await page.click('text=New document');
await page.keyboard.type('Perf'); await page.keyboard.press('Enter');
await page.waitForTimeout(200);
await page.click('.o-editor', { position: { x: 300, y: 120 } });
// build N lines, 3 levels deep, with formatting
const lines = [];
for (let i = 0; i < N; i++) lines.push('    '.repeat(i % 3) + `Item ${i} with **bold** #tag${i % 50} and \`code\` !(2026-11-${String(1 + (i % 28)).padStart(2, '0')})`);
const t0 = Date.now();
await page.evaluate((text) => {
  const dt = new DataTransfer();
  dt.setData('text/plain', text);
  document.activeElement.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
}, lines.join('\n'));
console.log(`paste+render ${N} items: ${Date.now() - t0} ms, rows: ${await page.$$eval('.o-node', (e) => e.length)}`);
// reload: cold load from IndexedDB
await page.waitForTimeout(1500);
const t1 = Date.now();
await page.reload();
await page.waitForFunction((n) => document.querySelectorAll('.o-node').length >= n, N);
console.log(`reload + full render: ${Date.now() - t1} ms`);
// typing latency in the middle of the doc
await page.click('.o-content >> text=Item 5000 with', { timeout: 5000 }).catch(async () => page.click('.o-node:nth-child(10) .o-content'));
await page.keyboard.press('End');
const lat = await page.evaluate(async () => {
  const el = document.activeElement;
  const times = [];
  for (let i = 0; i < 50; i++) {
    const s = performance.now();
    document.execCommand('insertText', false, 'x');
    times.push(performance.now() - s);
  }
  times.sort((a, b) => a - b);
  return { median: times[25].toFixed(2), p95: times[47].toFixed(2), max: times[49].toFixed(2) };
});
console.log('keystroke (ms):', lat);
const enterLat = await page.evaluate(async () => {
  const times = [];
  for (let i = 0; i < 20; i++) {
    const s = performance.now();
    document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true }));
    times.push(performance.now() - s);
  }
  times.sort((a, b) => a - b);
  return { median: times[10].toFixed(2), max: times[19].toFixed(2) };
});
console.log('enter/split (ms):', enterLat);
const tabLat = await page.evaluate(async () => {
  const times = [];
  for (let i = 0; i < 10; i++) {
    const s = performance.now();
    document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', code: 'Tab', shiftKey: i % 2 === 1, bubbles: true, cancelable: true }));
    times.push(performance.now() - s);
  }
  times.sort((a, b) => a - b);
  return { median: times[5].toFixed(2), max: times[9].toFixed(2) };
});
console.log('tab/shift-tab (ms):', tabLat);
await browser.close();
