/**
 * Unit tests for closing a tab opened only to download a file: a tab that never
 * showed a page closes, one that showed a page, the start page, or an unknown
 * page is left alone.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { closeDownloadOnlyTab, personMayUse } from '../../../../src/main/downloads/index.ts';

/** A tab whose page is at `url`, with or without a page before it. */
function setup(url: string, canGoBack = false, home = false) {
  const contents = {
    getURL: () => url,
    isDestroyed: () => false,
    navigationHistory: { canGoBack: () => canGoBack },
  };
  const closed: number[] = [];
  const tabs = { list: [{ id: 7, home, view: { webContents: contents } }], closeTab: (id: number) => closed.push(id) };
  return { deps: { tabs } as any, contents: contents as any, closed };
}

it('closes a tab that never showed a page once its download starts', () => {
  const { deps, contents, closed } = setup('about:blank');
  closeDownloadOnlyTab(deps, contents);
  assert.deepEqual(closed, [7]);
});

it('leaves a tab that shows a page, or had one before, or is the start page', () => {
  for (const [url, back, home] of [
    ['https://site.test/', false, false],
    ['about:blank', true, false],
    ['', false, true],
  ] as const) {
    const { deps, contents, closed } = setup(url, back, home);
    closeDownloadOnlyTab(deps, contents);
    assert.deepEqual(closed, [], `${url} back=${back} home=${home}`);
  }
});

it('ignores a download from a page that is not a tab', () => {
  const { deps, closed } = setup('about:blank');
  closeDownloadOnlyTab(deps, { getURL: () => '' } as any);
  assert.deepEqual(closed, []);
});

it('the person may use a tab no agent drives, and not one an agent does', () => {
  const { deps, contents } = setup('https://site.test/');
  deps.agentTab = { humanMay: (id: number | null) => id !== 7 };
  assert.equal(personMayUse(deps, contents), false);
  deps.agentTab = { humanMay: () => true };
  assert.equal(personMayUse(deps, contents), true);
});
