export type TimelineBoundary = "start" | "end";

export interface TimelineRange {
  start: number;
  end: number;
}

const clamp = (value: number, minimum: number, maximum: number) =>
  Math.max(minimum, Math.min(maximum, value));

/**
 * Moves one range boundary without ever returning a collapsed or inverted
 * selection. Crossing the opposite boundary shifts that boundary too while
 * preserving the previous span where the media duration allows it.
 */
export function moveTimelineBoundary(
  range: TimelineRange,
  boundary: TimelineBoundary,
  requested: number,
  duration: number,
  minimumSpan = 0.01,
): TimelineRange {
  const limit = Math.max(0, Number(duration) || 0);
  if (limit <= minimumSpan) return { start: 0, end: limit };

  const start = clamp(Number(range.start) || 0, 0, limit);
  const end = clamp(Number(range.end) || 0, start, limit);
  const span = clamp(end - start, minimumSpan, limit);
  const at = clamp(Number(requested) || 0, 0, limit);

  if (boundary === "start") {
    const nextStart = Math.min(at, limit - minimumSpan);
    const nextEnd = nextStart < end ? end : Math.min(limit, nextStart + span);
    return { start: nextStart, end: Math.max(nextStart + minimumSpan, nextEnd) };
  }

  const nextEnd = Math.max(at, minimumSpan);
  const nextStart = nextEnd > start ? start : Math.max(0, nextEnd - span);
  return { start: Math.min(nextStart, nextEnd - minimumSpan), end: nextEnd };
}
