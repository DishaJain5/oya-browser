/**
 * Unit tests for lanes: the agent a browser command comes from rides along with
 * its work into every command, so an Oya desktop can give each agent its own tab.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { inLane, currentLane, laneFrom, LaneMap } from '../../../../../src/modules/browsers/connection/lane.ts';
import { describeCall } from '../../../../../src/modules/browsers/connection/reporter.ts';

it("stamps every command sent inside a lane's work with that lane, across awaits", async () => {
  const call = await inLane('agent-2', async () => {
    await new Promise((resolve) => setImmediate(resolve));
    return describeCall('b1', 'click', {}, 1000);
  });
  assert.equal(call.lane, 'agent-2');
  assert.equal(describeCall('b1', 'click', {}, 1000).lane, undefined, 'outside a lane, no lane');
});

it('keeps two agents apart when their work interleaves', async () => {
  const seen: (string | undefined)[] = [];
  const work = (lane: string) =>
    inLane(lane, async () => {
      await new Promise((resolve) => setImmediate(resolve));
      seen.push(currentLane());
    });
  await Promise.all([work('a'), work('b')]);
  assert.deepEqual(seen.sort(), ['a', 'b']);
});

it('runs a lane that is not a short name in no lane, so a caller cannot smuggle text into it', () => {
  for (const bad of ['', 'has space', 'x'.repeat(65), 42, { lane: 'a' }])
    assert.equal(
      inLane(bad, () => currentLane()),
      undefined,
    );
});

it("takes an external caller's lane from X-Oya-Agent, or ?agent= on its URL", () => {
  assert.equal(laneFrom({ headers: { 'x-oya-agent': 'research' } }), 'research');
  assert.equal(laneFrom({ headers: {}, query: { agent: 'mail' } }), 'mail');
});

it("keeps each agent's state on a browser apart, and gives a read outside a lane the latest", () => {
  const pages = new LaneMap<string>();
  inLane('tab-1', () => pages.set('b1', 'calendar'));
  inLane('tab-2', () => pages.set('b1', 'checkboxes'));
  assert.deepEqual(
    [inLane('tab-1', () => pages.get('b1')), inLane('tab-2', () => pages.get('b1')), pages.get('b1')],
    ['calendar', 'checkboxes', 'checkboxes'],
  );
  inLane('tab-2', () => pages.delete('b1'));
  assert.equal(
    inLane('tab-1', () => pages.get('b1')),
    'calendar',
    "one agent forgetting its page leaves the other's",
  );
  pages.delete('b1');
  assert.equal(
    inLane('tab-1', () => pages.get('b1')),
    undefined,
    'a browser that went takes every lane with it',
  );
});

it('keeps at most its cap of entries, the oldest going first', () => {
  const runs = new LaneMap<number>(2);
  for (const [lane, n] of [
    ['a', 1],
    ['b', 2],
    ['c', 3],
  ] as const)
    inLane(lane, () => runs.set('b1', n));
  assert.equal(
    inLane('a', () => runs.get('b1')),
    undefined,
  );
  assert.equal(
    inLane('c', () => runs.get('b1')),
    3,
  );
});
