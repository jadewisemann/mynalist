// The shared render model (native/docs/handoff/6-design-source.md §3): the web app's model of the native fixtures, measured by the
// model page (apps/web/test/model) with the real editor and styles, compared with the native structural snapshot's
// `[model]` (native/crates/outliner-app/tests/snapshots), and both checked against the relations of
// design/spec.json in a set of modes.
//
//   node qa/model.mjs            measure, compare, check; exits 1 on a difference or a broken relation
//
// Positions are compared from the title's text (the web has a search row and breadcrumbs above the title, the
// native app not yet), the sidebar's from its box; widths not at all: glyph widths differ between the engines. Writes qa/out/model/.
import { execSync, spawn } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { getBrowser, closeBrowser, PHONE } from './lib/runner.mjs';
import { parseModel, compareModels, checkSpec, KNOWN } from './lib/model.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const goldens = join(root, 'native/crates/outliner-app/tests/snapshots');
const outDir = join(here, 'out', 'model');
mkdirSync(outDir, { recursive: true });

execSync('npx vite build --config test/model/vite.config.ts --logLevel error', { cwd: join(root, 'apps/web'), stdio: 'inherit' });
const server = spawn('npx', ['vite', 'preview', '--config', 'test/model/vite.config.ts'], { cwd: join(root, 'apps/web'), stdio: 'ignore', detached: true });
process.on('exit', () => {
  try {
    process.kill(-server.pid);
  } catch {}
});
const url = 'http://localhost:5176/';
for (let i = 0; i < 60; i++) {
  if (await fetch(url).then((r) => r.ok, () => false)) break;
  await new Promise((r) => setTimeout(r, 300));
}

const custom = readFileSync(join(root, 'design/test/custom.toml'), 'utf8');
const spec = JSON.parse(readFileSync(join(root, 'design/spec.json'), 'utf8'));
const combos = JSON.parse(readFileSync(join(root, 'design/test/combos.json'), 'utf8'));
// the order a combination's name lists its modes in (native tests/snapshot.rs `combos`)
const GROUP_ORDER = ['theme', 'size', 'density', 'font', 'eafont'];
const DEVICES = {
  desktop: { viewport: { width: 1100, height: 800 }, extra: {} },
  phone: { viewport: PHONE.viewport, extra: PHONE.device },
};
const b = await getBrowser();
const problems = [];
const known = [];
let checked = 0;
for (const [device, d] of Object.entries(DEVICES)) {
  const ctx = await b.newContext({ viewport: d.viewport, timezoneId: 'UTC', ...d.extra });
  const page = await ctx.newPage();
  // the native snapshot's clock
  await page.clock.install({ time: new Date('2024-06-15T12:00:00Z') });
  page.on('pageerror', (e) => problems.push(`${device}: page error: ${e.message}`));
  await page.goto(url);
  await page.waitForFunction(() => '__model' in window);
  const measure = (doc, modes, toml, editing, side = '') =>
    page.evaluate(([doc, modes, toml, editing, side]) => window.__model(doc, modes, toml, editing, side), [doc, modes, toml, editing, side]);

  // the same fixture and look on both apps; at rest, and with the item `fixture::edited_item` names being edited
  // (the bullet thread: `looks` edits an item at depth 2)
  const EDITED = { inline: 's0', looks: 'l17' };
  for (const [doc, look, toml] of [['inline', 'rest', ''], ['looks', 'rest', ''], ['looks', 'custom', custom], ['inline', 'edit', ''], ['looks', 'edit', '']]) {
    const name = `${doc}-${device}-${look}`;
    const web = await measure(doc, {}, toml, look === 'edit' ? EDITED[doc] : undefined);
    writeFileSync(join(outDir, `${name}.web.txt`), web);
    const native = readFileSync(join(goldens, `${name}.txt`), 'utf8');
    for (const p of compareModels(parseModel(native), parseModel(web))) {
      const k = KNOWN.find((k) => k.test(`${name}: ${p}`));
      (k ? known : problems).push(`${name}: ${p}${k ? `  (known: ${k.why})` : ''}`);
    }
    checked++;
  }
  // themes, modes and the user's appearance together (design/test/combos.json, native `structural_snapshots`)
  for (const c of combos.combos.filter((c) => c.device === device)) {
    const values = GROUP_ORDER.filter((g) => c.modes[g]).map((g) => c.modes[g]);
    const name = `${c.doc}-${device}-${values.join('-')}${c.custom ? '-custom' : ''}`;
    const web = await measure(c.doc, c.modes, c.custom ? custom : '');
    writeFileSync(join(outDir, `${name}.web.txt`), web);
    const native = readFileSync(join(goldens, `${name}.txt`), 'utf8');
    for (const p of compareModels(parseModel(native), parseModel(web))) {
      const k = KNOWN.find((k) => k.test(`${name}: ${p}`));
      (k ? known : problems).push(`${name}: ${p}${k ? `  (known: ${k.why})` : ''}`);
    }
    checked++;
  }
  // the sidebar on a pane (native `structural_snapshots`): the phone drawer on its files, the Tags tab
  const SIDES = { phone: [['looks', 'files'], ['inline', 'tags']], desktop: [['inline', 'tags']] };
  for (const [doc, side] of SIDES[device]) {
    const name = `${doc}-${device}-${side}`;
    const web = await measure(doc, {}, '', undefined, side);
    writeFileSync(join(outDir, `${name}.web.txt`), web);
    const native = readFileSync(join(goldens, `${name}.txt`), 'utf8');
    for (const p of compareModels(parseModel(native), parseModel(web))) {
      const k = KNOWN.find((k) => k.test(`${name}: ${p}`));
      (k ? known : problems).push(`${name}: ${p}${k ? `  (known: ${k.why})` : ''}`);
    }
    checked++;
  }

  // the relations, in the chosen modes
  for (const modes of spec.modes) {
    for (const doc of ['looks', 'inline']) {
      const web = parseModel(await measure(doc, modes, ''));
      for (const p of checkSpec(spec, web, { ...modes, device }, doc)) problems.push(`web ${doc} ${device} ${JSON.stringify(modes)}: ${p}`);
      checked++;
    }
  }
  await ctx.close();
}
await closeBrowser();

for (const k of known) console.log(`known ${k}`);
for (const p of problems) console.log(`DIFF ${p}`);
console.log(`model: ${checked} models, ${problems.length} problems, ${known.length} known differences`);
process.exit(problems.length ? 1 : 0);
