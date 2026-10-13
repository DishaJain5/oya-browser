/**
 * The bookmarks bar under the toolbar: each bookmark a click away (Cmd/Ctrl-click
 * opens it in a new tab), scrolling sideways when they do not all fit (a wheel works
 * too, with soft edges where more wait), All bookmarks at its end, and a way to hide it. Before any
 * bookmark, it says how to make one.
 */
import { useEffect, useRef, type RefObject } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Icon, IconButton } from '../../../ui/index.ts';
import type { BookmarksBarViewModel } from '../view-models/bookmarks-bar-view-model.ts';
import { LibraryButtons } from './library-buttons.tsx';
import { TEXT } from '../model/constants.ts';
import './bookmarks-bar.css';

/** What the bar is given. */
export interface BookmarksBarProps {
  /** The bar's ViewModel. */
  vm: BookmarksBarViewModel;
}

/** Which edges of `list` have more bookmarks past them: 'start', 'end', 'both' or ''. */
function moreOf(list: HTMLElement): string {
  const start = list.scrollLeft > 1;
  const end = list.scrollLeft + list.clientWidth < list.scrollWidth - 1;
  return start && end ? 'both' : start ? 'start' : end ? 'end' : '';
}

/** A vertical wheel turns into sideways scrolling, so a mouse reaches every bookmark, not only a trackpad. */
function sideways(list: HTMLElement, event: WheelEvent): void {
  if (Math.abs(event.deltaY) <= Math.abs(event.deltaX) || list.scrollWidth <= list.clientWidth) return;
  event.preventDefault();
  list.scrollBy({ left: event.deltaY, behavior: 'auto' });
}

/** Wires `list`'s scrolling: wheel to sideways, and its data-more edges kept in step; answers the unwiring. */
function wireSideScroll(list: HTMLElement): () => void {
  const mark = () => list.setAttribute('data-more', moreOf(list));
  const wheel = (event: WheelEvent) => sideways(list, event);
  const observer = new ResizeObserver(mark);
  list.addEventListener('scroll', mark, { passive: true });
  list.addEventListener('wheel', wheel, { passive: false });
  observer.observe(list);
  mark();
  return () => {
    list.removeEventListener('scroll', mark);
    list.removeEventListener('wheel', wheel);
    observer.disconnect();
  };
}

/** Scrolls the list sideways and keeps its fades in step, re-wired when the bookmarks change. */
function useSideScroll(ref: RefObject<HTMLUListElement | null>, count: number): void {
  useEffect(() => (ref.current ? wireSideScroll(ref.current) : undefined), [ref, count]);
}

/** The bar's bookmarks, or how to add one. */
function Bookmarks({ vm }: BookmarksBarProps) {
  const { bookmarks } = useViewModel(vm);
  const list = useRef<HTMLUListElement>(null);
  useSideScroll(list, bookmarks.length);
  if (!bookmarks.length) return <span className="bookmarks-empty">{TEXT.bookmarksEmpty}</span>;
  return (
    <ul className="bookmarks-list" aria-label={TEXT.bookmarks} ref={list}>
      {bookmarks.map((b) => (
        <li key={b.url}>
          <button className="bookmark-chip" title={b.url} onClick={(e) => vm.open(b.url, e.metaKey || e.ctrlKey)}>
            <Icon name="globe" />
            <span>{b.title}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}

/** The bookmarks bar. */
export function BookmarksBar({ vm }: BookmarksBarProps) {
  const { shown } = useViewModel(vm);
  return (
    <div className="bookmarks-bar" id="bookmarks-bar" hidden={!shown} role="toolbar" aria-label={TEXT.bookmarks}>
      <Bookmarks vm={vm} />
      <LibraryButtons vm={vm} />
      <IconButton
        className="bookmarks-hide"
        id="bookmarks-hide"
        label={TEXT.hideBookmarks}
        icon="close"
        onClick={() => vm.hide()}
      />
    </div>
  );
}
