import { describe, expect, it } from 'vitest';
import { menuTargets } from '../../src/ui/itemmenu.ts';

describe('item menu targets', () => {
  it('acts on the item it was opened on, not the item being edited', () => {
    // editing 'e' (targets() is ['e']) and no block selection: the menu of 'a' acts on 'a'
    expect(menuTargets('a', 'root', null, ['e'])).toEqual(['a']);
  });
  it('acts on the selected items when opened on one of them', () => {
    expect(menuTargets('b', 'root', ['c', 'b'], ['b', 'c'])).toEqual(['b', 'c']);
  });
  it('acts on the item alone when opened outside the selection', () => {
    expect(menuTargets('a', 'root', ['b', 'c'], ['b', 'c'])).toEqual(['a']);
  });
  it('acts on the zoomed-in title alone', () => {
    expect(menuTargets('z', 'z', ['z'], ['z'])).toEqual(['z']);
  });
});
