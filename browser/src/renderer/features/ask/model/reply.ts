/**
 * Pure helpers for the agent's replies: the reply's HTML with its verdict,
 * whether a reply is an error, and what Save as playbook offers.
 */
import { markdownToHtml } from '../../../core/markdown.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import { ASK_TEXT, VERDICT, NEEDS_INPUT, REPLAYABLE_TOOLS } from './constants.ts';
import type { ToolCall } from './types.ts';

/** The verdict above a report: done in Oya's green, or could not finish in amber. */
export function verdictHtml(word: string): string {
  const done = word.toUpperCase() === 'DONE';
  return `<span class="chat-verdict ${done ? 'done' : 'failed'}">${done ? ASK_TEXT.done : ASK_TEXT.failed}</span>`;
}

/** The pill on a reply where the agent stopped to ask: "Needs your answer", or "Answered" once the person replied (ask.css shows one). */
const ASKING_HTML =
  `<span class="chat-verdict asking"><span class="when-waiting">${ASK_TEXT.needsInput}</span>` +
  `<span class="when-answered">${ASK_TEXT.answered}</span></span>`;

/** An agent message as HTML: an opening DONE:, FAILED: or NEEDS INPUT: becomes its pill, above the escaped Markdown. */
export function replyHtml(content: string): string {
  const question = questionOf(content);
  if (question !== null) return ASKING_HTML + markdownToHtml(question);
  const verdict = VERDICT.exec(content);
  const body = markdownToHtml(verdict ? content.slice(verdict[0].length) : content);
  return verdict ? verdictHtml(verdict[1]) + body : body;
}

/** The question in a reply where the agent stopped to ask the person, or null for any other reply. */
export function questionOf(content: string): string | null {
  const match = NEEDS_INPUT.exec(content);
  return match ? content.slice(match[0].length) : null;
}

/** One entry of the conversation, as far as asking whether it waits goes. */
interface Entry {
  /** Who said it, for a message. */
  role?: unknown;
  /** What was said. */
  content?: unknown;
}

/** Whether the conversation is waiting on the person: its last entry is the agent's question. */
export function awaitingAnswer(items: readonly unknown[]): boolean {
  const last = items.at(-1) as Entry | undefined;
  return last?.role === 'assistant' && typeof last.content === 'string' && questionOf(last.content) !== null;
}

/** Whether an agent message is an error shown as its reply. */
export const isErrorReply = (content: string): boolean => content.startsWith(ASK_TEXT.errorPrefix.trim());

/**
 * Whether a run did something a playbook can replay. Navigating alone is not
 * enough, the server refuses it: a run that only visited and read pages would
 * replay as page loads that produce nothing, since reading needs the model.
 */
export const canReplay = (toolCalls: readonly ToolCall[]): boolean =>
  toolCalls.some((c) => c.name !== 'navigate' && REPLAYABLE_TOOLS.has(c.name));

/** A playbook name from the prompt: lowercase words joined by hyphens. */
export function suggestName(prompt: string): string {
  const slug = prompt.toLowerCase().replace(/[^a-z0-9]+/g, '-');
  return slug.slice(0, C.PLAYBOOK_NAME_SUGGESTION).replace(/^-+|-+$/g, '') || ASK_TEXT.defaultPlaybook;
}
