/**
 * Answering a connection request on the person's behalf: which app is asking,
 * allowing it for one of their projects, or turning it down. Each answer is the
 * redirect back to the app, carrying a code or an error as OAuth spells them.
 */
import { roleIn } from '../../modules/control/membership.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { DEFAULT_CLIENT_NAME } from './constants.ts';
import { closeRequest, findRequest, getClient, issueCode, type ConnectRequest } from './store.ts';

/** The refusal for a request that expired, was answered already, or never existed. */
const gone = () => new HttpError(Status.GONE, 'This connection request expired. Start it again from the app.');

/** The app's redirect URI carrying `params` and the app's own state. */
function answer(request: ConnectRequest, params: Record<string, string>) {
  const url = new URL(request.redirectUri);
  for (const [name, value] of Object.entries(params)) url.searchParams.set(name, value);
  if (request.state) url.searchParams.set('state', request.state);
  return url.href;
}

/** Who is asking: the app's name and the host its answer goes to. */
export async function describe(id: string) {
  const request = await findRequest(id);
  if (!request) throw gone();
  const client = await getClient(request.clientId);
  return { client: client?.client_name || DEFAULT_CLIENT_NAME, redirectHost: new URL(request.redirectUri).host };
}

/** Allows the request for a project the person holds a role in; the redirect carrying the code. */
export async function allow(id: string, userId: string, projectId: unknown) {
  const role = typeof projectId === 'string' ? (await roleIn(userId, projectId)).role : null;
  if (!role) throw new HttpError(Status.NOT_FOUND, 'Project not found');
  // A viewer's connector would be refused by every MCP call: say so here instead of after "connected".
  if (role === 'viewer') throw new HttpError(Status.FORBIDDEN, 'Only an operator or administrator can connect an app');
  const request = await closeRequest(id);
  if (!request) throw gone();
  const code = await issueCode({ ...request, userId, projectId: String(projectId) });
  return { redirect: answer(request, { code }) };
}

/** Turns the request down; the redirect telling the app so. */
export async function deny(id: string) {
  const request = await closeRequest(id);
  if (!request) throw gone();
  return { redirect: answer(request, { error: 'access_denied' }) };
}
