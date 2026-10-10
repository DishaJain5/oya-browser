/** Native tool parity, capability discovery and command ownership are shared by the agent and MCP. */
import { it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { executeTool } from '../../../../src/modules/agent/executor.ts';
import { toolsOn } from '../../../../src/modules/agent/tools.ts';
import { NATIVE_TOOLS } from '../../../../src/modules/agent/native-tools.ts';
import { registerBrowserTools } from '../../../../src/mcp/browser-tools.ts';
import { scriptedOyaBrowser, scriptedBrowser } from '../../support/agent.ts';
import { FakeMcpServer, stubControl } from '../../support/browsers.ts';
let browser;
afterEach(() => {
  browser?.disconnect();
  mock.restoreAll();
});
it('offers native tools only when their actual commands are announced', () => {
  assert.deepEqual(toolsOn([]), []);
  assert.deepEqual(
    toolsOn(['start_trace']).map((t) => t.function.name),
    ['start_trace'],
  );
  assert.ok(!toolsOn(['analyze']).some((t) => t.function.name === 'drag'));
});
it('preserves diagnostic structure and input arguments through both public tool surfaces', async () => {
  stubControl();
  const data = { events: [{ kind: 'network', data: { phase: 'error' } }], dropped: 2 };
  browser = scriptedOyaBrowser('native-tools', 'key-a', () => ({ ok: true, data }));
  const server = new FakeMcpServer();
  registerBrowserTools(server as any, { pick: () => 'native-tools', oneBrowser: true });
  for (const tool of NATIVE_TOOLS) {
    const name = tool.function.name,
      args = { trace_id: 'trace', key: 'Shift+Tab' };
    assert.deepEqual(JSON.parse(await executeTool('native-tools', name, args)), data);
    assert.deepEqual(browser.calls.at(-1).params, args);
    assert.equal(server.tools.get(name).description, tool.function.description);
    assert.deepEqual(JSON.parse((await server.call(name, args)).content[0].text), data);
  }
});
it('native tools cannot bypass human control or claim generic provider support', async () => {
  stubControl({
    beginCommand: async () => {
      throw Error('Human holds control');
    },
  });
  browser = scriptedOyaBrowser('native-tools', 'key-a', () => ({ ok: true }));
  assert.equal(await executeTool('native-tools', 'start_trace', {}), 'Error: Human holds control');
  assert.equal(browser.calls.length, 0);
  browser.disconnect();
  mock.restoreAll();
  stubControl();
  browser = scriptedBrowser('native-tools', 'key-a', () => ({ ok: true }), 'cdp');
  const server = new FakeMcpServer();
  registerBrowserTools(server as any, { pick: () => 'native-tools', oneBrowser: true });
  for (const tool of NATIVE_TOOLS) assert.equal(server.tools.has(tool.function.name), false);
  assert.match(await executeTool('native-tools', 'start_trace', {}), /not supported/);
  assert.equal(browser.calls.length, 0);
});
