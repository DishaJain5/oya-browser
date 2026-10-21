/**
 * The definition editor: a plain textarea over the same text highlighted by the
 * console's Prism renderer, so typing stays native (undo, selection, paste) and
 * the JSON still reads in colour. Line numbers run down the side.
 * ponytail: textarea and overlay; move to CodeMirror 6 if people need autocomplete or folding.
 */
'use client';

import { useRef } from 'react';
import SyntaxCode from '@/components/ui/syntax-code';
import { EDITOR_TEXT } from './editor-text';

/** Props for the field. */
interface Props {
  /** The text. */
  value: string;
  /** Changes it. */
  onChange: (text: string) => void;
  /** Why the text is not a definition, shown under it. */
  error: string | null;
}

/** Shared box metrics, so the overlay lines up with the textarea character for character. */
const BOX = 'm-0 whitespace-pre p-3 font-mono text-[12px] leading-[18px]';

/** Moves the highlighted text and the line numbers to where the textarea has scrolled. */
function scrollWith(el: HTMLTextAreaElement, behind: HTMLPreElement | null, gutter: HTMLPreElement | null) {
  if (behind) [behind.scrollTop, behind.scrollLeft] = [el.scrollTop, el.scrollLeft];
  if (gutter) gutter.scrollTop = el.scrollTop;
}

/** The textarea, its highlighted twin behind it, and the line numbers. */
export default function CodeField({ value, onChange, error }: Props) {
  const behind = useRef<HTMLPreElement>(null);
  const gutter = useRef<HTMLPreElement>(null);
  const lines = value.split('\n').length;
  const sync = (el: HTMLTextAreaElement) => scrollWith(el, behind.current, gutter.current);
  return (
    <div>
      <div className="flex h-[52dvh] overflow-hidden rounded-lg border border-border bg-bg">
        <pre
          ref={gutter}
          aria-hidden
          className={`${BOX} select-none overflow-hidden border-r border-border text-right text-text-dim`}
        >
          {Array.from({ length: lines }, (_, i) => i + 1).join('\n')}
        </pre>
        <div className="relative min-w-0 flex-1">
          <pre ref={behind} aria-hidden className={`${BOX} absolute inset-0 overflow-hidden`}>
            <SyntaxCode code={`${value}\n`} language="json" />
          </pre>
          <textarea
            aria-label={EDITOR_TEXT.definitionLabel}
            className={`${BOX} absolute inset-0 h-full w-full resize-none overflow-auto bg-transparent text-transparent caret-text outline-none`}
            spellCheck={false}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onScroll={(e) => sync(e.currentTarget)}
            aria-invalid={Boolean(error)}
            aria-describedby="definition-error"
          />
        </div>
      </div>
      <p id="definition-error" role={error ? 'alert' : undefined} className="mt-2 min-h-[18px] text-[12px] text-red">
        {error ?? ''}
      </p>
    </div>
  );
}
