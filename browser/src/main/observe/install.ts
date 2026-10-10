/**
 * Where the observer is attached: the persona's session for requests, and each
 * tab's webContents for console entries.
 *
 * Both hooks live in the browser process. `webRequest` is the same API
 * governance uses to contain egress, and `console-message` is Electron's own
 * renderer channel. Neither hook attaches a debugger or injects a page script.
 */
import type { Session, WebContents } from 'electron';
import type { Observer } from './observer.ts';
import { NetworkTrace } from './network.ts';

/** Watches every request the persona's session finishes or fails. */
export function watchSession(observer: Observer, session: Pick<Session, 'webRequest'>): void {
  const trace = new NetworkTrace(observer),
    requests = session.webRequest;
  requests.onSendHeaders((details) => trace.event('request', details));
  requests.onResponseStarted((details) => trace.event('response', details));
  requests.onBeforeRedirect((details) => trace.event('redirect', details));
  watchFinished(observer, trace, requests);
}

/**
 * Watches one tab's console. Electron passes the details on the event (the
 * positional arguments are deprecated); its level is a name, which the
 * observer keeps as is.
 */
export function watchContents(observer: Observer, contents: Pick<WebContents, 'on' | 'id'>): void {
  contents.on('destroyed', () => observer.traces.remove(contents.id));
  contents.on('console-message', ({ level, message, lineNumber, sourceId }) =>
    observer.addConsole({ level, message, line: lineNumber, sourceId, webContentsId: contents.id }),
  );
}

/** Completion snapshots and traces share the session's one listener per event. */
function watchFinished(observer: Observer, trace: NetworkTrace, requests: Session['webRequest']): void {
  requests.onCompleted((details) => {
    observer.addRequest(details);
    trace.event('complete', details);
  });
  requests.onErrorOccurred((details) => {
    observer.addRequest({ ...details, error: details.error || 'request failed' });
    trace.event('error', details);
  });
}
