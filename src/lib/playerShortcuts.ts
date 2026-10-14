// Keyboard shortcuts shared by the Toolbox and SmartCut players.

export type PlayerAction =
  | { kind: "toggle" }
  | { kind: "seek"; seconds: number }
  | { kind: "frame"; direction: 1 | -1 }
  | { kind: "start" }
  | { kind: "end" }
  | { kind: "mark"; edge: "start" | "end" };

// Sliders, checkboxes and buttons keep the other shortcuts: after clicking the
// seek bar, Space still plays. Typing fields and menus own all their keys.
const NON_TEXT_INPUTS = new Set(["range", "checkbox", "radio", "button", "submit", "color", "file"]);

/** True when the key press is typing into a field or choosing in a menu. */
export function isFormFieldTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  if (element.isContentEditable || element.tagName === "TEXTAREA" || element.tagName === "SELECT") return true;
  return element.tagName === "INPUT" && !NON_TEXT_INPUTS.has((element as HTMLInputElement).type);
}

const SLIDER_KEYS = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);
const PRESSABLE_INPUTS = new Set(["checkbox", "radio", "button", "submit", "color", "file"]);

/** A focused control keeps the keys it needs: a slider uses arrows, Home and
 * End; a checkbox or button uses Space and Enter. */
export function keyOwnedByControl(event: KeyboardEvent): boolean {
  const element = event.target as HTMLElement | null;
  if (!element) return false;
  const pressKey = event.code === "Space" || event.key === "Enter";
  if (element.tagName === "BUTTON" || element.tagName === "A") return pressKey;
  if (element.tagName !== "INPUT") return false;
  const type = (element as HTMLInputElement).type;
  if (type === "range") return SLIDER_KEYS.has(event.key);
  return PRESSABLE_INPUTS.has(type) && pressKey;
}

/** Maps a key press to a player action, or null when it is not a shortcut. */
export function playerAction(event: KeyboardEvent, options: { marks: boolean }): PlayerAction | null {
  if (event.ctrlKey || event.altKey || event.metaKey) return null;
  const key = event.key.toLowerCase();
  if (event.code === "Space" || key === "k") return { kind: "toggle" };
  if (key === "j") return { kind: "seek", seconds: -10 };
  if (key === "l") return { kind: "seek", seconds: 10 };
  if (event.key === "ArrowLeft") return { kind: "seek", seconds: event.shiftKey ? -1 : -5 };
  if (event.key === "ArrowRight") return { kind: "seek", seconds: event.shiftKey ? 1 : 5 };
  if (event.key === ",") return { kind: "frame", direction: -1 };
  if (event.key === ".") return { kind: "frame", direction: 1 };
  if (event.key === "Home") return { kind: "start" };
  if (event.key === "End") return { kind: "end" };
  if (options.marks && key === "i") return { kind: "mark", edge: "start" };
  if (options.marks && key === "o") return { kind: "mark", edge: "end" };
  return null;
}

export interface ShortcutGroup { title: [string, string]; items: { keys: string[]; label: [string, string] }[] }

/** Shown in the shortcuts dialog. Labels are [Turkish, English]. */
export const SHORTCUT_GROUPS: ShortcutGroup[] = [
  {
    title: ["Oynatıcı", "Player"],
    items: [
      { keys: ["Space", "K"], label: ["Oynat / durdur", "Play / pause"] },
      { keys: ["J"], label: ["10 saniye geri", "Back 10 seconds"] },
      { keys: ["L"], label: ["10 saniye ileri", "Forward 10 seconds"] },
      { keys: ["←", "→"], label: ["5 saniye geri / ileri", "Back / forward 5 seconds"] },
      { keys: ["Shift + ←", "Shift + →"], label: ["1 saniye geri / ileri", "Back / forward 1 second"] },
      { keys: [",", "."], label: ["Bir kare geri / ileri", "One frame back / forward"] },
      { keys: ["Home", "End"], label: ["Başa / sona git", "Go to start / end"] },
    ],
  },
  {
    title: ["Zaman çizelgesi", "Timeline"],
    items: [
      { keys: ["I"], label: ["Başlangıcı oynatıcı konumuna koy (Cut, GIF, Kaplama)", "Set start at the playhead (Cut, GIF, Overlay)"] },
      { keys: ["O"], label: ["Bitişi oynatıcı konumuna koy (Cut, GIF, Kaplama)", "Set end at the playhead (Cut, GIF, Overlay)"] },
    ],
  },
  {
    title: ["Düzenleme", "Editing"],
    items: [
      { keys: ["Ctrl + Z"], label: ["Geri al", "Undo"] },
      { keys: ["Ctrl + Shift + Z"], label: ["Yinele", "Redo"] },
      { keys: ["Shift"], label: ["Kaydırıcıyı sürüklerken basılı tut: ince ayar", "Hold while dragging a slider: fine adjustment"] },
      { keys: ["?"], label: ["Bu listeyi aç", "Open this list"] },
    ],
  },
];
