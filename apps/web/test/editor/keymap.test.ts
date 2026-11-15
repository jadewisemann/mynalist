import { describe, expect, it } from 'vitest';
import { Keymap } from '../../src/editor/keymap.ts';

describe('Keymap with the WorkFlowy keys', () => {
  it('gives Alt+Left/Right to zoom out/in, not to Back/Forward', () => {
    const km = new Keymap({ workflowy: true });
    expect(km.resolve('Alt+Left')).toBe('zoom-out');
    expect(km.resolve('Alt+Right')).toBe('zoom-in');
    expect(km.keysFor('back')).toEqual([]);
    expect(km.keysFor('forward')).toEqual([]);
    expect(km.resolve('Alt+Shift+Up')).toBe('swap-up');
  });

  it('keeps Back/Forward on Alt+Left/Right without them', () => {
    const km = new Keymap();
    expect(km.resolve('Alt+Left')).toBe('back');
    expect(km.resolve('Alt+Right')).toBe('forward');
    expect(km.resolve('Mod+[')).toBe('zoom-out');
  });

  it('lets a key the user gave a command win over them', () => {
    const km = new Keymap({ workflowy: true, overrides: { back: ['Alt+Left'] } });
    expect(km.resolve('Alt+Left')).toBe('back');
    expect(km.resolve('Alt+Right')).toBe('zoom-in');
    const own = new Keymap({ workflowy: true, overrides: { 'zoom-out': ['Mod+J'] } });
    expect(own.resolve('Alt+Left')).toBe('back');
    expect(own.resolve('Mod+J')).toBe('zoom-out');
  });
});
