/**
 * The OAuth server's state, kept in the control store so any replica can answer
 * any step: registered clients (kept), sign-in requests waiting on the person
 * (ten minutes) and authorization codes (one minute, single use). Requests and
 * codes are stored by digest, so reading the database never yields a usable one.
 */
import { randomBytes } from 'node:crypto';
import type { OAuthClientInformationFull } from '@modelcontextprotocol/sdk/shared/auth.js';
import { control, hash } from '../../modules/control/service.ts';
import { CODE_TTL_MS, REQUEST_TTL_MS, SECRET_BYTES } from './constants.ts';

/** A sign-in the client started and the person has yet to allow. */
export interface ConnectRequest {
  /** The registered client asking. */
  clientId: string;
  /** Where the answer goes, already checked against the client's registered URIs. */
  redirectUri: string;
  /** The PKCE challenge the code is bound to. */
  codeChallenge: string;
  /** The client's state, handed back unchanged. */
  state?: string;
}

/** An allowed request: who allowed it and for which project. The credential is minted at exchange. */
export interface Grant extends ConnectRequest {
  /** The person who allowed it. */
  userId: string;
  /** The project they chose. */
  projectId: string;
}

/** A fresh unguessable value. */
const secret = () => randomBytes(SECRET_BYTES).toString('base64url');

/** A registered client, or undefined. */
export async function getClient(clientId: string) {
  return (await control().store.get('oauth_client', clientId)) ?? undefined;
}

/** Marks a client as in use, so one that keeps signing people in is never forgotten. */
export async function touchClient(clientId: string) {
  await control().store.transact(async (tx) => {
    const client = await tx.get('oauth_client', clientId);
    if (client) tx.put('oauth_client', clientId, { ...client, lastUsedAt: Date.now() });
  });
}

/**
 * Keeps a client the SDK has just registered. The SDK's type leaves the id out, but
 * with its default id generation on it has filled the id in (and any secret) by now.
 */
export async function putClient(registered: Omit<OAuthClientInformationFull, 'client_id' | 'client_id_issued_at'>) {
  const client = registered as OAuthClientInformationFull;
  await control().store.transact(async (tx) => {
    tx.put('oauth_client', client.client_id, client);
  });
  return client;
}

/** Stores `body` under the digest of a fresh value, expiring after `ttl`; returns the value. */
async function putSecret(kind: string, body: object, ttl: number) {
  const value = secret();
  const id = hash(value);
  await control().store.transact(async (tx) => {
    tx.put(kind, id, { ...body, id, expiresAt: Date.now() + ttl });
  });
  return value;
}

/** An unexpired row stored by `putSecret`, or null. */
async function readSecret(kind: string, value: string) {
  const row = value ? await control().store.get(kind, hash(value)) : null;
  return row && row.expiresAt > Date.now() ? row : null;
}

/** Takes a row stored by `putSecret` out of the store; it if it had not expired, else null. */
async function takeSecret(kind: string, value: string) {
  if (!value) return null;
  const row = await control().store.transact(async (tx) => {
    const found = await tx.get(kind, hash(value));
    if (found) await tx.delete(kind, hash(value));
    return found;
  });
  return row && row.expiresAt > Date.now() ? row : null;
}

/** Saves a request for the person to allow; returns its id for the console link. */
export const openRequest = (request: ConnectRequest) => putSecret('oauth_request', request, REQUEST_TTL_MS);

/** A request still waiting, or null. */
export const findRequest = (id: string): Promise<ConnectRequest | null> => readSecret('oauth_request', id);

/** Ends a request, allowed or not; the request if it was still waiting, else null. */
export const closeRequest = (id: string): Promise<ConnectRequest | null> => takeSecret('oauth_request', id);

/** Saves an allowed request's grant; returns the authorization code. */
export const issueCode = (grant: Grant) => putSecret('oauth_code', grant, CODE_TTL_MS);

/** A code's grant without spending it (the SDK reads its PKCE challenge first), or null. */
export const peekCode = (code: string): Promise<Grant | null> => readSecret('oauth_code', code);

/** Spends a code; its grant if it was unused and unexpired, else null. */
export const spendCode = (code: string): Promise<Grant | null> => takeSecret('oauth_code', code);
