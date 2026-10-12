/** The footer's coding agents: which are listed, connecting one, and saying plainly what happened. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { AgentsViewModel } from '../../../../../../src/renderer/features/connection/view-models/agents-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** Lets pending promises settle. */
const settle = () => new Promise((resolve) => setImmediate(resolve));
const claude = { id: 'claude-code', name: 'Claude Code', connected: false };

it('lists the agents this computer has', async () => {
  const vm = new AgentsViewModel(fakeBridge({ localAgents: [claude] }).bridge);
  await settle();
  assert.deepEqual(vm.state.agents, [claude]);
});

it('connects an agent and says to restart it', async () => {
  const fake = fakeBridge({ localAgents: [claude], connectAgent: [{ ...claude, connected: true }] });
  const vm = new AgentsViewModel(fake.bridge);
  await settle();
  await vm.toggle('claude-code');
  assert.deepEqual(fake.called('connectAgent'), [['claude-code']]);
  assert.equal(vm.state.agents[0].connected, true);
  assert.match(vm.state.note, /Restart Claude Code/);
  assert.equal(vm.state.busy, '');
});

it('tells a Cursor user to allow Oya’s tools once, since Oya cannot do that for it', async () => {
  const cursor = { id: 'cursor', name: 'Cursor', connected: false };
  const fake = fakeBridge({ localAgents: [cursor], connectAgent: [{ ...cursor, connected: true }] });
  const vm = new AgentsViewModel(fake.bridge);
  await settle();
  await vm.toggle('cursor');
  assert.match(vm.state.note, /Always allow/);
});

it('disconnects a connected agent', async () => {
  const on = { ...claude, connected: true };
  const fake = fakeBridge({ localAgents: [on], disconnectAgent: [claude] });
  const vm = new AgentsViewModel(fake.bridge);
  await settle();
  await vm.toggle('claude-code');
  assert.deepEqual(fake.called('disconnectAgent'), [['claude-code']]);
  assert.match(vm.state.note, /removed/);
});

it('says why a connection failed and keeps the list as it was', async () => {
  const fake = fakeBridge({
    localAgents: [claude],
    connectAgent: () => Promise.reject(new Error('has comments; add Oya there by hand.')),
  });
  const vm = new AgentsViewModel(fake.bridge);
  await settle();
  await vm.toggle('claude-code');
  assert.match(vm.state.note, /by hand/);
  assert.equal(vm.state.agents[0].connected, false);
});

it('shows nothing when this browser offers no MCP port', async () => {
  const vm = new AgentsViewModel(fakeBridge({ localAgents: [] }).bridge);
  await settle();
  assert.deepEqual(vm.state.agents, []);
});
