/**
 * Produces a source URL for a recovered media element without reusing the
 * previous WebView session's failed/cached resource identity.
 */
export function recoveredMediaUrl(freshAuthorizedUrl: string, stamp = Date.now()): string {
  const separator = freshAuthorizedUrl.includes("?") ? "&" : "?";
  return `${freshAuthorizedUrl}${separator}recovery=${stamp}`;
}
