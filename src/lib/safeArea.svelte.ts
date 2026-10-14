// Platform UI safe areas for vertical (9:16) output. The zones are the parts of
// a 1080x1920 frame that the app interface usually covers: top tabs, the
// caption/username block at the bottom and the action buttons on the right.
// They are approximate; each platform's layout varies slightly by device.

export type SafeAreaPlatform = "off" | "tiktok" | "reels" | "shorts";

/** Covered bands as fractions of the output frame. */
export interface SafeAreaZones { top: number; bottom: number; left: number; right: number }

export const SAFE_AREAS: Record<Exclude<SafeAreaPlatform, "off">, { name: string; zones: SafeAreaZones }> = {
  tiktok: { name: "TikTok", zones: { top: 0.07, bottom: 0.23, left: 0.055, right: 0.12 } },
  reels: { name: "Instagram Reels", zones: { top: 0.115, bottom: 0.22, left: 0.055, right: 0.11 } },
  shorts: { name: "YouTube Shorts", zones: { top: 0.08, bottom: 0.2, left: 0.055, right: 0.13 } },
};

const STORAGE_KEY = "container-safe-area-v1";
const platforms: SafeAreaPlatform[] = ["off", "tiktok", "reels", "shorts"];

function stored(): SafeAreaPlatform {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    return platforms.includes(value as SafeAreaPlatform) ? (value as SafeAreaPlatform) : "off";
  } catch {
    return "off";
  }
}

/** The chosen platform is a preview preference, shared by the controls and the overlay. */
export const safeArea = $state({ platform: stored() });

export function setSafeAreaPlatform(platform: SafeAreaPlatform) {
  safeArea.platform = platform;
  try { localStorage.setItem(STORAGE_KEY, platform); } catch { /* preview preference only */ }
}

/** The area left uncovered, as fractions of the output frame. */
export function safeRectangle(zones: SafeAreaZones) {
  return { x: zones.left, y: zones.top, width: 1 - zones.left - zones.right, height: 1 - zones.top - zones.bottom };
}
