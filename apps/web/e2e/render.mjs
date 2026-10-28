// Render tests (apps/web/test/render): named scenarios + seeded fuzzing of the editor, checking the DOM against
// the model after every step. Builds and serves the test page itself.
//
//   node apps/web/e2e/render.mjs                 scenarios + fuzz (SEEDS=60 STEPS=200 by default)
//   SEED=1234 node apps/web/e2e/render.mjs       one fuzz seed (prints its full log on failure)
//   ONLY=zoom node apps/web/e2e/render.mjs       scenarios whose name contains "zoom", no fuzz
//   SEEDS=500 STEPS=400 SIZE=2000 SHAPE=flat ... heavier runs
import { execSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';

const dir = fileURLToPath(new URL('../', import.meta.url));
const cfg = 'test/render/vite.config.ts';
const exe = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const env = process.env;
const SEEDS = Number(env.SEEDS ?? 60);
const STEPS = Number(env.STEPS ?? 200);
const opts = { steps: STEPS, ...(env.SIZE ? { size: Number(env.SIZE) } : {}), ...(env.SHAPE ? { shape: env.SHAPE } : {}) };

execSync(`npx vite build --config ${cfg} --logLevel error`, { cwd: dir, stdio: 'inherit' });
const server = spawn('npx', ['vite', 'preview', '--config', cfg], { cwd: dir, stdio: 'ignore', detached: true });
const stop = () => {
  try {
    process.kill(-server.pid);
  } catch {}
};
process.on('exit', stop);

const url = 'http://localhost:5175/';
for (let i = 0; i < 60; i++) {
  if (await fetch(url).then((r) => r.ok, () => false)) break;
  await new Promise((r) => setTimeout(r, 300));
}

const browser = await chromium.launch({ executablePath: exe });
const failures = [];
const pageErrors = [];
async function page(variant) {
  const ctx = await browser.newContext({
    viewport: variant.phone ? { width: 390, height: 800 } : { width: 1100, height: 800 },
    reducedMotion: variant.reducedMotion ? 'reduce' : 'no-preference',
  });
  const p = await ctx.newPage();
  p.on('pageerror', (e) => pageErrors.push(String(e)));
  await p.goto(url);
  await p.waitForFunction(() => window.renderTests?.ready);
  return p;
}
const show = (f) => {
  console.log(`\nFAIL ${f.name} at step ${f.step}`);
  for (const e of f.errors) console.log(`  ! ${e}`);
  console.log('  log:');
  for (const l of f.log) console.log(`    ${l}`);
};

const t0 = Date.now();
if (!env.SEED) {
  const p = await page({});
  const r = await p.evaluate((only) => window.renderTests.scenarios(only), env.ONLY ?? '');
  console.log(`scenarios: ${r.ran - r.failures.length}/${r.ran} ok`);
  r.failures.forEach(show);
  failures.push(...r.failures);
}
if (!env.ONLY) {
  const seeds = env.SEED ? [Number(env.SEED)] : Array.from({ length: SEEDS }, (_, i) => 1000 + i);
  const variants = [{}, { reducedMotion: true }, { phone: true }];
  let ok = 0;
  for (const v of variants) {
    const p = await page(v);
    for (const seed of seeds.filter((s) => env.SEED || s % variants.length === variants.indexOf(v))) {
      const f = await p.evaluate(([s, o]) => window.renderTests.fuzz(s, o), [seed, opts]);
      if (f) {
        f.name += ` (${JSON.stringify(v)})`;
        show(f);
        failures.push(f);
      } else ok++;
    }
  }
  console.log(`fuzz: ${ok}/${seeds.length} seeds ok, ${STEPS} steps each`);
}
if (pageErrors.length) console.log(`page errors:\n  ${[...new Set(pageErrors)].slice(0, 10).join('\n  ')}`);
console.log(`${failures.length ? 'FAILED' : 'passed'} in ${((Date.now() - t0) / 1000).toFixed(0)}s`);
await browser.close();
stop();
process.exit(failures.length || pageErrors.length ? 1 : 0);
