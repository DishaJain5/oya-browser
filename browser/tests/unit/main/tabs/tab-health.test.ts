/**
 * Unit tests for the tab health log: what each entry keeps (and leaves out),
 * how many it keeps, and that a bad file never stops it.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { TabHealthLog } from '../../../../src/main/tabs/tab-health.ts';
import { TAB_HEALTH_ENTRIES } from '../../../../src/main/tabs/constants.ts';
import { ownSession } from '../../../../src/main/native-contexts/registry.ts';

/** A JsonFile held in memory. */
function memoryFile(initial = null) {
  return {
    value: initial,
    read() {
      return this.value;
    },
    write(value) {
      this.value = value;
    },
  };
}

/** A tab whose page is at `url` in renderer `pid`. */
function tabAt(id, url, pid, session = {}) {
  const webContents = { getURL: () => url, getOSProcessId: () => pid, isDestroyed: () => false, session };
  return { id, view: { webContents } };
}

describe('TabHealthLog', () => {
  let file, tabs, claimed, log;
  beforeEach(() => {
    file = memoryFile();
    tabs = [];
    claimed = [];
    log = new TabHealthLog({ tabs: { list: tabs }, agentTab: { claimed: () => claimed } }, file);
  });

  it("records when a hang was detected, the page's site, its process, whether an agent held it, and the tabs sharing it", () => {
    const hung = tabAt(1, 'https://www.youtube.com/watch?v=x&token=secret', 42);
    tabs.push(hung, tabAt(2, 'https://www.youtube.com/', 42), tabAt(3, 'https://other.test/', 7));
    claimed.push(2);
    log.record(hung, 'probe-failed');
    const [entry] = file.value;
    assert.match(entry.detectedAt, /^\d{4}-\d\d-\d\dT/);
    const { detectedAt: _at, ...rest } = entry;
    assert.deepEqual(rest, {
      event: 'probe-failed',
      tab: 1,
      origin: 'https://www.youtube.com',
      pid: 42,
      agent: false,
      peers: [2],
    });
  });

  it('keeps only the newest entries', () => {
    const tab = tabAt(1, 'https://a.test/', 1);
    for (let i = 0; i < TAB_HEALTH_ENTRIES + 5; i++) log.record(tab, 'responsive');
    assert.equal(file.value.length, TAB_HEALTH_ENTRIES);
  });

  it('starts over from a corrupt file', () => {
    file.value = { not: 'a list' };
    log.record(tabAt(1, 'https://a.test/', 1), 'gone', 'killed');
    assert.equal(file.value.length, 1);
    assert.equal(file.value[0].reason, 'killed');
  });

  it("never writes a private tab's address", () => {
    const session = {};
    ownSession(session, {}, 'private-1');
    const priv = tabAt(1, 'https://secret.test/', 1, session);
    log.record(priv, 'probe-failed');
    assert.equal(file.value[0].origin, '');
  });

  it('records a helper process that went away, by name and reason', () => {
    log.processGone({ type: 'GPU', name: undefined, reason: 'crashed', exitCode: 1 });
    log.processGone({ type: 'Utility', name: 'Audio Service', reason: 'killed', exitCode: 9 });
    assert.deepEqual(
      file.value.map((e) => [e.event, e.process, e.reason]),
      [
        ['process-gone', 'GPU', 'crashed'],
        ['process-gone', 'Audio Service', 'killed'],
      ],
    );
  });
});
