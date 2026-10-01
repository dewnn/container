export const DEFAULT_ACCENT = { hue: 213, saturation: 94 };
const key = "container-accent";

// Keep accent text readable on white panels, including yellow and lime hues.
function lightThemeTone(hue: number, saturation: number) {
  const s = saturation / 100;
  for (let tone = 40; tone >= 16; tone--) {
    const l = tone / 100;
    const a = s * Math.min(l, 1 - l);
    const channel = (n: number) => {
      const k = (n + hue / 30) % 12;
      const c = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
      return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4;
    };
    const luminance = .2126 * channel(0) + .7152 * channel(8) + .0722 * channel(4);
    if (1.05 / (luminance + .05) >= 5.5) return tone;
  }
  return 16;
}

export function readAccent(): typeof DEFAULT_ACCENT {
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? "null");
    if (value && Number.isFinite(value.hue) && Number.isFinite(value.saturation)
      && value.hue >= 0 && value.hue < 360 && value.saturation >= 55 && value.saturation <= 100) {
      return { hue: value.hue, saturation: value.saturation };
    }
  } catch { /* Corrupt or unavailable preferences must not prevent startup. */ }
  return { ...DEFAULT_ACCENT };
}

export function applyAccent(hue: number, saturation: number, persist = true) {
  if (!Number.isFinite(hue) || !Number.isFinite(saturation)) return;
  const value = { hue: ((hue % 360) + 360) % 360, saturation: Math.max(55, Math.min(100, saturation)) };
  document.documentElement.style.setProperty("--accent-hue", String(value.hue));
  document.documentElement.style.setProperty("--accent-saturation", `${value.saturation}%`);
  document.documentElement.style.setProperty("--accent-lightness-light", `${lightThemeTone(value.hue, value.saturation)}%`);
  if (persist) {
    try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* Live color still works without storage. */ }
  }
}
