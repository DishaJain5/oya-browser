/** Collection tabs reuse normal navigation while private data stays in the trusted shell. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { showLibrary, readLibrary, changeLibrary } from '../../../../src/main/library/tabs.ts';
import { leaveHomeFor, contentReady } from '../../../../src/main/tabs/home.ts';
import { BrowsingLibrary } from '../../../../src/main/library/index.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
/** Window-local tabs and a mutable profile/ownership seam exercise the public commands. */
function fixture() {
  const deps = mainCtx();
  deps.library = new BrowsingLibrary(deps);
  deps.tabs.list = [];
  deps.tabs.createTab = (url) => {
    const id = deps.tabs.list.length + 1;
    deps.tabs.list.push({ id, url, home: true });
    return id;
  };
  deps.tabs.find = (id) => deps.tabs.list.find((tab) => tab.id === id);
  deps.tabs.activateTab = (id) => {
    deps.tabs.activeTabId = id;
  };
  return deps;
}
it('opens separate collection tabs and selects an existing one without duplicates', () => {
  const deps = fixture();
  showLibrary(deps, 'bookmarks');
  showLibrary(deps, 'history');
  showLibrary(deps, 'bookmarks');
  assert.equal(deps.tabs.list.length, 2);
  assert.equal(deps.tabs.activeTabId, 1);
  assert.deepEqual(
    deps.tabs.list.map((tab) => tab.title),
    ['Bookmarks', 'History'],
  );
});
it('returns data only for the selected collection in the opening profile', () => {
  const deps = fixture();
  showLibrary(deps, 'history');
  assert.equal(readLibrary(deps, 1).kind, 'history');
  deps.persona.partitionName = () => 'another-profile';
  assert.throws(() => readLibrary(deps, 1), /no longer available/);
});
it('refuses collection reads after a tab switch or ownership takeover', () => {
  const deps = fixture();
  showLibrary(deps, 'history');
  showLibrary(deps, 'bookmarks');
  assert.throws(() => readLibrary(deps, 1), /no longer available/);
  deps.control.snapshot = () => ({ interactive: false });
  assert.throws(() => readLibrary(deps, 2), /Human Mode/);
});
it('does not turn arbitrary renderer addresses into saved pages or navigation', () => {
  const deps = fixture();
  showLibrary(deps, 'bookmarks');
  assert.throws(() => changeLibrary(deps, 1, 'open', 'https://unknown.test/'), /no longer/);
  assert.throws(() => changeLibrary(deps, 1, 'bookmark', 'file:///etc/passwd'), /no longer/);
});
it('repeated saves are idempotent and opens select a regular tab', () => {
  const deps = fixture();
  deps.library.visit('https://saved.test/', 'Saved');
  showLibrary(deps, 'history');
  changeLibrary(deps, 1, 'bookmark', 'https://saved.test/');
  changeLibrary(deps, 1, 'bookmark', 'https://saved.test/');
  assert.equal(readLibrary(deps, 1).bookmarks.length, 1);
  changeLibrary(deps, 1, 'open', 'https://saved.test/');
  assert.equal(deps.tabs.list[1].url, 'https://saved.test/');
});
it('website navigation removes collection identity before exposing the page', () => {
  const deps = fixture();
  showLibrary(deps, 'history');
  const tab = deps.tabs.list[0];
  leaveHomeFor(tab, 'https://next.test/');
  assert.equal(tab.collection, undefined);
  assert.equal(tab.libraryPartition, undefined);
  assert.equal(tab.home, false);
});
it('agent-written blank-page content cannot retain a private collection marker', () => {
  const deps = fixture();
  showLibrary(deps, 'bookmarks');
  const tab = deps.tabs.list[0];
  assert.equal(contentReady(tab), true);
  assert.equal(tab.collection, undefined);
});

/** The Downloads tab over one saved download (`d1`) in the opening profile. */
function downloadsFixture() {
  const deps = fixture();
  const entry = {
    id: 'd1',
    url: 'https://f.test/a.pdf',
    title: 'a.pdf',
    time: 1,
    state: 'completed',
    received: 2,
    total: 2,
  };
  const calls: string[] = [];
  deps.downloads = {
    list: () => [entry],
    open: (id: string) => (calls.push(`open ${id}`), true),
    reveal: (id: string) => (calls.push(`reveal ${id}`), false),
    forget: (id: string) => void calls.push(`forget ${id}`),
  };
  showLibrary(deps, 'downloads');
  return { deps, calls };
}

it('opens a Downloads tab that lists the profile’s downloads', () => {
  const { deps } = downloadsFixture();
  assert.equal(deps.tabs.list[0].title, 'Downloads');
  const shown = readLibrary(deps, 1);
  assert.deepEqual([shown.kind, shown.downloads.map((d) => d.id)], ['downloads', ['d1']]);
});

it('opens, shows in its folder or forgets a listed download by id, and says when its file is gone', () => {
  const { deps, calls } = downloadsFixture();
  changeLibrary(deps, 1, 'open', 'd1');
  assert.throws(() => changeLibrary(deps, 1, 'reveal', 'd1'), /moved or deleted/);
  changeLibrary(deps, 1, 'remove', 'd1');
  assert.deepEqual(calls, ['open d1', 'reveal d1', 'forget d1']);
});

it('refuses a download id the profile does not list, and actions downloads do not have', () => {
  const { deps, calls } = downloadsFixture();
  assert.throws(() => changeLibrary(deps, 1, 'open', 'not-listed'), /no longer in your list/);
  assert.throws(() => changeLibrary(deps, 1, 'bookmark', 'd1'), /no longer in your list/);
  assert.deepEqual(calls, []);
});
