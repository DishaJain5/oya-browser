/**
 * How a row's body maps onto its indexed columns: project, the indexed `state`
 * field, and when the pruner may delete it.
 */
import { DELIVERY_TTL_MS, IDEMPOTENCY_TTL_MS, INSTANCE_TTL_MS, TERMINAL_SESSION_TTL_MS } from './constants.ts';
import { CLIENT_TTL_MS } from '../../../mcp/oauth/constants.ts';
import { MS_PER_SECOND } from '../../../platform/constants.ts';

/** States a session never leaves. */
const TERMINAL = ['stopped', 'failed'];
/** Which body field each kind indexes in the `state` column. */
const STATE_FIELD = {
  credential: 'role',
  hold: 'resource',
  attachment: 'instance',
  membership: 'userId',
  project: 'ownerUser',
  recording: 'owner',
};
/** Rows that expire at their own `expiresAt`. */
const ownExpiry = (body) => body.expiresAt;
/** When each kind of row may be pruned; kinds not listed are kept. */
const EXPIRY: Record<string, (body) => number | null> = {
  session: (body) =>
    TERMINAL.includes(body.state) ? (body.updatedAt || body.createdAt) + TERMINAL_SESSION_TTL_MS : null,
  ticket: ownExpiry,
  invite: ownExpiry,
  hold: ownExpiry,
  slack_state: ownExpiry,
  chatgpt_login: ownExpiry,
  oauth_request: ownExpiry,
  oauth_code: ownExpiry,
  // An MCP client (claude.ai) registers itself; one nobody has used in a while is dropped and registers again.
  oauth_client: (body) => (body.lastUsedAt || (body.client_id_issued_at || 0) * MS_PER_SECOND) + CLIENT_TTL_MS,
  attachment: (body) => body.leaseUntil,
  idempotency: (body) => body.createdAt + IDEMPOTENCY_TTL_MS,
  delivery: (body) => (body.state === 'pending' ? null : body.at + DELIVERY_TTL_MS),
  instance: (body) => body.leaseUntil + INSTANCE_TTL_MS,
};

/** Indexed columns and the time after which the pruner may delete the row. */
export function columns(kind, body) {
  const expiresAt = Object.hasOwn(EXPIRY, kind) ? EXPIRY[kind](body) : null;
  return {
    project: body.project ?? null,
    state: body[STATE_FIELD[kind] || 'state'] ?? null,
    expiresAt: expiresAt ?? null,
  };
}

/** Whether a body satisfies an indexed filter (`project`, `states`). */
export const matches = (kind, body, filter) => {
  const c = columns(kind, body);
  return (
    (filter.project === undefined || c.project === filter.project) &&
    (!filter.states || filter.states.includes(c.state))
  );
};
