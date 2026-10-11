/**
 * Accessible completion rows: focus stays in the combobox while arrows select a
 * destination. One line each, as a browser's address bar lists them: the page's
 * title, then its address without the scheme; a star marks a bookmark.
 */
import { useViewModel } from '../../../hooks/index.ts';
import { Icon } from '../../../ui/index.ts';
import type { AddressBarProps } from './address-bar.tsx';
import './address-suggestions.css';

/** An address as people read it: no scheme, no www., no trailing slash on a home page. */
const shortAddress = (url: string): string => url.replace(/^https?:\/\/(www\.)?/i, '').replace(/^([^/?#]+)\/$/, '$1');

/** Local history and bookmarks, never a remote provider's search suggestions. */
export function AddressSuggestions({ vm }: AddressBarProps) {
  const { items, selected, open } = useViewModel(vm.completion);
  if (!open) return null;
  return (
    <div className="address-suggestions" id="address-options" role="listbox" aria-label="Address suggestions">
      {items.map((item, index) => (
        <div
          key={item.url}
          id={`address-option-${index}`}
          role="option"
          aria-selected={selected === index}
          aria-description={item.source === 'bookmark' ? 'Bookmark' : 'History'}
          className="address-option"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => vm.submit(item.url)}
        >
          <span className="address-option-icon" aria-hidden="true">
            <Icon name={item.source === 'bookmark' ? 'star' : 'globe'} />
          </span>
          <span className="address-option-title">{item.title}</span>
          <span className="address-option-url">{shortAddress(item.url)}</span>
        </div>
      ))}
    </div>
  );
}
