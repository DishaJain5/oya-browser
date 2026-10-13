/** Structured plan refusals become useful account actions instead of raw error paragraphs. */
import type { ChatAnswer } from './types.ts';
/** Fixed, trusted copy never turns a server-supplied URL into an external action. */
export interface PlanNotice {
  /** Small context label above the heading. */
  label: string;
  /** What prevented this task from continuing. */
  title: string;
  /** Plain-language explanation that avoids claiming a task completed. */
  body: string;
  /** Clearly named destination for the primary action. */
  action: string;
  /** Optional exact usage from the server, never inferred from a run's step count. */
  usage?: string;
}
/** Supported refusal codes, with distinct copy for payment and capacity. */
const NOTICES: Record<string, PlanNotice> = {
  plan_limit: {
    label: 'PLAN ALLOWANCE',
    title: 'You’ve reached your plan’s limit',
    body: 'This task stopped because your allowance for this billing period is used up. Review your usage or choose a plan with more room.',
    action: 'View plans & usage',
  },
  plan_concurrency: {
    label: 'BROWSER CAPACITY',
    title: 'All your cloud browser slots are in use',
    body: 'Stop a cloud browser you no longer need, or choose a plan with more concurrent browsers. Then try your task again.',
    action: 'Manage plan',
  },
  payment_required: {
    label: 'PAYMENT NEEDED',
    title: 'Your payment needs attention',
    body: 'Your last payment did not go through. Review your billing details before starting another task.',
    action: 'Review billing',
  },
};
/** Only explicit server codes activate billing UI; arbitrary agent text remains ordinary text. */
export function planNotice(data: ChatAnswer): PlanNotice | undefined {
  if (!data.code || !Object.hasOwn(NOTICES, data.code)) return undefined;
  const notice = { ...NOTICES[data.code] };
  if (data.resource === 'agent_steps') notice.title = 'Your agent steps are used up';
  if (typeof data.used === 'number' && typeof data.limit === 'number' && data.used >= 0 && data.limit >= 0)
    notice.usage = `${data.used.toLocaleString()} / ${data.limit.toLocaleString()}${data.resource === 'agent_steps' ? ' steps' : ''}`;
  if (['free', 'developer', 'startup'].includes(data.plan ?? '')) notice.label = `${data.plan} PLAN`.toUpperCase();
  return notice;
}
