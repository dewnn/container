import { readFileSync } from "node:fs";

const app = readFileSync(new URL("../src/App.svelte", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const backend = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const kickLogo = readFileSync(new URL("../src/assets/kick-mark.svg", import.meta.url), "utf8");
const twitchLogo = readFileSync(new URL("../src/assets/twitch-mark.svg", import.meta.url), "utf8");
const tools = readFileSync(new URL("../src/lib/tools.ts", import.meta.url), "utf8");
const socialGeometry = readFileSync(new URL("../src/lib/socialTagGeometry.ts", import.meta.url), "utf8");
const ciWorkflow = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
const releaseWorkflow = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const stableConfig = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const devConfig = JSON.parse(readFileSync(new URL("../src-tauri/tauri.dev.conf.json", import.meta.url), "utf8"));
const mediaLoad = app.slice(app.indexOf("async function loadMedia("), app.indexOf("function closeMedia()"));

const contracts: Array<[string, boolean]> = [
  ["Windows installer publisher is dewn", stableConfig.bundle.publisher === "dewn"],
  ["DEV uses an isolated application identifier", devConfig.identifier === "dev.dean.container.dev"],
  ["camera detection ships visibly with an experimental label", !app.includes("{#if experimentalFeatures}") && app.includes("AUTO-DETECT CAMERA") && app.includes(">EXPERIMENTAL</em>")],
  ["DEV builds never query or offer the production updater", app.includes("updatesAllowedForVersion(appVersion)") && app.includes("{#if updaterEnabled}")],
  ["recovery uses a freshly authorized media URL", app.includes("recoveredMediaUrl(preparedMediaUrl)")],
  ["compact SmartCut layout has a 900px breakpoint", css.includes("@media(max-height:900px)")],
  ["compact SmartCut layout has a 700px breakpoint", css.includes("@media(max-height:700px)")],
  ["shared typography tokens are present", css.includes("--type-micro:") && css.includes("--type-label:")],
  ["application startup does not rewrite file associations", !backend.includes("register_project_file_association")],
  ["missing project media can be relinked without discarding the project", app.includes('invoke<boolean>("project_media_available"') && app.includes('restoredPath=replacement') && app.includes('saved.mediaPath=restoredPath') && app.includes('Source file not found')],
  ["startup and drag-drop files share project-aware routing", app.includes('async function openIncomingPath') && app.includes('await openIncomingPath(path)') && app.includes('if (path) void openIncomingPath(path)')],
  ["failed media loads retain the open session", app.includes('if(!await loadMedia(restoredPath))return') && mediaLoad.includes('return false;') && !mediaLoad.includes('media = null') && !mediaLoad.includes('selected = null')],
  ["startup recovery is ordered after both startup checks", app.includes('const [path,interrupted]=await Promise.all(') && app.includes('if(mediaLoadId!==initialMediaLoadId||media||restoringSession)return') && app.includes('startupAction(path,interrupted)')],
  ["Social Tag uses the supplied Kick SVG mark", kickLogo.includes('M278.26 216.86H646.7v245.62') && app.includes('import kickMark from "./assets/kick-mark.svg"')],
  ["Social Tag uses the supplied Twitch SVG mark", twitchLogo.includes('M5.7 0L1.4 10.985V55.88') && app.includes('import twitchMark from "./assets/twitch-mark.svg"')],
  ["Social Tag defaults to 36px and Twitch purple", tools.includes('number("social_tag_size", "Social Tag size", 36,') && twitchLogo.includes('#9146FF') && css.includes('background:#9146ff') && backend.includes('"0x9146ff"')],
  ["Social Tag platform and style selectors are independent", app.includes('toolValue("social_tag_platform")') && app.includes('value="twitch">Twitch</option>')],
  ["Social Tag styles have distinct fixed camera-relative anchors", app.includes('socialTagGeometry({width,height') && socialGeometry.includes('cameraBottom-side/2') && socialGeometry.includes('anchorX-totalWidth/2') && !app.includes('social_tag_position')],
  ["plain Social Tag remains legible over white video", css.includes('-webkit-text-stroke:.055em #050505') && backend.includes('borderw=3:bordercolor=black@0.95')],
  ["CI runs UI regression contracts", ciWorkflow.includes("pnpm test:ui")],
  ["release runs UI regression contracts", releaseWorkflow.includes("pnpm test:ui")],
  ["portable package includes FFmpeg license", releaseWorkflow.includes("FFmpeg-GPLv3.txt")],
  ["portable package includes face model license", releaseWorkflow.includes("face_detection_yunet-LICENSE.txt")],
];

for (const [name, passed] of contracts) {
  if (!passed) throw new Error(`UI contract failed: ${name}`);
}

console.log(`UI/release contracts: ${contracts.length} passed`);
