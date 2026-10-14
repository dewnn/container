import { recoveredMediaUrl } from "../../src/lib/recovery.ts";

const fresh = "http://asset.localhost/C%3A/video.mp4";
const restored = recoveredMediaUrl(fresh, 1234);
if (restored !== `${fresh}?recovery=1234`) {
  throw new Error(`Unexpected recovered URL: ${restored}`);
}

const queried = recoveredMediaUrl(`${fresh}?prepared=1`, 5678);
if (queried !== `${fresh}?prepared=1&recovery=5678`) {
  throw new Error(`Existing query was not preserved: ${queried}`);
}

if (restored === fresh || queried.includes("??")) {
  throw new Error("Recovery must create a fresh, valid media resource identity.");
}

console.log("recovery media source scenarios: 3 passed");

import { sanitizeSessionParts, validToolboxSnapshot } from "../../src/lib/sessionValidation.ts";
import { turkishBackendError } from "../../src/lib/backendErrors.ts";

const toolbox = {
  media: { path: "C:/video.mp4", kind: "video" }, mediaUrl: fresh, selected: { id: "cut", fields: [{ key: "start", value: 0 }] },
  activeKind: "video", output: "", renderedImageUrl: "", colorEnabled: { contrast: false }, colorPreviewVisible: true,
  textLayers: [{ id: 1, text: "Hi" }], activeTextId: 1, qualityAnalysis: null, customNumberFields: {}, processingStack: [], editingStackStepId: null,
};
if (!validToolboxSnapshot(toolbox)) throw new Error("A valid Toolbox snapshot was rejected.");
for (const broken of [{ ...toolbox, textLayers: "x" }, { ...toolbox, colorEnabled: undefined }, { ...toolbox, media: { path: "" } }, { ...toolbox, activeKind: "pdf" }]) {
  if (sanitizeSessionParts({ toolbox: broken })) throw new Error("A damaged Toolbox snapshot was accepted.");
}
const session: Record<string, unknown> = { toolbox, autocut: { cuts: "broken" }, batch: { selected: { id: "encode", fields: [] }, items: [{ path: "C:/a.mp4" }], recursive: false } };
if (!sanitizeSessionParts(session) || session.autocut !== null || session.batch === null) {
  throw new Error("A damaged SmartCut part must be dropped while valid parts are kept.");
}
if (!sanitizeSessionParts({ toolbox: null, autocut: undefined, batch: undefined })) throw new Error("Empty optional parts must be accepted.");

if (turkishBackendError("Threshold must be between 0.05 and 0.95.") !== "Threshold değeri 0.05 ile 0.95 arasında olmalı.") throw new Error("Range errors must be translated.");
if (!turkishBackendError("Another media job is already running.")?.includes("Başka bir")) throw new Error("Known backend errors must be translated.");
if (turkishBackendError("Some brand new message") !== null) throw new Error("Unknown backend errors must fall back to the original text.");

console.log("project validation and backend error scenarios: 9 passed");

import { playerAction, isFormFieldTarget, keyOwnedByControl } from "../../src/lib/playerShortcuts.ts";
import { parseTimecode, formatTimecode } from "../../src/lib/timecode.ts";

const press = (key: string, extra: Record<string, unknown> = {}) => ({ key, code: key === " " ? "Space" : `Key${key.toUpperCase()}`, ctrlKey: false, altKey: false, metaKey: false, shiftKey: false, ...extra }) as unknown as KeyboardEvent;
const expectAction = (event: KeyboardEvent, expected: unknown, marks = false) => {
  const actual = playerAction(event, { marks });
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`Shortcut ${event.key}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
};
expectAction(press(" "), { kind: "toggle" });
expectAction(press("k"), { kind: "toggle" });
expectAction(press("j"), { kind: "seek", seconds: -10 });
expectAction(press("L", { shiftKey: true }), { kind: "seek", seconds: 10 });
expectAction(press("ArrowLeft", { code: "ArrowLeft", shiftKey: true }), { kind: "seek", seconds: -1 });
expectAction(press(".", { code: "Period" }), { kind: "frame", direction: 1 });
expectAction(press("Home", { code: "Home" }), { kind: "start" });
expectAction(press("i"), null);
expectAction(press("i"), { kind: "mark", edge: "start" }, true);
expectAction(press("ArrowRight", { code: "ArrowRight", ctrlKey: true }), null);
if (!isFormFieldTarget({ tagName: "INPUT", type: "text" } as unknown as EventTarget)) throw new Error("Text fields must keep their keys.");
if (isFormFieldTarget({ tagName: "INPUT", type: "range" } as unknown as EventTarget)) throw new Error("Sliders must keep Space for playback.");
const owned = (tagName: string, type: string, key: string, code = "") => keyOwnedByControl({ target: { tagName, type }, key, code } as unknown as KeyboardEvent);
if (!owned("INPUT", "checkbox", " ", "Space")) throw new Error("Space must tick a focused checkbox, not play.");
if (!owned("BUTTON", "", "Enter")) throw new Error("Enter must press a focused button.");
if (owned("INPUT", "range", " ", "Space")) throw new Error("Space on a slider must still play.");
if (!owned("INPUT", "range", "ArrowLeft")) throw new Error("Arrows must move a focused slider.");
if (owned("INPUT", "checkbox", "j", "KeyJ")) throw new Error("J must still seek from a checkbox.");

for (const [text, seconds] of [["5025", 5025], ["83:20", 5000], ["1:23:45", 5025], ["0:05.5", 5.5], ["1:23,5", 83.5]] as const) {
  if (parseTimecode(text) !== seconds) throw new Error(`parseTimecode(${text}) should be ${seconds}`);
}
for (const text of ["", "1:75", "1:2:3:4", "abc", "-5", "1:60:00"]) {
  if (parseTimecode(text) !== null) throw new Error(`parseTimecode(${text}) should be rejected`);
}
if (formatTimecode(5025) !== "1:23:45" || formatTimecode(75) !== "1:15") throw new Error("formatTimecode output changed");

console.log("shortcut and timecode scenarios: 19 passed");
