/**
 * Unit tests for connecting an app to Oya: the request waits through a sign-in,
 * the signed-in person sees which app asks and picks a project they can
 * operate, and allowing or turning it down posts that answer; a missing or
 * refused request says so.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import ConnectPage from '@/app/connect/page';
import { call } from '@/components/dashboard/projects/project-api';
import { afterSignIn } from '@/components/auth/use-auth-form';

const auth = { token: null as string | null, loading: false };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('next/navigation', () => ({ useRouter: () => ({ replace: vi.fn() }) }));
vi.mock('@/components/dashboard/projects/project-api', () => ({
  call: vi.fn(),
  message: (e: unknown) => (e instanceof Error ? e.message : 'Something went wrong'),
}));

/** The projects the account holds: an owned one, a shared one, and one it can only view. */
const PROJECTS = [
  { id: 'prj_a', name: 'Acme', role: 'administrator' },
  { id: 'prj_b', name: 'Beta', role: 'operator' },
  { id: 'prj_v', name: 'Viewing', role: 'viewer' },
];

/** Answers the page's reads and records its answers. */
function serve(_token: string, path: string, init?: RequestInit) {
  if (path === '/auth/projects') return Promise.resolve(PROJECTS);
  if (init?.method === 'POST') return Promise.resolve({ redirect: 'https://claude.ai/cb?code=c' });
  return Promise.resolve({ client: 'Claude', redirectHost: 'claude.ai' });
}

/** Opens the link claude.ai sends the person to. */
const openLink = (request = 'req-1') =>
  window.history.replaceState(null, '', `/connect${request ? `?request=${request}` : ''}`);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  sessionStorage.clear();
  auth.token = null;
});

describe('ConnectPage', () => {
  it('asks a signed-out person to sign in, then brings them back here', async () => {
    openLink();
    render(<ConnectPage />);
    expect(await screen.findByText('Connect an app to Oya')).toBeTruthy();
    expect(afterSignIn()).toBe('/connect');
    expect(call).not.toHaveBeenCalled();
  });

  it('names the app and offers only projects the person can operate', async () => {
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect(await screen.findByText('Allow Claude to use Oya?')).toBeTruthy();
    const options = screen.getAllByRole('option').map((o) => o.textContent);
    expect(options).toEqual(['Acme', 'Beta']);
  });

  it('allows the request for the chosen project', async () => {
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    fireEvent.change(await screen.findByLabelText('Project'), { target: { value: 'prj_b' } });
    fireEvent.click(screen.getByRole('button', { name: 'Allow' }));
    await vi.waitFor(() =>
      expect(call).toHaveBeenCalledWith('jwt', '/auth/connect/req-1/allow', {
        method: 'POST',
        body: JSON.stringify({ project: 'prj_b' }),
      }),
    );
  });

  it('turns the request down on Cancel', async () => {
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));
    await vi.waitFor(() =>
      expect(call).toHaveBeenCalledWith('jwt', '/auth/connect/req-1/deny', { method: 'POST', body: '{}' }),
    );
  });

  it('refuses to follow a redirect that is not http(s)', async () => {
    vi.mocked(call).mockImplementation(((_t: string, path: string, init?: RequestInit) =>
      init?.method === 'POST' ? Promise.resolve({ redirect: 'javascript:alert(1)' }) : serve(_t, path, init)) as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    fireEvent.click(await screen.findByRole('button', { name: 'Allow' }));
    expect((await screen.findByRole('alert')).textContent).toMatch(/unsafe address/);
  });

  it('lets a person with only viewer projects cancel, so the app is not left waiting', async () => {
    vi.mocked(call).mockImplementation(((t: string, path: string, init?: RequestInit) =>
      path === '/auth/projects' ? Promise.resolve([PROJECTS[2]]) : serve(t, path, init)) as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));
    await vi.waitFor(() =>
      expect(call).toHaveBeenCalledWith('jwt', '/auth/connect/req-1/deny', { method: 'POST', body: '{}' }),
    );
  });

  it('says so when the request expired', async () => {
    vi.mocked(call).mockRejectedValue(new Error('This connection request expired. Start it again from the app.'));
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect((await screen.findByRole('alert')).textContent).toMatch(/expired/);
  });

  it('says so when the link carries no request', async () => {
    openLink('');
    render(<ConnectPage />);
    expect(await screen.findByText('No request in this link')).toBeTruthy();
  });
});
