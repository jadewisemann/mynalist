// Recorded Dynalist results ("golden" files), so the parity suites can run without the live demo.
//
// qa/golden/<suite>.json(.gz) = { meta, scenarios: { [id]: { fp, ok, status, dyn } } }
//   fp      fingerprint of the scenario definition: a changed scenario no longer matches its recording
//   dyn     what Dynalist did ({ pre, post, clip, trace, errors }), the expectation for replays
//   ok      whether our build matched at recording time (match/geometry/known, or flaky but matching when
//           re-run alone); replays fail only when a scenario that was ok stops being ok
// One scenario per line keeps git diffs of a re-recording readable; big suites are gzipped.

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gunzipSync, gzipSync } from 'node:zlib';

export const GOLDEN_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', 'golden');
const GZIP_OVER = 1 << 20;

/** Everything that determines what Dynalist does in a scenario (not its id, group or comparison tolerances). */
export function fingerprint(sc) {
  const def = { fx: sc.fx, steps: sc.steps, setup: sc.setup, render: sc.render, trace: sc.trace, marks: sc.marks };
  return createHash('sha1').update(JSON.stringify(def)).digest('hex').slice(0, 12);
}

const paths = (suite) => ({ plain: join(GOLDEN_DIR, `${suite}.json`), gz: join(GOLDEN_DIR, `${suite}.json.gz`) });

export function loadGolden(suite) {
  const p = paths(suite);
  if (existsSync(p.gz)) return JSON.parse(gunzipSync(readFileSync(p.gz)).toString());
  if (existsSync(p.plain)) return JSON.parse(readFileSync(p.plain, 'utf8'));
  return { meta: {}, scenarios: {} };
}

/** Only what compare() reads from Dynalist's side. */
export function goldenDyn(dyn, sc) {
  const out = { pre: dyn.pre, post: dyn.post };
  if (dyn.clip != null) out.clip = dyn.clip;
  if (sc.trace) out.trace = dyn.trace;
  if (dyn.errors?.length) out.errors = dyn.errors;
  return out;
}

export function saveGolden(suite, golden, order) {
  mkdirSync(GOLDEN_DIR, { recursive: true });
  const ids = [...new Set([...order, ...Object.keys(golden.scenarios)])].filter((id) => golden.scenarios[id]);
  const lines = ids.map((id) => `${JSON.stringify(id)}:${JSON.stringify(golden.scenarios[id])}`);
  const text = `{"meta":${JSON.stringify(golden.meta)},\n"scenarios":{\n${lines.join(',\n')}\n}}\n`;
  const p = paths(suite);
  rmSync(p.gz, { force: true });
  rmSync(p.plain, { force: true });
  if (text.length > GZIP_OVER) writeFileSync(p.gz, gzipSync(text, { level: 9 }));
  else writeFileSync(p.plain, text);
  return text.length > GZIP_OVER ? p.gz : p.plain;
}
