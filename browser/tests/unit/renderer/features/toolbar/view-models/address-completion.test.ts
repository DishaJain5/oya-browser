/** Address suggestions never reopen after dismissal or navigate using a stale selected row. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import {
  AddressCompletion,
  inlineFor,
} from '../../../../../../src/renderer/features/toolbar/view-models/address-completion.ts';
import { fakeBridge } from '../../../support/bridge.ts';

const rows = [
  { url: 'https://one.test/', title: 'One', source: 'history' },
  { url: 'https://two.test/', title: 'Two', source: 'bookmark' },
];
/** Flush asynchronous overlay work without real time or networking. */
const settle = () => new Promise((resolve) => setImmediate(resolve));
it('keeps typed text as the default, then wraps selection with arrows', async () => {
  const fake = fakeBridge({ addressSuggestions: rows });
  const vm = new AddressCompletion(fake.bridge);
  await vm.search('test');
  assert.equal(vm.selectedUrl, undefined);
  vm.move(1);
  assert.equal(vm.selectedUrl, rows[0].url);
  vm.move(-1);
  assert.equal(vm.selectedUrl, rows[1].url);
  vm.dismiss();
  assert.equal(vm.state.open, false);
  assert.equal(vm.selectedUrl, undefined);
  await settle();
});
it('ignores older results when a newer query finishes first', async () => {
  const old = Promise.withResolvers();
  const fake = fakeBridge({ addressSuggestions: (query) => (query === 'old' ? old.promise : [rows[1]]) });
  const vm = new AddressCompletion(fake.bridge);
  const request = vm.search('old');
  await vm.search('new');
  old.resolve([rows[0]]);
  await request;
  assert.deepEqual(vm.state.items, [rows[1]]);
});
it('does not reopen after blur, Escape, clearing input or disposal', async () => {
  for (const action of ['dismiss', 'dispose', 'clear']) {
    const pending = Promise.withResolvers();
    const fake = fakeBridge({ addressSuggestions: () => pending.promise });
    const vm = new AddressCompletion(fake.bridge);
    const request = vm.search('test');
    if (action === 'clear') await vm.search('');
    else vm[action]();
    pending.resolve(rows);
    await request;
    await settle();
    assert.equal(vm.state.open, false);
    assert.deepEqual(fake.called('showOverlay'), []);
  }
});
it('waits for a delayed native overlay show before hiding it', async () => {
  const pending = Promise.withResolvers();
  const fake = fakeBridge({ addressSuggestions: rows, showOverlay: () => pending.promise });
  const vm = new AddressCompletion(fake.bridge);
  await vm.search('test');
  await settle();
  vm.dismiss();
  assert.deepEqual(fake.called('hideOverlay'), []);
  pending.resolve();
  await settle();
  assert.deepEqual(fake.called('showOverlay'), [['address']]);
  assert.deepEqual(fake.called('hideOverlay'), [['address']]);
});
it('falls back to ordinary typed navigation when the library fails', async () => {
  const fake = fakeBridge({
    addressSuggestions: () => {
      throw new Error('unavailable');
    },
  });
  const vm = new AddressCompletion(fake.bridge);
  await vm.search('test');
  assert.deepEqual(vm.state.items, []);
  assert.equal(vm.state.open, false);
});

it('completes what was typed inline with the rest of the best address, but never after a deletion', async () => {
  const fake = fakeBridge({
    addressSuggestions: [{ url: 'https://www.github.com/', title: 'GitHub', source: 'history' }],
  });
  const vm = new AddressCompletion(fake.bridge);
  await vm.search('git', true);
  assert.equal(vm.state.inline, 'hub.com', 'a home page loses its trailing slash');
  await vm.search('git', false);
  assert.equal(vm.state.inline, '', 'Backspace takes the text back instead of completing it again');
  vm.dismiss();
  await settle();
});

it('completes only when the best address starts with exactly what was typed', () => {
  const best = { url: 'https://docs.test/guide/start', title: 'Guide', source: 'history' as const };
  assert.equal(inlineFor('docs.test/gu', best), 'ide/start');
  assert.equal(inlineFor('DOCS', best), '.test/guide/start');
  assert.equal(inlineFor('guide', best), '', 'a match in the middle is not completed');
  assert.equal(inlineFor('docs', undefined), '');
});
