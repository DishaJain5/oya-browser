/**
 * The shared OpenAI chat-completions shape to and from OpenAI's Responses API, which
 * is the only API a ChatGPT plan answers on. System prompts become `instructions`,
 * each message becomes input items (a tool call and its result are items of their
 * own, paired by call id), and the completed response's output items come back as
 * one assistant message with text and tool calls.
 */

/** OpenAI user or assistant content, a string or parts, as Responses content parts of `textType`. */
function partsOf(content: any, textType: 'input_text' | 'output_text'): any[] {
  if (!Array.isArray(content)) return content ? [{ type: textType, text: String(content) }] : [];
  return content.flatMap((part) =>
    part.type === 'image_url'
      ? [{ type: 'input_image', image_url: part.image_url?.url }]
      : partsOf(part.text, textType),
  );
}

/** A chat tool call as a function_call item. */
const callItem = (call: any) => ({
  type: 'function_call',
  call_id: call.id,
  name: call.function?.name,
  arguments: call.function?.arguments || '{}',
});

/** An assistant turn: its text as a message item, then one function_call item per tool call. */
function assistantItems(message: any): any[] {
  const text = partsOf(message.content, 'output_text');
  const said = text.length ? [{ role: 'assistant', content: text }] : [];
  return [...said, ...(message.tool_calls || []).map(callItem)];
}

/** A tool result as the function_call_output its call id pairs it with. */
const toolOutput = (message: any) => ({
  type: 'function_call_output',
  call_id: message.tool_call_id,
  output: typeof message.content === 'string' ? message.content : JSON.stringify(message.content ?? ''),
});

/** One non-system message as Responses input items. */
function itemsOf(message: any): any[] {
  if (message.role === 'assistant') return assistantItems(message);
  if (message.role === 'tool') return [toolOutput(message)];
  const content = partsOf(message.content, 'input_text');
  return content.length ? [{ role: 'user', content }] : [];
}

/** The system prompts, joined, as the request's instructions; undefined when there are none. */
export function instructionsOf(messages: any[]) {
  const text = messages
    .filter((m) => m.role === 'system' && m.content)
    .map((m) => m.content)
    .join('\n\n');
  return text || undefined;
}

/** Every other message as input items, in order. */
export const inputOf = (messages: any[]) => messages.filter((m) => m.role !== 'system').flatMap(itemsOf);

/** OpenAI chat tool definitions as Responses function tools, which are flat. */
export const toolsOf = (tools: any[] = []) =>
  tools.map((t) => ({
    type: 'function',
    name: t.function.name,
    description: t.function.description,
    parameters: t.function.parameters || { type: 'object', properties: {} },
  }));

/** A chat tool_choice as Responses spells it: a named function loses its `function` wrapper. */
export const toolChoiceOf = (choice: any) =>
  choice?.type === 'function' ? { type: 'function', name: choice.function?.name } : choice || 'auto';

/** The text of a completed response's message items. */
const textOf = (output: any[]) =>
  output
    .filter((item) => item.type === 'message')
    .flatMap((item) => item.content || [])
    .filter((part) => part.type === 'output_text')
    .map((part) => part.text)
    .join('');

/** A completed response's function calls as chat tool calls. */
const toolCallsOf = (output: any[]) =>
  output
    .filter((item) => item.type === 'function_call')
    .map((item) => ({ id: item.call_id, type: 'function', function: { name: item.name, arguments: item.arguments } }));

/** A response's token counts as chat usage, when it reported them. */
const usageOf = (usage: any) =>
  usage ? { prompt_tokens: usage.input_tokens ?? 0, completion_tokens: usage.output_tokens ?? 0 } : undefined;

/** A completed Responses response as a chat completion. */
export function fromResponses(response: any) {
  const output = response?.output || [];
  const calls = toolCallsOf(output);
  const message = {
    role: 'assistant',
    content: textOf(output) || null,
    ...(calls.length ? { tool_calls: calls } : {}),
  };
  return { choices: [{ message }], usage: usageOf(response?.usage) };
}
