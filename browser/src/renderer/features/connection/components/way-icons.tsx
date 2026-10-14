/**
 * The marks on the checklist's ways in: Oya's O, the Gemini and ChatGPT
 * logos so a person spots the plan they already have, and the shell's key icon.
 */
import type { ReactNode } from 'react';
import { Icon } from '../../../ui/index.ts';
import type { ConnectWay } from '../view-models/checklist-view-model.ts';

/** Oya's O on its own: the logo's green ring, without the orb around it. */
const OYA = (
  <svg className="way-icon" viewBox="0 0 24 24" aria-hidden="true">
    <circle cx="12" cy="12" r="7.5" fill="none" stroke="var(--accent)" strokeWidth="5" />
  </svg>
);

/** Gemini's four-point spark, in its blue-to-violet gradient. */
const GEMINI = (
  <svg className="way-icon" viewBox="0 0 24 24" aria-hidden="true">
    <defs>
      <linearGradient id="way-gemini" x1="0" y1="1" x2="1" y2="0">
        <stop offset="0" stopColor="#4285f4" />
        <stop offset="1" stopColor="#b388ff" />
      </linearGradient>
    </defs>
    <path
      fill="url(#way-gemini)"
      d="M12 2c0 5.5 4.5 10 10 10-5.5 0-10 4.5-10 10 0-5.5-4.5-10-10-10 5.5 0 10-4.5 10-10Z"
    />
  </svg>
);

/** OpenAI's knot, in the text colour. */
const CHATGPT = (
  <svg className="way-icon" viewBox="0 0 24 24" aria-hidden="true">
    <path
      fill="currentColor"
      d="M22.28 9.82a5.98 5.98 0 0 0-.52-4.91 6.05 6.05 0 0 0-6.51-2.9A6.07 6.07 0 0 0 4.98 4.18a5.98 5.98 0 0 0-4 2.9 6.05 6.05 0 0 0 .74 7.1 5.98 5.98 0 0 0 .51 4.91 6.05 6.05 0 0 0 6.52 2.9A5.98 5.98 0 0 0 13.26 24a6.06 6.06 0 0 0 5.77-4.21 5.99 5.99 0 0 0 4-2.9 6.06 6.06 0 0 0-.75-7.07Zm-9.02 12.61a4.48 4.48 0 0 1-2.88-1.04l.14-.08 4.78-2.76a.79.79 0 0 0 .39-.68v-6.74l2.02 1.17a.07.07 0 0 1 .04.05v5.58a4.5 4.5 0 0 1-4.49 4.5Zm-9.66-4.13a4.47 4.47 0 0 1-.53-3.01l.14.09 4.78 2.76a.77.77 0 0 0 .78 0l5.84-3.37v2.33a.08.08 0 0 1-.03.06L9.74 19.95a4.5 4.5 0 0 1-6.14-1.65ZM2.34 7.9a4.49 4.49 0 0 1 2.37-1.97v5.67a.77.77 0 0 0 .39.68l5.81 3.35-2.02 1.17a.08.08 0 0 1-.07 0l-4.83-2.79A4.5 4.5 0 0 1 2.34 7.87Zm16.6 3.86-5.84-3.39 2.02-1.16a.08.08 0 0 1 .07 0l4.83 2.79a4.49 4.49 0 0 1-.68 8.1v-5.68a.79.79 0 0 0-.4-.66Zm2.01-3.03-.14-.09-4.77-2.78a.78.78 0 0 0-.79 0L9.41 9.23V6.9a.07.07 0 0 1 .03-.06l4.83-2.79a4.5 4.5 0 0 1 6.68 4.66ZM8.31 12.86l-2.02-1.16a.08.08 0 0 1-.04-.06V6.07a4.5 4.5 0 0 1 7.38-3.45l-.14.08L8.7 5.46a.79.79 0 0 0-.39.68Zm1.1-2.37 2.6-1.5 2.6 1.5v3l-2.6 1.5-2.6-1.5Z"
    />
  </svg>
);

/** Each way's mark. */
export const WAY_ICONS: Record<ConnectWay, ReactNode> = {
  oya: OYA,
  gemini: GEMINI,
  chatgpt: CHATGPT,
  key: <Icon name="key" />,
};
