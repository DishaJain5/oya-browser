/**
 * Unit tests for how an agent reply is drawn: an opening DONE: or FAILED:
 * becomes the verdict above the rest, the rest is the escaped Markdown, and an
 * error reply is told apart.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  awaitingAnswer,
  isErrorReply,
  questionOf,
  replyHtml,
} from '../../../../../../src/renderer/features/ask/model/reply.ts';

describe('replyHtml', () => {
  it('shows DONE: as the done verdict above the report', () => {
    assert.equal(
      replyHtml('DONE: **found** it'),
      '<span class="chat-verdict done">Done</span><p><strong>found</strong> it</p>',
    );
  });

  it('shows FAILED: as could not finish, whatever its case', () => {
    assert.match(
      replyHtml('failed: no stock'),
      /^<span class="chat-verdict failed">Could not finish<\/span><p>no stock<\/p>$/,
    );
  });

  it('escapes the reply, so a page cannot inject markup through it', () => {
    assert.equal(replyHtml('<img src=x onerror=alert(1)>'), '<p>&lt;img src=x onerror=alert(1)&gt;</p>');
  });

  it('tells an error reply apart', () => {
    assert.equal(isErrorReply('Error: boom'), true);
    assert.equal(isErrorReply('No error here'), false);
  });
});

describe('a reply where the agent stopped to ask', () => {
  it('gives the question without the NEEDS INPUT marker, and null for any other reply', () => {
    assert.equal(questionOf('NEEDS INPUT: Which size, M or L?'), 'Which size, M or L?');
    assert.equal(questionOf('DONE: Ordered.'), null);
  });

  it('draws the question under a Needs your answer pill, without the marker', () => {
    const html = replyHtml('NEEDS INPUT: Which **size**?');
    assert.match(html, /^<span class="chat-verdict asking">/);
    assert.match(html, /<strong>size<\/strong>/);
    assert.doesNotMatch(html, /NEEDS INPUT/);
  });

  it('waits on the person only while the question is the last entry', () => {
    const question = { role: 'assistant', content: 'NEEDS INPUT: Which size?' };
    assert.equal(awaitingAnswer([question]), true);
    assert.equal(awaitingAnswer([question, { role: 'user', content: 'M' }]), false);
    assert.equal(awaitingAnswer([{ role: 'assistant', content: 'DONE: ok' }]), false);
    assert.equal(awaitingAnswer([]), false);
  });
});
