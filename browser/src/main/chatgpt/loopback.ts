/**
 * The one-shot listener a sign-in returns to: 127.0.0.1 on a port the OS picks,
 * answering /auth/callback once with a "close this tab" page and closing after
 * it, or when the sign-in has taken too long.
 */
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as C from './constants.ts';

/** A listener waiting for one sign-in to come back. */
export interface Callback {
  /** The address to return to, known once the listener is up. */
  redirectUri: string;
  /** The returned query, once the browser comes back. */
  done: Promise<Record<string, string>>;
  /** Stops listening. */
  close(): void;
}

/** Answers one request: the callback's page (and its query), or 404 for anything else. */
function answer(req: http.IncomingMessage, res: http.ServerResponse, settle: (q: Record<string, string>) => void) {
  const url = new URL(req.url || '/', `http://${C.LOOPBACK_HOST}`);
  if (url.pathname !== C.CALLBACK_PATH) return void res.writeHead(C.HTTP_NOT_FOUND).end();
  res.writeHead(C.HTTP_OK, { 'Content-Type': 'text/html; charset=utf-8' }).end(C.CALLBACK_PAGE);
  settle(Object.fromEntries(url.searchParams));
}

/** Starts listening; `ttlMs` bounds how long the sign-in may take. */
export function awaitCallback(ttlMs = C.LOGIN_TTL_MS): Promise<Callback> {
  let settle: (q: Record<string, string>) => void = () => {};
  let fail: (e: Error) => void = () => {};
  const done = new Promise<Record<string, string>>((resolve, reject) => ((settle = resolve), (fail = reject)));
  const server = http.createServer((req, res) => answer(req, res, settle));
  const timer = setTimeout(() => fail(Error('Sign-in took too long. Try again.')), ttlMs);
  const close = () => (clearTimeout(timer), server.close());
  void done.then(close, close);
  return listening(server, done, close);
}

/** Resolves once the server listens, with the address it listens on. */
function listening(server: http.Server, done: Callback['done'], close: () => void): Promise<Callback> {
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(C.LOOPBACK_PORT, C.LOOPBACK_HOST, () => {
      const { port } = server.address() as AddressInfo;
      resolve({ redirectUri: `http://${C.LOOPBACK_HOST}:${port}${C.CALLBACK_PATH}`, done, close });
    });
  });
}
