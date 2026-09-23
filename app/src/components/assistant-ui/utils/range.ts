/**
 * Range normalization for the numeric props the elements take (vendored from
 * the assistant-ui `elements-range` registry item).
 *
 * Elements are driven by a caller's state, so a prop can arrive negative, past
 * the end of its collection, or NaN. Left raw, a negative slice length counts
 * from the end of the array instead of returning nothing.
 */

/** Constrains a value to `min…max`. NaN maps to `min`; with inverted bounds `max` wins. */
export function clamp(value: number, min: number, max: number) {
  if (Number.isNaN(value)) return min;
  return Math.min(max, Math.max(min, value));
}

/** The first `count` items, for a `count` that may be out of range. */
export function take<T>(items: readonly T[], count: number) {
  return items.slice(0, Math.floor(clamp(count, 0, items.length)));
}
