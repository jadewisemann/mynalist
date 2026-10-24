// Seeded random key sequences (differential fuzzing). Each step's state is traced on both apps;
// the report shows the first step where they diverge. Reproduce one with --only=fuzz/<seed>.

const FIXTURES = [
  'one\ntwo\nthree',
  'alpha\n  a1\n  a2\nbeta\n  b1\ngamma',
  'top\n  mid\n    deep\n  mid2\nlast',
  'x\ny\n  y1\n    y2\nz\n  z1',
  'single',
];
const KEYS = [
  'Enter', 'Enter', 'Backspace', 'Backspace', 'Delete', 'Tab', 'Tab', 'Shift+Tab', 'Shift+Tab',
  'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End',
  'Control+ArrowUp', 'Control+ArrowDown', 'Control+Enter', 'Control+Period',
  'Shift+ArrowDown', 'Shift+ArrowUp', 'Control+z', 'Control+y', 'Control+Shift+Backspace', 'Escape',
  { type: 'q' }, { type: 'ab' }, { type: ' ' },
];

function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const N = +(process.env.FUZZ_N ?? 60);
const STEPS = +(process.env.FUZZ_STEPS ?? 8);
const FROM = +(process.env.FUZZ_FROM ?? 1); // first seed: run a fresh range without re-checking old ones
const S = [];
for (let seed = FROM; seed < FROM + N; seed++) {
  const r = rng(seed * 7919);
  const pick = (xs) => xs[Math.floor(r() * xs.length)];
  const fx = pick(FIXTURES);
  const lines = fx.split('\n');
  const i = Math.floor(r() * lines.length);
  const len = lines[i].trim().length;
  const at = pick(['start', 'end', Math.floor(r() * (len + 1))]);
  const steps = [{ at: [i, at] }];
  for (let k = 0; k < STEPS; k++) steps.push(pick(KEYS));
  S.push({ group: 'fuzz', id: `fuzz/${seed}`, fx, steps, trace: true });
}
export default S;
