/**
 * The playbook editor: the definition as JSON, its variables, the Playwright code
 * for the saved steps, and a tester that runs the edit before it is saved. Save
 * replaces the playbook; Save as keeps the original; leaving with changes asks.
 */
'use client';

import { useState } from 'react';
import Dialog from '@/components/ui/dialog';
import SyntaxCode from '@/components/ui/syntax-code';
import CodeField from './code-field';
import TesterPane from './tester-pane';
import VariablesPane from './variables-pane';
import { EDITOR_TEXT } from './editor-text';
import { usePlaybookEditor, type PlaybookEditorState } from './use-playbook-editor';
import type { BrowserRow } from '../../types';
import type { PlaybookInfo } from '../types';

/** The editor's tabs. */
type Tab = keyof typeof EDITOR_TEXT.tabs;

/** Props for the editor. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The playbook edited. */
  playbook: PlaybookInfo;
  /** Running browsers, to test on. */
  browsers: BrowserRow[];
  /** Closes it. */
  onClose: () => void;
  /** Runs after a save, to refresh the list. */
  onSaved: () => void;
}

/** The tab strip. */
function Tabs({ tab, setTab }: { /** The open tab. */ tab: Tab; /** Opens one. */ setTab: (t: Tab) => void }) {
  return (
    <div role="tablist" className="mb-3 flex gap-1 border-b border-border">
      {(Object.keys(EDITOR_TEXT.tabs) as Tab[]).map((t) => (
        <button
          key={t}
          role="tab"
          aria-selected={tab === t}
          className={`-mb-px border-b-2 px-3 py-2 text-[13px] ${tab === t ? 'border-accent text-text' : 'border-transparent text-text-muted'}`}
          onClick={() => setTab(t)}
        >
          {EDITOR_TEXT.tabs[t]}
        </button>
      ))}
    </div>
  );
}

/** Discard, Save as and Save, with the unsaved-changes note. */
function Footer({ e }: { /** The editor. */ e: PlaybookEditorState }) {
  const saveAs = () => {
    const name = window.prompt(EDITOR_TEXT.saveAsPrompt);
    if (name?.trim()) e.saveAs(name.trim());
  };
  const ok = Boolean(e.parsed.definition) && !e.busy;
  return (
    <>
      {e.dirty && <span className="mr-auto text-[12px] text-yellow">{EDITOR_TEXT.unsaved}</span>}
      <button className="btn-ghost" onClick={e.discard} disabled={!e.dirty || e.busy}>
        {EDITOR_TEXT.discard}
      </button>
      <button className="btn-ghost" onClick={saveAs} disabled={!ok}>
        {EDITOR_TEXT.saveAs}
      </button>
      <button className="btn-primary" onClick={e.save} disabled={!ok || !e.dirty}>
        {e.busy ? EDITOR_TEXT.saving : EDITOR_TEXT.save}
      </button>
    </>
  );
}

/** The open tab's pane. */
function Pane({
  tab,
  e,
  props,
}: {
  /** The tab. */ tab: Tab;
  /** The editor. */ e: PlaybookEditorState;
  /** The editor's props. */ props: Props;
}) {
  if (tab === 'definition') return <CodeField value={e.text} onChange={e.setText} error={e.parsed.error} />;
  if (tab === 'variables')
    return e.parsed.definition ? (
      <VariablesPane definition={e.parsed.definition} onChange={e.setText} />
    ) : (
      <p className="text-[13px] text-red">{e.parsed.error}</p>
    );
  if (tab === 'code')
    return (
      <div className="space-y-2">
        <p className="text-[12px] text-text-muted">{EDITOR_TEXT.codeHint}</p>
        <pre className="max-h-[52dvh] overflow-auto rounded-lg border border-border bg-bg p-3 text-[12px]">
          <SyntaxCode code={e.code ?? props.playbook.code} language="typescript" />
        </pre>
      </div>
    );
  return <TesterPane apiKey={props.apiKey} definition={e.parsed.definition} browsers={props.browsers} />;
}

/** The editor in a dialog. */
export default function PlaybookEditor(props: Props) {
  const e = usePlaybookEditor(props.apiKey, props.playbook.name, props.onSaved);
  const [tab, setTab] = useState<Tab>('definition');
  const close = () => (!e.dirty || window.confirm(EDITOR_TEXT.confirmClose)) && props.onClose();
  return (
    <Dialog
      open
      onClose={close}
      size="lg"
      title={EDITOR_TEXT.title(props.playbook.name)}
      description={EDITOR_TEXT.description}
      footer={<Footer e={e} />}
    >
      {e.loadError && (
        <p role="alert" className="text-[13px] text-red">
          {e.loadError}
        </p>
      )}
      {!e.loadError && e.saved === null && (
        <p role="status" className="py-8 text-center text-[13px] text-text-muted">
          {EDITOR_TEXT.loading}
        </p>
      )}
      {e.saved !== null && (
        <>
          <Tabs tab={tab} setTab={setTab} />
          <Pane tab={tab} e={e} props={props} />
          {e.saveError && (
            <p role="alert" className="mt-3 text-[13px] text-red">
              {e.saveError}
            </p>
          )}
        </>
      )}
    </Dialog>
  );
}
