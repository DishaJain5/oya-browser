/**
 * Keel usage kept apart from Oya Browser installs. One row per install and
 * reported day makes repeated or concurrent deliveries idempotent.
 */
import { getConnection } from '../../platform/storage/index.ts';
import type { KeelPing } from './keel-ping.ts';

/** Keeps the first report for this install and usage day, including the initial empty day. */
export async function recordKeelPing(ping: KeelPing) {
  await getConnection().upsert('keel_usage', [{ ...ping, received_at: new Date().toISOString() }]);
}
