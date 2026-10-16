/** Pure screen-space hit testing for transfers between existing tab strips. */
import type { Point, Rectangle } from 'electron';
import { WINDOW_TAB_STRIP_HEIGHT } from './constants.ts';
/** Electron screen and window bounds are both device-independent, including on mixed-DPI monitors. */
export function inTabStrip(point: Point, bounds: Rectangle): boolean {
  return (
    point.x >= bounds.x &&
    point.x < bounds.x + bounds.width &&
    point.y >= bounds.y &&
    point.y < bounds.y + WINDOW_TAB_STRIP_HEIGHT
  );
}

/** Full-window hit testing prevents dropping through a foreground browser into a hidden strip. */
export function inWindow(point: Point, bounds: Rectangle): boolean {
  return (
    point.x >= bounds.x &&
    point.x < bounds.x + bounds.width &&
    point.y >= bounds.y &&
    point.y < bounds.y + bounds.height
  );
}
