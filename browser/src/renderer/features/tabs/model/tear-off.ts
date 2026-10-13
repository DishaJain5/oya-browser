/** A drag leaving the tab strip by a deliberate margin becomes a native-window transfer. */
import { RendererConstants as C } from '../../../core/constants.ts';
/** Synthetic reorder-only strip ports may omit their vertical extent. */
type Edges = Pick<DOMRectReadOnly, 'left' | 'right'> & Partial<Pick<DOMRectReadOnly, 'top' | 'bottom'>>;
/**
 * Only measured vertical bounds allow tear-off; synthetic ports remain reorder-only. Sideways the
 * tab leaves only past `across` (the whole tab bar, as in Chrome): the strip's own ends sit a few
 * pixels from its first tab, so a drag thrown to the start would tear off instead of reordering.
 */
export function outsideStrip(x: number, y: number, bounds?: Edges, across: Edges = bounds!): boolean {
  const { top, bottom } = bounds ?? {};
  if (!bounds || top === undefined || bottom === undefined) return false;
  const margin = C.TAB_DRAG_THRESHOLD;
  const horizontal = x < across.left - margin || x > across.right + margin;
  return horizontal || y < top - margin || y > bottom + margin;
}
