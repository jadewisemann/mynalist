import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { appearanceStyle, parseAppearance, toToml } from '../../src/design/appearance.ts';
import { BASE, GROUPS, SCHEMA } from '../../src/design/tokens.ts';

const css = readFileSync(new URL('../../src/styles.css', import.meta.url), 'utf8');
const err = (t: string) => {
  const r = parseAppearance(t);
  if (!('error' in r)) throw new Error(`accepted: ${t}`);
  return r.error;
};

describe('user appearance TOML', () => {
  it('reads what it changes, in the schema', () => {
    const r = parseAppearance('[color]\ntag = "#008080"\nline = "rgba(0, 0, 0, .5)"\n\n[type]\nsize = 17\n');
    expect(r).toEqual({ values: [['color.tag', '#008080'], ['color.line', 'rgba(0, 0, 0, .5)'], ['type.size', 17]] });
  });

  it('rejects errors with their line, as the native app does', () => {
    expect(err('[color]\nbg = "#fff"\ntag = "teal"\n').line).toBe(3);
    expect(err('[type]\n\nsize = 100\n').line).toBe(3);
    expect(err('[type]\nsize = 100\n').message).toBe('type.size: more than 40');
    expect(err('[color]\nnope = "#fff"\n').message).toBe('unknown entry color.nope');
    expect(err('[color]\nbg = \n').line).toBe(2);
    expect(err('[space]\npad = "x"\n').message).toBe('space.pad: a length is a number (pixels)');
    expect(err('[color]\nbg = "{space.pad}"\n').message).toBe('color.bg: a color reference is one color entry');
    expect(err('[type]\nheading_weight = 650.5\n').line).toBe(2);
    expect(err('[color]\nscheme = "blue"\n').message).toBe('color.scheme: one of light, dark');
    expect(err('[color]\nbg = "rgba(300, 0, 0, 1)"\n').message).toBe('color.bg: not a color: rgba(300, 0, 0, 1)');
  });

  it('writes back what it reads', () => {
    const src = '[color]\ntag = "#008080"\n\n[type]\nsize = 17\n';
    const r = parseAppearance(src);
    expect('values' in r && toToml(r.values)).toBe(src);
  });

  it('sets the custom properties styles.css reads, references as var()', () => {
    expect(appearanceStyle([['color.topbar_border', '{color.tag}'], ['type.size', 17], ['font.editor', '{font.ui}, Batang, serif']])).toEqual([
      ['--topbar-border', 'var(--tag)'],
      ['--efs', '17px'],
      ['--editor-font', 'var(--font), Batang, serif'],
    ]);
  });
});

describe('the design tokens', () => {
  it('every entry changes something: styles.css reads its property', () => {
    const unused = SCHEMA.filter((e) => e.css.startsWith('--') && !css.includes(`var(${e.css})`)).map((e) => e.id);
    // what only the native app draws from
    expect(unused).toEqual([]);
  });

  it('the base, the built-in themes and modes pass the user rules too (one format)', () => {
    expect('values' in parseAppearance(toToml(BASE))).toBe(true);
    for (const g of GROUPS) for (const m of g.modes) expect('values' in parseAppearance(toToml(m.values)), `${g.name}/${m.name}`).toBe(true);
  });
});
