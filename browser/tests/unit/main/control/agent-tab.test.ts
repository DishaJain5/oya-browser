/**
 * Unit tests for per-tab agent control: the server's agent drives one tab, the
 * person keeps every other; nothing is fenced until the agent starts on a tab, and
 * local automation or another operator still takes the whole browser.
 */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { AgentTab, inLane } from '../../../../src/main/control/agent-tab.ts';
import { LANE_IDLE_MS } from '../../../../src/main/control/constants.ts';

/** The server's agent driving a connected desktop. */
const AGENT = { mode: 'agent', mine: false, interactive: false, connected: true, localClients: 0 };

/** An agent tab over tabs 1 and 2, the window showing `shown`, under control `state`. */
function setup(state: object = AGENT, shown = 1) {
  const views = { 1: { id: 'view-1' }, 2: { id: 'view-2' } };
  const tabs = {
    activeTabId: shown,
    created: [] as string[],
    find: (id: number) => (id in views ? { id, view: views[id as keyof typeof views] } : undefined),
    getActiveView: () => views[tabs.activeTabId as 1 | 2],
    createTab: (url: string, activate: boolean) => {
      const id = 3 + tabs.created.length;
      tabs.created.push(`${url}${activate ? ' shown' : ''}`);
      (views as Record<number, object>)[id] = { id: `view-${id}` };
      return id;
    },
  };
  const control = { state, snapshot: () => ({ ...control.state }) };
  let changes = 0;
  const agentTab = new AgentTab({ control, tabs } as any, () => changes++);
  return { agentTab, tabs, control, views, changes: () => changes };
}

it('fences nothing while the agent has no tab yet, though a connected desktop sits in agent mode', () => {
  const { agentTab } = setup();
  assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2), agentTab.humanMay(null)], [true, true, true]);
});

it("takes the shown tab on the agent's first command, then fences only that tab", () => {
  const { agentTab, views, changes } = setup();
  assert.equal(agentTab.drive(), views[1]);
  assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2), agentTab.humanMay(null)], [false, true, true]);
  assert.equal(changes(), 1, 'the windows hear the claim');
});

it('keeps driving its own tab while the person shows another', () => {
  const { agentTab, tabs, views } = setup();
  agentTab.drive();
  tabs.activeTabId = 2;
  assert.equal(agentTab.view(), views[1]);
  assert.equal(agentTab.following(), false);
  assert.equal(agentTab.tabId(), 1);
});

it("answers no page once the agent's tab is gone, never the person's", () => {
  const { agentTab, tabs } = setup();
  agentTab.claim(7);
  tabs.activeTabId = 2;
  assert.equal(agentTab.view(), null);
});

it('local automation and another operator still take the whole browser', () => {
  for (const state of [
    { ...AGENT, localClients: 1 },
    { mode: 'human', mine: false, interactive: false, connected: true, localClients: 0 },
  ]) {
    const { agentTab } = setup(state);
    agentTab.drive();
    assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2)], [false, false]);
  }
});

it('taking control ends the claim, and a run that ends lets the tab go', () => {
  const { agentTab, control } = setup();
  agentTab.drive();
  agentTab.release();
  assert.equal(agentTab.id, null);
  agentTab.drive();
  control.state = { ...AGENT, mode: 'human', mine: true, interactive: true };
  agentTab.sync(control.snapshot() as any);
  assert.equal(agentTab.id, null);
  assert.equal(agentTab.humanMay(1), true);
});

it("refuses the person on the agent's tab with the take-control message", () => {
  const { agentTab } = setup();
  agentTab.drive();
  assert.throws(() => agentTab.requireFor(1), /Take control/);
  assert.doesNotThrow(() => agentTab.requireFor(2));
});

it('gives each agent its own tab: a second one beside the first gets a fresh background tab, the person keeps the rest', () => {
  const { agentTab, tabs } = setup();
  inLane('agent-1', () => agentTab.drive());
  tabs.activeTabId = 2; // the person turns to their own tab
  inLane('agent-2', () => agentTab.drive());
  assert.deepEqual(tabs.created, ['about:blank'], 'opened in the background, never on the person');
  assert.deepEqual([inLane('agent-1', () => agentTab.id), inLane('agent-2', () => agentTab.id)], [1, 3]);
  assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2), agentTab.humanMay(3)], [false, true, false]);
  assert.equal(
    inLane('agent-2', () => agentTab.othersDrive(1)),
    true,
  );
  assert.equal(
    inLane('agent-1', () => agentTab.othersDrive(1)),
    false,
  );
});

it("ends one agent's run without touching another's tab", () => {
  const { agentTab, tabs } = setup();
  inLane('agent-1', () => agentTab.drive());
  tabs.activeTabId = 2;
  inLane('agent-2', () => agentTab.drive());
  inLane('agent-1', () => agentTab.release());
  assert.deepEqual(agentTab.claimed(), [3]);
  assert.equal(agentTab.humanMay(1), true);
});

it("gives a quiet agent's tab back to the person: an API or MCP agent never says it finished", () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  try {
    const { agentTab } = setup();
    inLane('mcp-agent', () => agentTab.drive());
    mock.timers.tick(LANE_IDLE_MS - 1);
    inLane('mcp-agent', () => agentTab.drive()); // still working
    mock.timers.tick(LANE_IDLE_MS - 1);
    assert.deepEqual(agentTab.claimed(), [1]);
    mock.timers.tick(1);
    assert.deepEqual(agentTab.claimed(), []);
    assert.equal(agentTab.humanMay(1), true);
  } finally {
    mock.timers.reset();
  }
});

it("a link the agent's page opens in a new tab stays the agent's, and the old tab is the person's again", () => {
  const { agentTab } = setup();
  agentTab.drive();
  agentTab.handOff(1, 2);
  assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2)], [true, false]);
  agentTab.handOff(1, 3);
  assert.deepEqual(agentTab.claimed(), [2], 'a tab no agent drives hands nothing on');
});
