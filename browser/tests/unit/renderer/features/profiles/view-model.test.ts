/**
 * Unit tests for the profile card: the circle opens and closes it with its
 * overlay, a click on another profile switches to it, the active one does not
 * switch again, and Settings closes the card before opening the account page.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { ProfilesViewModel } from '../../../../../src/renderer/features/profiles/view-model.ts';

/** A card over a fake bridge and profile list, the active profile being `p1`. */
function setup() {
  const overlays: string[] = [];
  const changed: string[] = [];
  const opened: string[] = [];
  const bridge = {
    showOverlay: async (name: string) => void overlays.push(`show ${name}`),
    hideOverlay: async (name: string) => void overlays.push(`hide ${name}`),
  };
  const persona = {
    state: { active: 'p1' },
    load: async () => {},
    change: (id: string) => changed.push(id),
    dispose() {},
  };
  const vm = new ProfilesViewModel({ bridge, persona: persona as any, openSettings: () => opened.push('settings') });
  return { vm, overlays, changed, opened };
}

/** Lets queued overlay calls run. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

it('the circle opens the card over its overlay, and a second click closes it', async () => {
  const { vm, overlays } = setup();
  vm.toggle();
  await settle();
  assert.deepEqual([vm.state.open, overlays], [true, ['show profiles']]);
  vm.toggle();
  await settle();
  assert.deepEqual([vm.state.open, overlays], [false, ['show profiles', 'hide profiles']]);
});

it('picking another profile switches to it and closes the card', () => {
  const { vm, changed } = setup();
  vm.toggle();
  vm.pick('p2');
  assert.deepEqual([changed, vm.state.open], [['p2'], false]);
});

it('picking the profile in use only closes the card', () => {
  const { vm, changed } = setup();
  vm.toggle();
  vm.pick('p1');
  assert.deepEqual([changed, vm.state.open], [[], false]);
});

it('Settings closes the card and opens the account page', () => {
  const { vm, opened } = setup();
  vm.toggle();
  vm.settings();
  assert.deepEqual([opened, vm.state.open], [['settings'], false]);
});
