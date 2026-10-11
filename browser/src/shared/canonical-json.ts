/**
 * JSON with object keys in sorted order, so two values with the same content
 * always give the same text whichever order their fields arrived in. A persona
 * read back from disk and the same persona sent by the server differ only in
 * field order; comparing them as plain JSON called that a change.
 */

/** `value` with every object's keys sorted, arrays left in order. */
function sorted(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sorted);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((k) => [k, sorted((value as Record<string, unknown>)[k])]),
  );
}

/** Order-independent JSON text of `value`. */
export const canonicalJson = (value: unknown): string => JSON.stringify(sorted(value));
