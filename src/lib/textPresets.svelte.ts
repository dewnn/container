// Saved text presets (layer appearance + position), kept in localStorage.
import type { TextLayer } from "./editorTypes";

export interface TextPreset { id: string; name: string; sourceWidth?: number; layer: TextLayer }

const STORAGE_KEY = "container-text-presets-v1";
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;

function validPreset(value: unknown): value is TextPreset {
  if (!value || typeof value !== "object") return false;
  const p = value as Record<string, unknown>;
  if (typeof p.id !== "string" || typeof p.name !== "string" || !p.layer || typeof p.layer !== "object") return false;
  const layer = p.layer as Record<string, unknown>;
  return typeof layer.text === "string" && typeof layer.font_path === "string"
    && ["x", "y", "size", "opacity", "outline", "shadow", "background_opacity", "background_padding"].every(key => Number.isFinite(layer[key]))
    && ["color", "outline_color", "shadow_color", "background_color"].every(key => typeof layer[key] === "string" && /^#[0-9a-f]{6}$/i.test(layer[key]))
    && ["left", "center", "right"].includes(layer.align as string);
}

export class TextPresetStore {
  list: TextPreset[] = $state([]);
  /** The name field: used for new presets and for renaming. */
  name = $state("");
  selected = $state("");
  busy = $state(false);

  constructor(
    private readonly language: () => "tr" | "en",
    private readonly notify: (message: string) => void,
  ) {}

  private tr(turkish: string, english: string) {
    return this.language() === "tr" ? turkish : english;
  }

  load() {
    try {
      const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
      if (Array.isArray(saved)) this.list = saved.filter(validPreset);
    } catch {
      this.list = [];
    }
  }

  find(id: string) {
    return this.list.find(preset => preset.id === id);
  }

  private persist(next: TextPreset[]) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      this.list = next;
      return true;
    } catch {
      this.notify(this.tr("Yazı preseti kaydedilemedi. Depolama alanını kontrol et.", "Could not save text presets. Check available storage."));
      return false;
    }
  }

  save(layer: TextLayer | null, sourceWidth: number | undefined) {
    const name = this.name.trim();
    if (!layer || !name) return;
    const existing = this.list.find(preset => preset.name.toLocaleLowerCase() === name.toLocaleLowerCase());
    if (existing) {
      this.selected = existing.id;
      this.notify(this.tr("Bu ad zaten var. Seçili preseti güncelle veya farklı bir ad kullan.", "This name already exists. Update the selected preset or use another name."));
      return;
    }
    const preset = { id: crypto.randomUUID(), name, sourceWidth, layer: clone(layer) };
    if (this.persist([...this.list, preset])) {
      this.selected = preset.id;
      this.name = "";
      this.notify(this.tr("Yazı preseti kaydedildi.", "Text preset saved."));
    }
  }

  remove() {
    if (this.persist(this.list.filter(preset => preset.id !== this.selected))) this.selected = "";
  }

  update(layer: TextLayer | null, sourceWidth: number | undefined) {
    if (!layer || !this.selected) return;
    const next = this.list.map(preset => preset.id === this.selected ? { ...preset, sourceWidth, layer: clone(layer) } : preset);
    if (this.persist(next)) this.notify(this.tr("Preset güncellendi.", "Preset updated."));
  }

  rename() {
    const name = this.name.trim();
    if (!name || !this.selected) return;
    if (this.list.some(preset => preset.id !== this.selected && preset.name.toLocaleLowerCase() === name.toLocaleLowerCase())) {
      this.notify(this.tr("Bu ad zaten kullanılıyor. Başka bir ad gir.", "This name is already used. Enter another name."));
      return;
    }
    if (this.persist(this.list.map(preset => preset.id === this.selected ? { ...preset, name } : preset))) {
      this.name = "";
      this.notify(this.tr("Presetin adı değiştirildi.", "Preset renamed."));
    }
  }
}
