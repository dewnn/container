// Shape checks for project (.cproj) and recovery data before it is restored.
// A hand-edited, truncated or older file must be rejected with a clear
// message instead of breaking the editor halfway through a restore.

type Data = Record<string, unknown>;

const isObject = (value: unknown): value is Data => !!value && typeof value === "object" && !Array.isArray(value);
const isString = (value: unknown): value is string => typeof value === "string";
const isBoolean = (value: unknown): value is boolean => typeof value === "boolean";
const isNumber = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value);
const optional = (value: unknown, check: (value: unknown) => boolean) => value === undefined || check(value);
const nullable = (value: unknown, check: (value: unknown) => boolean) => value === null || check(value);
const arrayOf = (value: unknown, check: (value: unknown) => boolean, limit = 10_000) =>
  Array.isArray(value) && value.length <= limit && value.every(check);
const recordOf = (value: unknown, check: (value: unknown) => boolean) =>
  isObject(value) && Object.values(value).every(check);

function validTool(value: unknown): boolean {
  return isObject(value) && isString(value.id) && arrayOf(value.fields, field => isObject(field) && isString(field.key), 500);
}

function validMedia(value: unknown): boolean {
  return isObject(value) && isString(value.path) && value.path.length > 0 && isString(value.kind);
}

/** Toolbox editor state (`EditorSnapshot` in App.svelte). */
export function validToolboxSnapshot(value: unknown): boolean {
  if (!isObject(value)) return false;
  return validMedia(value.media)
    && isString(value.mediaUrl)
    && nullable(value.selected, validTool)
    && ["video", "audio", "image"].includes(value.activeKind as string)
    && isString(value.output)
    && optional(value.outputSettingsKey, isString)
    && optional(value.outputMode, mode => mode === "tool" || mode === "stack")
    && isString(value.renderedImageUrl)
    && recordOf(value.colorEnabled, isBoolean)
    && isBoolean(value.colorPreviewVisible)
    && arrayOf(value.textLayers, layer => isObject(layer) && isNumber(layer.id), 200)
    && nullable(value.activeTextId, isNumber)
    && nullable(value.qualityAnalysis, isObject)
    && recordOf(value.customNumberFields, isBoolean)
    && optional(value.mergeInputs, inputs => arrayOf(inputs, isString, 1000))
    && optional(value.processingStack, stack => arrayOf(stack, step => isObject(step) && isNumber(step.id), 200))
    && optional(value.stackQuality, isString)
    && optional(value.editingStackStepId, id => nullable(id, isNumber));
}

/** SmartCut workspace state (`HistorySnapshot` in SmartCutWorkspace.svelte). */
export function validSmartCutSession(value: unknown): boolean {
  if (!isObject(value)) return false;
  const numbers = ["threshold", "minSilence", "minSpeech", "minimumPause", "keepBeforeSpeech", "keepAfterSpeech"];
  const strings = ["preset", "autoSummary", "output", "exportFormat", "quality", "resolution", "analysisInput", "lastAnalyzedKey"];
  return numbers.every(key => isNumber(value[key]))
    && strings.every(key => isString(value[key]))
    && isBoolean(value.skipRemoved)
    && isBoolean(value.hasAnalyzed)
    && arrayOf(value.cuts, cut => isObject(cut) && isNumber(cut.start) && isNumber(cut.end) && isBoolean(cut.enabled), 20_000)
    && arrayOf(value.linkedTracks, track => isObject(track) && isString(track.path), 100);
}

/** Batch workspace state (`HistorySnapshot` in BatchWorkspace.svelte). */
export function validBatchSession(value: unknown): boolean {
  return isObject(value)
    && validTool(value.selected)
    && arrayOf(value.items, item => isObject(item) && isString(item.path), 5000)
    && isBoolean(value.recursive);
}

/**
 * Checks the parts of a recovery or project session before they are restored.
 * The Toolbox state is required to be well formed. An outdated or damaged
 * SmartCut or Batch part is dropped instead, so the rest of the project still
 * opens. Returns false when the session must be rejected.
 */
export function sanitizeSessionParts(value: Data): boolean {
  if (!nullable(value.toolbox ?? null, validToolboxSnapshot)) return false;
  if (!nullable(value.autocut ?? null, validSmartCutSession)) value.autocut = null;
  if (!nullable(value.batch ?? null, validBatchSession)) value.batch = null;
  return true;
}
