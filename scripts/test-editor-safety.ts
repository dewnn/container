import { isTextEditingTarget } from "../src/lib/editorInput.ts";
import { startupAction } from "../src/lib/startupRecovery.ts";
import { localizedTool, preserveToolValues, tools } from "../src/lib/tools.ts";

function assert(condition: boolean, message: string) {
  if (!condition) throw new Error(message);
}

const gif = tools.find(tool => tool.id === "gif")!;
const edited = localizedTool(gif, "en");
edited.fields.find(field => field.key === "start")!.value = 12.5;
edited.fields.find(field => field.key === "end")!.value = 19.75;
const translated = preserveToolValues(localizedTool(gif, "tr"), edited);
assert(translated.title === "GIF Oluştur", "The tool title should follow the new language.");
assert(translated.fields.find(field => field.key === "start")?.value === 12.5, "The IN point was lost on language change.");
assert(translated.fields.find(field => field.key === "end")?.value === 19.75, "The OUT point was lost on language change.");
assert(edited.title === "GIF Maker", "Changing language must not mutate the previous tool.");

const clipper = tools.find(tool => tool.id === "clipper")!;
const named = localizedTool(clipper, "en");
named.fields.find(field => field.key === "social_tag_username")!.value = "example_user";
assert(preserveToolValues(localizedTool(clipper, "tr"), named).fields.find(field => field.key === "social_tag_username")?.value === "example_user", "The Social Tag name was lost on language change.");

for (const tag of ["INPUT", "SELECT", "TEXTAREA"]) {
  assert(isTextEditingTarget(tag, false), `${tag} must keep its native undo and keyboard handling.`);
}
assert(isTextEditingTarget("SPAN", true), "Contenteditable descendants must keep native undo.");
assert(!isTextEditingTarget("DIV", false), "The editor must still receive shortcuts outside text fields.");

assert(startupAction("C:/video.mp4", true) === "open-path", "An explicit startup file must take precedence over old recovery.");
assert(startupAction(null, true) === "offer-recovery", "Interrupted work should be offered when no file was opened.");
assert(startupAction(null, false) === "discard-recovery", "A clean previous exit must discard stale recovery.");

console.log("editor safety scenarios: 13 passed");
