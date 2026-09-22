import { readFileSync } from "node:fs";

const app = readFileSync(new URL("../src/App.svelte", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const backend = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const ciWorkflow = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
const releaseWorkflow = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const devConfig = JSON.parse(readFileSync(new URL("../src-tauri/tauri.dev.conf.json", import.meta.url), "utf8"));

const contracts: Array<[string, boolean]> = [
  ["DEV uses an isolated application identifier", devConfig.identifier === "dev.dean.container.dev"],
  ["camera detection ships visibly with an experimental label", !app.includes("{#if experimentalFeatures}") && app.includes("AUTO-DETECT CAMERA") && app.includes(">EXPERIMENTAL</em>")],
  ["DEV builds never query or offer the production updater", app.includes("updatesAllowedForVersion(appVersion)") && app.includes("{#if updaterEnabled}")],
  ["recovery uses a freshly authorized media URL", app.includes("recoveredMediaUrl(preparedMediaUrl)")],
  ["compact SmartCut layout has a 900px breakpoint", css.includes("@media(max-height:900px)")],
  ["compact SmartCut layout has a 700px breakpoint", css.includes("@media(max-height:700px)")],
  ["shared typography tokens are present", css.includes("--type-micro:") && css.includes("--type-label:")],
  ["application startup does not rewrite file associations", !backend.includes("register_project_file_association")],
  ["CI runs UI regression contracts", ciWorkflow.includes("pnpm test:ui")],
  ["release runs UI regression contracts", releaseWorkflow.includes("pnpm test:ui")],
  ["portable package includes FFmpeg license", releaseWorkflow.includes("FFmpeg-GPLv3.txt")],
  ["portable package includes face model license", releaseWorkflow.includes("face_detection_yunet-LICENSE.txt")],
];

for (const [name, passed] of contracts) {
  if (!passed) throw new Error(`UI contract failed: ${name}`);
}

console.log(`UI/release contracts: ${contracts.length} passed`);
