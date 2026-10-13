/** Local collections live in the trusted shell alongside the start page, never in agent-accessible pages. */
import { useEffect, useRef } from 'react';
import { useViewModel } from '../../hooks/index.ts';
import type { ShellViewModels } from '../../app/view-models.ts';
import type { OyaBrowser } from '../../core/bridge.ts';
import { LibraryView } from './library-view.ts';
import template from './library-template.html?raw';
import './library.css';
/** A component lifetime owns its scoped DOM and cancels reads on tab or ownership changes. */
function useCollection(bridge: OyaBrowser, id: number) {
  const root = useRef<HTMLElement>(null);
  useEffect(() => {
    const view = new LibraryView(bridge, root.current!, id);
    return () => view.dispose();
  }, [bridge, id]);
  return root;
}
/** Only the packaged constant template uses HTML; all website data uses textContent. */
function Collection({
  bridge,
  id,
}: {
  /** Trusted shell bridge. */
  bridge: OyaBrowser;
  /** Native selected tab. */
  id: number;
}) {
  const root = useCollection(bridge, id);
  return (
    <section
      className="library-page"
      ref={root}
      aria-label="Saved pages"
      dangerouslySetInnerHTML={{ __html: template }}
    />
  );
}
/** Profile changes close tabs; ownership changes unmount private contents immediately. */
export function LibraryPage({
  vms,
}: {
  /** Shared shell state and its exact-frame bridge. */
  vms: ShellViewModels;
}) {
  const { tabs } = useViewModel(vms.tabs.strip);
  const { control } = useViewModel(vms.control);
  const tab = tabs.find((item) => item.active);
  if (!tab?.collection || (control && !control.pageInteractive && !control.interactive)) return null;
  return <Collection key={tab.id} bridge={vms.env.bridge} id={tab.id} />;
}
