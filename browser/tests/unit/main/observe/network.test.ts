/** Native traces preserve real lifecycle identity while excluding sensitive URL fields and other sessions. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { Observer } from '../../../../src/main/observe/observer.ts';
import { NetworkTrace } from '../../../../src/main/observe/network.ts';
const details = {
  id: 1,
  webContentsId: 7,
  timestamp: 100,
  url: 'https://u:p@a.test/start?token=secret#x',
  method: 'GET',
  resourceType: 'xhr',
};
it('correlates redirects, native response status and time since headers', () => {
  const observer = new Observer(),
    network = new NetworkTrace(observer);
  const { trace_id } = observer.traces.start(7, () => true);
  network.event('request', details);
  network.event('redirect', {
    ...details,
    timestamp: 120,
    statusCode: 302,
    redirectURL: 'https://a.test/end?secret=x',
  });
  network.event('complete', { ...details, timestamp: 150, statusCode: 200 });
  const data = observer.traces.read(7, { trace_id }).events.map((e) => e.data as any);
  assert.equal(new Set(data.map((d) => d.request_id)).size, 1);
  assert.equal(data[1].redirect_url, 'https://a.test/end');
  assert.equal(data[2].elapsed_ms, 50);
  assert.ok(!JSON.stringify(data).includes('secret'));
  assert.equal(data[0].url, 'https://a.test/start');
});
it('does not guess start times or share native request ids across sessions', () => {
  const observer = new Observer(),
    first = new NetworkTrace(observer),
    second = new NetworkTrace(observer);
  const { trace_id } = observer.traces.start(7, () => true);
  first.event('error', { ...details, error: 'net::ERR_BLOCKED_BY_CLIENT' });
  second.event('request', details);
  const data = observer.traces.read(7, { trace_id }).events.map((e) => e.data as any);
  assert.equal(data[0].elapsed_ms, null);
  assert.equal(data[0].started_at, null);
  assert.notEqual(data[0].request_id, data[1].request_id);
  assert.equal(data[0].error, 'net::ERR_BLOCKED_BY_CLIENT');
});
