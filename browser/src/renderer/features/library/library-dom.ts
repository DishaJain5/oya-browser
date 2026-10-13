/** Accessible, literal DOM for local library rows; no remote icons or unsafe markup. */
import { host } from './library-model.ts';
import type { SavedPage, LibraryAction } from '../../../shared/library.ts';
/** Website titles remain plain text even when they contain markup. */
export function text(tag: string, className: string, value: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  node.textContent = value;
  return node;
}
/** Native buttons give every row and action keyboard and assistive-technology support. */
export function button(label: string, className: string, run: () => void): HTMLButtonElement {
  const node = document.createElement('button');
  node.className = className;
  node.textContent = label;
  node.addEventListener('click', run);
  return node;
}
/** The title and readable URL take the space, with a local site monogram for recognition. */
function pageLink(page: SavedPage, open: () => void): HTMLElement {
  const link = button('', 'page-link', open);
  const info = text('span', 'page-info', '');
  info.append(text('strong', 'page-title', page.title || host(page)), text('span', 'page-url', page.url));
  link.append(monogram(page), info);
  link.title = `Open in new tab: ${page.url}`;
  return link;
}
/** A saved-state action stays visible and names exactly which page it changes. */
export function row(page: SavedPage, saved: boolean, action: (kind: LibraryAction, url: string) => void): HTMLElement {
  const item = text('li', 'page-row', '');
  item.dataset.url = page.url;
  item.append(
    pageLink(page, () => action('open', page.url)),
    visitTime(page),
    saveButton(page, saved, action),
  );
  return item;
}

/** Site recognition without sending any history to a favicon service. */
function monogram(page: SavedPage): HTMLElement {
  return text(
    'span',
    'site-icon',
    host(page)
      .replace(/^www\./, '')
      .charAt(0)
      .toUpperCase(),
  );
}
/** Visible time also exposes the complete local date on hover. */
function visitTime(page: SavedPage): HTMLElement {
  const node = text(
    'time',
    'page-time',
    new Date(page.time).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }),
  );
  node.title = new Date(page.time).toLocaleString();
  return node;
}
/** A labeled toggle remains keyboard accessible after its row refreshes. */
function saveButton(page: SavedPage, saved: boolean, action: (kind: LibraryAction, url: string) => void): HTMLElement {
  const save = button(saved ? '★' : '☆', 'save-page', () => action(saved ? 'remove' : 'bookmark', page.url));
  save.setAttribute('aria-label', `${saved ? 'Remove bookmark' : 'Bookmark'}: ${page.title}`);
  save.setAttribute('aria-pressed', String(saved));
  save.title = saved ? 'Remove bookmark' : 'Save bookmark';
  return save;
}
