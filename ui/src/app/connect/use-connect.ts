/**
 * Answering an app's request to connect to Oya (claude.ai, over OAuth). The
 * request id arrives in the link and waits in this tab's storage through a
 * sign-in; once someone is signed in the page shows which app asks and lets
 * them allow it for one of their projects, or turn it down. Either answer sends
 * the browser back to the app.
 */
'use client';

import { useCallback, useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { call, message } from '@/components/dashboard/projects/project-api';
import type { Project } from '@/components/dashboard/projects/types';
import { forgetConnect, pendingConnect, rememberConnect } from '@/lib/connect';

/** Where the request stands. */
export interface ConnectState {
  /** Loading, waiting on a sign-in, ready to answer, answering, refused, or no request in the link. */
  status: 'loading' | 'signed-out' | 'ready' | 'working' | 'failed' | 'missing';
  /** The app asking. */
  client?: string;
  /** The host the answer goes back to. */
  redirectHost?: string;
  /** Projects the person can connect, operators and up. */
  projects?: Project[];
  /** Why it failed, when it did. */
  error?: string;
}

/** What the page needs: the state, the chosen project, and the two answers. */
export interface Connect {
  /** Where the request stands. */
  state: ConnectState;
  /** The project chosen to connect. */
  project: string;
  /** Chooses the project. */
  setProject: (id: string) => void;
  /** Allows the request for the chosen project. */
  allow: () => void;
  /** Turns the request down. */
  deny: () => void;
}

/** Which app asks, as GET /auth/connect/:id answers. */
interface Asking {
  /** The app's name. */
  client: string;
  /** The host the answer goes back to. */
  redirectHost: string;
}

/** An answer's reply: where to send the browser. */
interface Answered {
  /** The app's redirect URI, carrying a code or an error. */
  redirect: string;
}

/** Moves a request id in the link to storage; the request waiting to be answered, or ''. */
function takeRequestFromUrl() {
  const fromLink = new URLSearchParams(window.location.search).get('request');
  if (fromLink) rememberConnect(fromLink);
  return pendingConnect();
}

/** The request's app and the projects a viewer cannot connect left out (MCP needs an operator). */
async function load(token: string, id: string): Promise<ConnectState> {
  const path = `/auth/connect/${encodeURIComponent(id)}`;
  const [asking, projects] = await Promise.all([
    call<Asking>(token, path, {}, 'Could not load this request'),
    call<Project[]>(token, '/auth/projects', {}, 'Could not load projects'),
  ]);
  return { status: 'ready', ...asking, projects: projects.filter((p) => p.role !== 'viewer') };
}

/** The redirect as a URL to follow, refusing anything but http(s): a javascript: one would run on this origin. */
function followable(redirect: string) {
  const url = URL.parse(redirect);
  if (url?.protocol !== 'https:' && url?.protocol !== 'http:')
    throw new Error('The app gave an unsafe address to return to');
  return url.href;
}

/** Sends one answer and follows its redirect back to the app. */
async function answer(token: string, id: string, verb: 'allow' | 'deny', project?: string) {
  const init = { method: 'POST', body: JSON.stringify(project ? { project } : {}) };
  const { redirect } = await call<Answered>(token, `/auth/connect/${encodeURIComponent(id)}/${verb}`, init);
  forgetConnect();
  window.location.assign(followable(redirect));
}

/** The request's state once the session is known. */
function useRequest(token: string | null, loading: boolean) {
  const [state, setState] = useState<ConnectState>({ status: 'loading' });
  useEffect(() => {
    if (loading) return;
    const id = takeRequestFromUrl();
    if (!id || !token) return void queueMicrotask(() => setState({ status: id ? 'signed-out' : 'missing' }));
    load(token, id).then(setState, (e) => setState({ status: 'failed', error: message(e) }));
  }, [loading, token]);
  return [state, setState] as const;
}

/** Sends an answer, showing it as in flight, and as failed if the server refuses it. */
function useSend(token: string | null, project: string, setState: (next: (s: ConnectState) => ConnectState) => void) {
  return useCallback(
    (verb: 'allow' | 'deny') => {
      setState((s) => ({ ...s, status: 'working' }));
      const fail = (e: unknown) => setState((s) => ({ ...s, status: 'failed', error: message(e) }));
      answer(token || '', pendingConnect(), verb, verb === 'allow' ? project : undefined).catch(fail);
    },
    [token, project, setState],
  );
}

/** The connect page's state and answers. */
export function useConnect(): Connect {
  const { token, loading } = useAuth();
  const [state, setState] = useRequest(token, loading);
  const [chosen, setProject] = useState('');
  const project = chosen || state.projects?.[0]?.id || '';
  const send = useSend(token, project, setState);
  return { state, project, setProject, allow: () => send('allow'), deny: () => send('deny') };
}
