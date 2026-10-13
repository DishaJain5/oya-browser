/**
 * Checks the anonymous report keel-license sends. Only these six fields reach
 * storage; paths, addresses and arbitrary properties are never kept.
 */
import { invalid } from '../../platform/errors.ts';
import { KEEL_DAY_CHARS, KEEL_MAX_STEPS, KEEL_VERSION_MAX_LENGTH } from './constants.ts';

/** Random installation ids made by keel-license. */
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
/** Cargo release versions, including prereleases and build metadata. */
const VERSION = /^\d+\.\d+\.\d+(?:[-+][\w.+-]+)?$/;
/** Short opaque license identifiers, never the signed key itself. */
const LICENSE_ID = /^[\w-]{1,64}$/;
/** Rust's OS names for Keel's supported desktop platforms. */
const OS = /^(macos|windows|linux)$/;

/** A string that matches its field's format, without coercing other JSON types. */
function text(body, field: string, pattern: RegExp): string {
  const value = body?.[field];
  if (typeof value !== 'string' || !pattern.test(value)) throw invalid(field, 'a valid string', value);
  return value;
}

/** The unsigned step count, with no rounding or coercion. */
function stepsOf(body): number {
  const steps = body?.steps;
  if (!Number.isInteger(steps) || steps < 0 || steps > KEEL_MAX_STEPS)
    throw invalid('steps', 'an unsigned 32-bit whole number', steps);
  return steps;
}

/** A real calendar day, or Keel's empty-day marker before it has a completed day. */
function dayOf(body, steps: number): string {
  const day = body?.day;
  if (day === '' && steps === 0) return day;
  if (typeof day !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(day)) throw invalid('day', 'YYYY-MM-DD', day);
  const date = new Date(`${day}T00:00:00.000Z`);
  if (!Number.isFinite(date.getTime()) || date.toISOString().slice(0, KEEL_DAY_CHARS) !== day)
    throw invalid('day', 'a calendar date', day);
  return day;
}

/** The Cargo version, with a bounded prerelease suffix. */
function versionOf(body): string {
  const version = text(body, 'version', VERSION);
  if (version.length > KEEL_VERSION_MAX_LENGTH) throw invalid('version', 'a short version', version);
  return version;
}

/** A checked report with unknown fields discarded. */
export function parseKeelPing(body) {
  const install_id = text(body, 'install_id', UUID).toLowerCase();
  const version = versionOf(body);
  const os = text(body, 'os', OS);
  const steps = stepsOf(body);
  const day = dayOf(body, steps);
  const license_id = body.license_id === null ? null : text(body, 'license_id', LICENSE_ID);
  return { install_id, version, os, day, steps, license_id };
}

/** A report after its boundary validation. */
export type KeelPing = ReturnType<typeof parseKeelPing>;
