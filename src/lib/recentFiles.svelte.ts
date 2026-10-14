// Recently opened media and project files for the start screen.

export type RecentKind = "video" | "audio" | "image" | "project";
export interface RecentFile { path: string; kind: RecentKind; openedAt: number }

const STORAGE_KEY = "container-recent-files-v1";
const LIMIT = 6;
const AUDIO = new Set(["mp3", "wav", "flac", "aac", "m4a", "ogg", "opus", "wma", "aiff", "aif", "alac", "mka"]);
const IMAGE = new Set(["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff", "heic", "heif", "avif"]);

export function recentKind(path: string): RecentKind {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  if (extension === "cproj") return "project";
  if (AUDIO.has(extension)) return "audio";
  if (IMAGE.has(extension)) return "image";
  return "video";
}

function valid(value: unknown): value is RecentFile {
  const item = value as Partial<RecentFile> | null;
  return !!item && typeof item.path === "string" && item.path.length > 0 && typeof item.openedAt === "number"
    && ["video", "audio", "image", "project"].includes(item.kind as string);
}

export class RecentFiles {
  items: RecentFile[] = $state([]);

  load() {
    try {
      const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
      this.items = Array.isArray(saved) ? saved.filter(valid).slice(0, LIMIT) : [];
    } catch {
      this.items = [];
    }
  }

  private save(next: RecentFile[]) {
    this.items = next;
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(next)); } catch { /* the list is a convenience */ }
  }

  add(path: string) {
    const key = path.toLowerCase();
    const rest = this.items.filter(item => item.path.toLowerCase() !== key);
    this.save([{ path, kind: recentKind(path), openedAt: Date.now() }, ...rest].slice(0, LIMIT));
  }

  remove(path: string) {
    this.save(this.items.filter(item => item.path !== path));
  }

  clear() {
    this.save([]);
  }
}
