// Parses typed times such as "5025", "83:20", "1:23:45" or "1:23:45.5" into
// seconds. Returns null for anything else.
export function parseTimecode(text: string): number | null {
  const value = text.trim().replace(",", ".");
  if (!/^\d+(?::\d{1,2}){0,2}(?:\.\d+)?$/.test(value)) return null;
  const parts = value.split(":");
  const seconds = Number(parts.pop());
  if (parts.length > 0 && seconds >= 60) return null;
  const minutes = parts.length ? Number(parts.pop()) : 0;
  if (parts.length > 0 && minutes >= 60) return null;
  const hours = parts.length ? Number(parts.pop()) : 0;
  const total = hours * 3600 + minutes * 60 + seconds;
  return Number.isFinite(total) ? total : null;
}

/** Formats seconds as H:MM:SS (or M:SS under an hour). */
export function formatTimecode(value: number): string {
  const total = Math.max(0, Math.round(value));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = String(total % 60).padStart(2, "0");
  return hours ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}` : `${minutes}:${seconds}`;
}
