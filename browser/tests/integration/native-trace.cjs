/** Real native observation covers redirect correlation and console ordering without replacing egress hooks. */
const assert = require('node:assert/strict');
const { createServer } = require('node:http');
const { Observer, watchSession, watchContents } = require('../../src/main/observe/index.ts');
module.exports = async function checkNativeTrace(win) {
  const server = createServer((req, res) => {
    if (req.url.startsWith('/redirect')) {
      res.writeHead(302, { location: '/result?private=two' });
      return res.end();
    }
    if (req.url.startsWith('/result')) {
      res.writeHead(418);
      return res.end('fixture rejection');
    }
    res.end('<!doctype html><title>Native trace</title>Trace fixture');
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  const observer = new Observer(),
    contents = win.webContents;
  let governanceCalls = 0;
  contents.session.webRequest.onBeforeRequest((details, reply) => {
    governanceCalls++;
    reply({ cancel: !details.url.startsWith(origin) });
  });
  watchSession(observer, contents.session);
  watchContents(observer, contents);
  const { trace_id } = observer.traces.start(contents.id, () => true);
  try {
    await win.loadURL(origin);
    await contents.executeJavaScript(
      'console.warn("trace-start"); fetch("/redirect?private=one").then(r => r.text()).then(() => console.info("trace-end"))',
    );
    await new Promise((resolve) => setTimeout(resolve, 200));
    const result = observer.traces.read(contents.id, { trace_id, limit: 1000 }, true);
    const network = result.events.filter((e) => e.kind === 'network').map((e) => e.data);
    const redirect = network.find((e) => e.phase === 'redirect');
    assert.ok(redirect, 'native redirect observed');
    const complete = network.find((e) => e.request_id === redirect.request_id && e.phase === 'complete');
    assert.equal(complete.status, 418);
    assert.ok(complete.elapsed_ms >= 0);
    assert.ok(result.events.some((e) => e.kind === 'console' && e.data.message === 'trace-start'));
    assert.ok(!JSON.stringify(network).includes('private='));
    assert.ok(governanceCalls > 0);
    assert.ok(observer.readNetwork({ failedOnly: true }).some((e) => e.status === 418));
    console.log(
      'PASS: native network/console tracing, redirects, elapsed timing, URL redaction and egress coexistence',
    );
  } finally {
    contents.session.webRequest.onBeforeRequest(null);
    await new Promise((resolve) => server.close(resolve));
  }
};
