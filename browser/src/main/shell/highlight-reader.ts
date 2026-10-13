/** Native, document-scoped measurements for scan outlines and action targets. */
import type { BrowserView } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { ShieldBox } from './control-shield.ts';
import { highlightMeasureJs } from './highlight-measure.ts';
import type { AnalyzedElement } from './narration.ts';
import { pageTone } from './page-tone.ts';

/** Action feedback uses the colour of the interaction it represents. */
const TARGET_TYPES: Record<string, string> = { type: 'input', select: 'select' };

/** Page reads share one zoom conversion, and never retry in a replacement document. */
export class HighlightReader {
  /** Native isolated-world evaluator. */
  private readonly deps: Pick<AppServices, 'world'>;
  /** Current overlay, whose independent zoom defines its CSS pixel grid. */
  private readonly shield: () => BrowserView | null;

  /** Receives only the page reader and current overlay. */
  constructor(deps: Pick<AppServices, 'world'>, shield: () => BrowserView | null) {
    this.deps = deps;
    this.shield = shield;
  }

  /** Captures exact elements and the page's tone for one scan. */
  async analysis(view: BrowserView, elements: readonly AnalyzedElement[], key: string) {
    const read = (js: string) => this.deps.world.evaluate(view, js, { retry: false });
    const geometry = this.measure(view, highlightMeasureJs(elements, 'capture', key));
    const [boxes, tone] = await Promise.all([geometry, pageTone(read)]);
    return { phase: 'found', boxes: boxes || [], ...(tone && { tone }) };
  }

  /** Pins the exact action target for its brief feedback, including any subsequent movement. */
  async target(view: BrowserView, id: number, action: string, key: string): Promise<ShieldBox | null> {
    const type = Object.hasOwn(TARGET_TYPES, action) ? TARGET_TYPES[action] : 'button';
    const js = highlightMeasureJs([{ id, type, visible: true }], 'target', key);
    return (await this.measure(view, js))?.[0] || null;
  }

  /** Maps current page CSS pixels to overlay CSS pixels; destroyed surfaces have no valid geometry. */
  async measure(view: BrowserView, js: string): Promise<ShieldBox[] | null> {
    const boxes = await this.deps.world.evaluate<ShieldBox[] | null>(view, js, { retry: false }).catch(() => null);
    const shield = this.shield();
    if (!boxes || !shield || view.webContents.isDestroyed() || shield.webContents.isDestroyed()) return null;
    const k = view.webContents.getZoomFactor() / shield.webContents.getZoomFactor();
    return boxes.map((b) => ({ ...b, x: b.x * k, y: b.y * k, w: b.w * k, h: b.h * k }));
  }
}
