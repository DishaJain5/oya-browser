/**
 * Unit tests for the playbook editor (editor/playbook-editor.tsx), through what
 * a person sees and clicks: it loads the definition, flags text that is not one,
 * edits a variable, saves with PUT, asks before throwing changes away, and tests
 * the unsaved edit on a browser, showing each step as it plays.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));
vi.mock('@/components/ui/syntax-code', () => ({ default: ({ code }: { code: string }) => <code>{code}</code> }));

import { api } from '@/lib/api-client';
import PlaybookEditor from '@/components/dashboard/playbooks/editor/playbook-editor';
import type { PlaybookInfo } from '@/components/dashboard/playbooks/types';
import type { BrowserRow } from '@/components/dashboard/types';

const apiMock = vi.mocked(api);
const DEF = {
  steps: [{ action: 'navigate', url: 'https://a.test/{{path}}' }],
  defaults: { path: 'signup' },
  secrets: [],
};
const PB = {
  name: 'order',
  variables: ['path'],
  defaults: {},
  steps: 1,
  code: 'await page.goto(url)',
  createdAt: 'c',
  promotedAt: null,
  draft: null,
} as unknown as PlaybookInfo;
const BROWSERS = [{ id: 'b-1', name: 'Local' }] as BrowserRow[];

/** Answers the definition, and `other` for anything else. */
function serve(other: (path: string, opts: { method?: string; body?: unknown }) => unknown = () => ({})) {
  apiMock.mockImplementation(async (path, opts) =>
    path === '/playbooks/order' && !opts?.method ? structuredClone(DEF) : other(path, opts),
  );
}

/** Renders the editor and waits for the definition. */
async function setup(onClose = vi.fn(), onSaved = vi.fn()) {
  render(<PlaybookEditor apiKey="k" playbook={PB} browsers={BROWSERS} onClose={onClose} onSaved={onSaved} />);
  await act(async () => {});
  return { onClose, onSaved };
}

/** The definition textarea. */
const field = () => screen.getByLabelText('Playbook definition (JSON)') as HTMLTextAreaElement;

describe('PlaybookEditor', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('loads the definition into the editor', async () => {
    serve();
    await setup();
    expect(JSON.parse(field().value)).toEqual(DEF);
    expect((screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('says why text that is not a definition cannot be saved', async () => {
    serve();
    await setup();
    fireEvent.change(field(), { target: { value: '{"steps": ' } });
    expect(screen.getByRole('alert').textContent).toMatch(/^Not valid JSON/);
    expect((screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('edits a variable’s default from the Variables tab, and saves with PUT', async () => {
    const { onSaved } = await (serve((path, opts) => (opts.method === 'PUT' ? { name: 'order' } : {})), setup());
    await userEvent.click(screen.getByRole('tab', { name: 'Variables' }));
    fireEvent.change(screen.getByLabelText('Default for path'), { target: { value: 'checkout' } });
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    const put = apiMock.mock.calls.find(([, o]) => o?.method === 'PUT');
    expect(put?.[0]).toBe('/playbooks/order');
    expect((put?.[1]?.body as typeof DEF).defaults).toEqual({ path: 'checkout' });
    expect(onSaved).toHaveBeenCalled();
  });

  it('saves under a new name as an export the import route reads', async () => {
    serve((path) => (path === '/playbooks/import' ? { name: 'order-2', code: 'c' } : {}));
    await setup();
    fireEvent.change(field(), { target: { value: toTextWith('copy') } });
    vi.spyOn(window, 'prompt').mockReturnValue('order-2');
    await userEvent.click(screen.getByRole('button', { name: 'Save as…' }));
    const call = apiMock.mock.calls.find(([p]) => p === '/playbooks/import');
    expect(call?.[1]?.body).toMatchObject({
      name: 'order-2',
      playbook: { format: 'oya-playbook', version: 1, playbook: { name: 'order-2' } },
    });
  });

  it('shows the code for the steps just saved', async () => {
    serve((path, opts) => (opts.method === 'PUT' ? { name: 'order', code: 'await page.goto(saved)' } : {}));
    await setup();
    fireEvent.change(field(), { target: { value: toTextWith('new') } });
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    await userEvent.click(screen.getByRole('tab', { name: 'Code' }));
    expect(screen.getByText('await page.goto(saved)')).toBeTruthy();
  });

  it('asks before closing with unsaved changes, and keeps them when told no', async () => {
    serve();
    const { onClose } = await setup();
    fireEvent.change(field(), { target: { value: toTextWith('x') } });
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
    await userEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(confirm).toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('tests the unsaved edit on a browser and shows each step as it played', async () => {
    serve((path) =>
      path === '/browsers/b-1/runs'
        ? {
            id: 'run_1',
            status: 'failed',
            attention: null,
            error: 'Step 1 broke',
            events: [{ index: 0, action: 'navigate', ok: false }],
          }
        : {},
    );
    await setup();
    fireEvent.change(field(), { target: { value: toTextWith('edited') } });
    await userEvent.click(screen.getByRole('tab', { name: 'Test' }));
    await userEvent.type(screen.getByLabelText('Sample value for path'), 'cart');
    await userEvent.click(screen.getByRole('button', { name: 'Run test' }));
    const body = apiMock.mock.calls.find(([p]) => p === '/browsers/b-1/runs')?.[1]?.body as {
      playbook: typeof DEF;
      data: Record<string, string>;
      autoHeal: boolean;
    };
    expect(body.playbook.steps[0].url).toBe('https://a.test/{{path}}/edited');
    expect(body).toMatchObject({ data: { path: 'cart' }, autoHeal: false });
    expect(screen.getByText('Failed: Step 1 broke')).toBeTruthy();
    expect(screen.getByText('broke')).toBeTruthy();
  });
});

/** The definition with its URL given a suffix, as text. */
function toTextWith(suffix: string) {
  return JSON.stringify({ ...DEF, steps: [{ action: 'navigate', url: `https://a.test/{{path}}/${suffix}` }] });
}
