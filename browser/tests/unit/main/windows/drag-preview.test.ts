/**
 * Unit tests for src/main/windows/drag-preview.ts: the native drag card lives
 * exactly as long as the person may move the dragged tab. Agents driving other
 * tabs do not end it; an agent claiming this one does; a tab an agent drives
 * never gets one.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { TabDragPreview } from '../../../../src/main/windows/drag-preview.ts';
import { AgentTab } from '../../../../src/main/control/agent-tab.ts';
import { DRAG_FRAME_MS } from '../../../../src/main/windows/constants.ts';

/** The card as a sandboxed window, recording whether it was destroyed. */
class Card {
  /** The latest card made. */
  static last: Card | undefined;
  /** Set by destroy(). */
  destroyed = false;
  /** Shown or hidden. */
  visible = false;
  /** As Electron's, less everything the card never uses. */
  webContents = { setWindowOpenHandler() {}, on() {}, executeJavaScript: async () => {} };
  /** Remembers itself for the test. */
  constructor() {
    Card.last = this;
  }
  /** As Electron's. */
  setIgnoreMouseEvents() {}
  /** As Electron's. */
  loadFile() {
    return Promise.resolve();
  }
  /** As Electron's. */
  isDestroyed() {
    return this.destroyed;
  }
  /** As Electron's. */
  destroy() {
    this.destroyed = true;
  }
  /** As Electron's. */
  setPosition() {}
  /** As Electron's. */
  hide() {
    this.visible = false;
  }
  /** As Electron's. */
  showInactive() {
    this.visible = true;
  }
  /** As Electron's. */
  isVisible() {
    return this.visible;
  }
}

/** Control scoped to the server's agents: they drive, and the person keeps the tabs no agent claimed. */
const SCOPED = { connected: true, mode: 'agent', interactive: false, localClients: 0 };

/** A window's services with one open tab and the real claims over a fixed control state. */
function source(control = SCOPED) {
  const tab = { id: 1, title: 'Tab', view: { webContents: { capturePage: async () => ({ isEmpty: () => true }) } } };
  const ctx: any = {
    appDir: '/app',
    electron: {
      BrowserWindow: Card,
      screen: {
        getCursorScreenPoint: () => ({ x: 10, y: 900 }),
        getDisplayNearestPoint: () => ({ workArea: { x: 0, y: 0, width: 2000, height: 1200 } }),
      },
    },
    shell: {
      alive: () => true,
      window: Object.assign(new EventEmitter(), { getContentBounds: () => ({ x: 0, y: 0, width: 1280, height: 800 }) }),
    },
    control: { snapshot: () => control },
    tabs: {
      list: [tab],
      activeTabId: 1,
      find: (id: number) => (id === 1 ? tab : undefined),
      getActiveView: () => tab.view,
    },
  };
  ctx.agentTab = new AgentTab(ctx);
  return ctx;
}

describe('TabDragPreview', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setInterval', 'setTimeout', 'Date'] }));
  afterEach(() => mock.timers.reset());

  it('keeps the card for a tab no agent claimed while agents drive other tabs', () => {
    const ctx = source();
    new TabDragPreview().begin(ctx, 1);
    mock.timers.tick(DRAG_FRAME_MS);
    assert.equal(Card.last!.destroyed, false);
  });

  it('drops the card once an agent claims the dragged tab', () => {
    const ctx = source();
    new TabDragPreview().begin(ctx, 1);
    ctx.agentTab.claim(1);
    mock.timers.tick(DRAG_FRAME_MS);
    assert.equal(Card.last!.destroyed, true);
  });

  it('refuses to begin on a tab an agent drives', () => {
    const ctx = source();
    ctx.agentTab.claim(1);
    assert.throws(() => new TabDragPreview().begin(ctx, 1), /Human Mode/);
  });
});
