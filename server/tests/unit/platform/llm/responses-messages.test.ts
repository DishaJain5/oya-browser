/**
 * Unit tests for the chat-completions ⇄ Responses API translation: system prompts
 * become instructions, a tool call and its result become paired items, images and
 * tools keep their meaning, and a completed response comes back as one assistant
 * message with text, tool calls and usage.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  fromResponses,
  inputOf,
  instructionsOf,
  toolChoiceOf,
  toolsOf,
} from '../../../../src/platform/llm/responses-messages.ts';

const CONVERSATION = [
  { role: 'system', content: 'Be careful.' },
  { role: 'system', content: 'Use tools.' },
  {
    role: 'user',
    content: [
      { type: 'text', text: 'Look' },
      { type: 'image_url', image_url: { url: 'data:image/png;base64,AA' } },
    ],
  },
  {
    role: 'assistant',
    content: 'Opening it',
    tool_calls: [{ id: 'c1', type: 'function', function: { name: 'navigate', arguments: '{"url":"x"}' } }],
  },
  { role: 'tool', tool_call_id: 'c1', content: 'done' },
];

describe('Responses translation', () => {
  it('joins every system prompt into the instructions', () => {
    assert.equal(instructionsOf(CONVERSATION), 'Be careful.\n\nUse tools.');
    assert.equal(instructionsOf([{ role: 'user', content: 'hi' }]), undefined);
  });

  it('turns messages into input items, pairing a tool call with its result by call id', () => {
    assert.deepEqual(inputOf(CONVERSATION), [
      {
        role: 'user',
        content: [
          { type: 'input_text', text: 'Look' },
          { type: 'input_image', image_url: 'data:image/png;base64,AA' },
        ],
      },
      { role: 'assistant', content: [{ type: 'output_text', text: 'Opening it' }] },
      { type: 'function_call', call_id: 'c1', name: 'navigate', arguments: '{"url":"x"}' },
      { type: 'function_call_output', call_id: 'c1', output: 'done' },
    ]);
  });

  it('flattens chat tools and a named tool choice', () => {
    const tools = [{ type: 'function', function: { name: 'go', description: 'Go', parameters: { type: 'object' } } }];
    assert.deepEqual(toolsOf(tools), [
      { type: 'function', name: 'go', description: 'Go', parameters: { type: 'object' } },
    ]);
    assert.deepEqual(toolChoiceOf({ type: 'function', function: { name: 'go' } }), { type: 'function', name: 'go' });
    assert.equal(toolChoiceOf(undefined), 'auto');
  });

  it('reads a completed response back as text, tool calls and usage', () => {
    const completion = fromResponses({
      output: [
        { type: 'reasoning', summary: [] },
        { type: 'message', content: [{ type: 'output_text', text: 'On it' }] },
        { type: 'function_call', call_id: 'c2', name: 'click', arguments: '{"id":3}' },
      ],
      usage: { input_tokens: 10, output_tokens: 4 },
    });
    assert.deepEqual(completion.choices[0].message, {
      role: 'assistant',
      content: 'On it',
      tool_calls: [{ id: 'c2', type: 'function', function: { name: 'click', arguments: '{"id":3}' } }],
    });
    assert.deepEqual(completion.usage, { prompt_tokens: 10, completion_tokens: 4 });
  });
});
