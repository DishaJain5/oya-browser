/**
 * Unit tests for the bookmarks bar: it follows the main process (bookmarks and
 * whether it shows), opens a bookmark here or, Cmd/Ctrl held, in a new tab, and hides.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { BookmarksBarViewModel } from '../../../../../../src/renderer/features/toolbar/view-models/bookmarks-bar-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** Lets pending promises settle. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

it('shows the bookmarks the main process sends, and hides when it is turned off', async () => {
  const fake = fakeBridge({
    bookmarksBar: {
      shown: true,
      bookmarks: [
        { url: 'https://a.test/', title: 'A' },
        { url: 'https://b.test/', title: '' },
      ],
    },
  });
  const vm = new BookmarksBarViewModel(fake.bridge);
  await settle();
  assert.deepEqual(vm.state.bookmarks, [
    { url: 'https://a.test/', title: 'A' },
    { url: 'https://b.test/', title: 'https://b.test/' },
  ]);
  fake.emit('onBookmarksBar', { shown: false, bookmarks: [] });
  assert.equal(vm.state.shown, false);
  vm.dispose();
});

it('opens a bookmark in this tab, or in a new tab with Cmd/Ctrl held, and hides the bar on request', async () => {
  const fake = fakeBridge({ bookmarksBar: { shown: true, bookmarks: [] } });
  const vm = new BookmarksBarViewModel(fake.bridge);
  vm.open('https://a.test/', false);
  vm.open('https://b.test/', true);
  vm.hide();
  await settle();
  assert.deepEqual(fake.called('navigate'), [['https://a.test/']]);
  assert.deepEqual(fake.called('newTab'), [['https://b.test/']]);
  assert.deepEqual(fake.called('setBookmarksBar'), [[false]]);
  vm.dispose();
});

it('knows every saved address for the star, and the star saves or removes the shown page', async () => {
  const fake = fakeBridge({ bookmarksBar: { shown: true, saved: ['https://a.test/', 7], bookmarks: [] } });
  const vm = new BookmarksBarViewModel(fake.bridge);
  await settle();
  assert.deepEqual(vm.state.saved, ['https://a.test/']);
  vm.toggleCurrent();
  await settle();
  assert.equal(fake.called('toggleBookmark').length, 1);
  vm.dispose();
});
