/** Workflow keyboard input shares the native agent command implementation. */
import type { PageDriver, DriverTab } from '../actions/driver.ts';
import { nativeChord, pressNativeChord, type NativeChord } from '../input/index.ts';
/** Preserve the published workflow key shape. */
export type WorkflowKey = NativeChord;
/** Preserve workflow validation diagnostics while using the shared native parser. */
export function workflowKey(value: string): WorkflowKey {
  try {
    return nativeChord(value);
  } catch (error) {
    throw Error(String((error as Error).message).replace('native page', 'native workflow'), { cause: error });
  }
}
/** One native key chord directed at the exact run-owned tab. */
export async function pressWorkflowKey(driver: PageDriver, tab: DriverTab, value: string): Promise<void> {
  await pressNativeChord(driver.keyboard, tab.view, value);
}
