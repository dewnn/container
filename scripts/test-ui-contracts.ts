import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";

const app = readFileSync(new URL("../src/App.svelte", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const backend = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const kickLogo = readFileSync(new URL("../src/assets/kick-mark.svg", import.meta.url), "utf8");
const twitchLogo = readFileSync(new URL("../src/assets/twitch-mark.svg", import.meta.url), "utf8");
const tools = readFileSync(new URL("../src/lib/tools.ts", import.meta.url), "utf8");
const socialGeometry = readFileSync(new URL("../src/lib/socialTagGeometry.ts", import.meta.url), "utf8");
const ciWorkflow = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
const releaseWorkflow = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const downloader = readFileSync(new URL("../src/lib/DownloaderWorkspace.svelte", import.meta.url), "utf8");
const buildScript = readFileSync(new URL("../src-tauri/build.rs", import.meta.url), "utf8");
const installerHooks = readFileSync(new URL("../src-tauri/windows/hooks.nsh", import.meta.url), "utf8");
const darkBrandSource = readFileSync(new URL("../src-tauri/icons/container.svg", import.meta.url), "utf8");
const lightBrandSource = readFileSync(new URL("../src-tauri/icons/container-light.svg", import.meta.url), "utf8");
const darkMark = readFileSync(new URL("../public/mark-dark.svg", import.meta.url), "utf8");
const lightMark = readFileSync(new URL("../public/mark-light.svg", import.meta.url), "utf8");
const appIcon = readFileSync(new URL("../src-tauri/icons/icon.ico", import.meta.url));
const projectIcon = readFileSync(new URL("../src-tauri/icons/project.ico", import.meta.url));
const installerIcon = readFileSync(new URL("../src-tauri/windows/setup-dark.ico", import.meta.url));
const withoutBackground = (svg: string) => svg.replace(/^  <rect width="1254" height="1254" fill="#[0-9A-Fa-f]{6}"\/>\r?\n/m, "");
const stableConfig = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const devConfig = JSON.parse(readFileSync(new URL("../src-tauri/tauri.dev.conf.json", import.meta.url), "utf8"));
const desktopPermissions = JSON.parse(readFileSync(new URL("../src-tauri/capabilities/default.json", import.meta.url), "utf8")).permissions;
const mediaLoad = app.slice(app.indexOf("async function loadMedia("), app.indexOf("function closeMedia()"));
const markRoot = new URL("../src-tauri/resources/social-tags/", import.meta.url);
const markManifest = JSON.parse(readFileSync(new URL("manifest.json", markRoot), "utf8")) as Record<string, {source: string; sourceSha256: string; pngSha256: string}>;
const marksMatchSources = ["kick-plain", "kick-boxed", "twitch"].every(name => {
  const entry = markManifest[name];
  if (!entry) return false;
  const source = readFileSync(new URL(`../src/assets/${entry.source}`, import.meta.url), "utf8").replace(/\r\n/g, "\n");
  const png = readFileSync(new URL(`${name}.png`, markRoot));
  return createHash("sha256").update(source).digest("hex") === entry.sourceSha256
    && createHash("sha256").update(png).digest("hex") === entry.pngSha256;
});

const contracts: Array<[string, boolean]> = [
  ["all UI workspaces use the new theme-aware brand mark", app.includes('src="/mark-dark.svg"') && app.includes('src="/mark-light.svg"') && downloader.includes('src="/mark-dark.svg"') && downloader.includes('src="/mark-light.svg"')],
  ["UI marks keep the exact source artwork without a boxed background", withoutBackground(darkBrandSource) === darkMark && withoutBackground(lightBrandSource) === lightMark],
  ["application, project and installer ICOs are identical", appIcon.equals(projectIcon) && appIcon.equals(installerIcon)],
  ["Windows icon changes retrigger resource compilation", buildScript.includes('cargo:rerun-if-changed=icons/icon.ico')],
  ["Windows upgrades refresh the cached desktop and Start menu icon", installerHooks.includes('container-brand-${VERSION}.ico') && installerHooks.includes('CreateShortCut "$DESKTOP\\${PRODUCTNAME}.lnk"') && installerHooks.includes('CreateShortCut "$SMPROGRAMS\\${PRODUCTNAME}.lnk"') && installerHooks.includes('SHChangeNotify(i 0x08000000')],
  ["closing hides the editor in the tray and Exit really terminates", backend.includes('api.prevent_close()') && backend.includes('window.hide()') && backend.includes('"tray-exit" => app.exit(0)')],
  ["tray update action uses the existing updater UI and DEV keeps it hidden", backend.includes('"tray-updates"') && backend.includes('app.emit("tray-check-updates", ())') && backend.includes('if is_development_build()') && app.includes('listen("tray-check-updates",()=>{void checkForUpdates(true)})')],
  ["tray menu follows the selected TR/EN language", app.includes('invoke("set_tray_language",{language:next})') && backend.includes('fn set_tray_language(language: &str')],
  ["release packaging chooses the exact current executable and installer", releaseWorkflow.includes("Get-Item -LiteralPath 'src-tauri/target/release/container-studio.exe'") && releaseWorkflow.includes('Filter "CONTAINER_${version}_x64-setup.exe"')],
  ["native confirmation dialogs have their required IPC permission", desktopPermissions.includes("dialog:allow-message")],
  ["export logo assets match the preview SVGs (regenerate with scripts/generate-social-tag-marks.mjs)", marksMatchSources],
  ["Windows installer publisher is dewn", stableConfig.bundle.publisher === "dewn"],
  ["DEV uses an isolated application identifier", devConfig.identifier === "dev.dean.container.dev"],
  ["camera detection ships visibly with a localized experimental label", !app.includes("{#if experimentalFeatures}") && app.includes("AUTO-DETECT CAMERA") && app.includes('"DENEYSEL":"EXPERIMENTAL"')],
  ["DEV builds never query or offer the production updater", app.includes("updatesAllowedForVersion(appVersion)") && app.includes("{#if updaterEnabled}")],
  ["recovery uses a freshly authorized media URL", app.includes("recoveredMediaUrl(preparedMediaUrl)")],
  ["compact SmartCut layout has a 900px breakpoint", css.includes("@media(max-height:900px)")],
  ["compact SmartCut layout has a 700px breakpoint", css.includes("@media(max-height:700px)")],
  ["shared typography tokens are present", css.includes("--type-micro:") && css.includes("--type-label:")],
  ["application startup does not rewrite file associations", !backend.includes("register_project_file_association")],
  ["missing project media can be relinked without discarding the project", app.includes('invoke<boolean>("project_media_available"') && app.includes('restoredPath=replacement') && app.includes('saved.mediaPath=restoredPath') && app.includes('Source file not found')],
  ["startup and drag-drop files share project-aware routing", app.includes('async function openIncomingPath') && app.includes('await openIncomingPath(path)') && app.includes('if (path) void openIncomingPath(path)')],
  ["failed media loads retain the open session", app.includes('if(!await loadMedia(restoredPath,true))return') && mediaLoad.includes('return false;') && !mediaLoad.includes('media = null') && !mediaLoad.includes('selected = null')],
  ["startup recovery is ordered after both startup checks", app.includes('const [path,interrupted]=await Promise.all(') && app.includes('if(mediaLoadId!==initialMediaLoadId||media||restoringSession)return') && app.includes('startupAction(path,interrupted)')],
  ["Social Tag uses the supplied Kick SVG mark", kickLogo.includes('M278.26 216.86H646.7v245.62') && app.includes('import kickMark from "./assets/kick-mark.svg"')],
  ["Social Tag uses the supplied Twitch SVG mark", twitchLogo.includes('M5.7 0L1.4 10.985V55.88') && app.includes('import twitchMark from "./assets/twitch-mark.svg"')],
  ["Social Tag defaults to 36px and Twitch purple", tools.includes('number("social_tag_size", "Social Tag size", 36,') && twitchLogo.includes('#9146FF') && css.includes('background:#9146ff') && backend.includes('"0x9146ff"')],
  ["Social Tag platform and style selectors are independent", app.includes('toolValue("social_tag_platform")') && app.includes('value="twitch">Twitch</option>')],
  ["Social Tag positions keep independent style defaults and camera-relative anchors", tools.includes('"social_tag_boxed_position", "Boxed Social Tag position", "left"') && tools.includes('"social_tag_plain_position", "Plain Social Tag position", "center"') && app.includes('socialTagGeometry({width,height') && socialGeometry.includes('cameraLeft+cameraWidth/2') && socialGeometry.includes('cameraRight') && backend.includes('"social_tag_boxed_position"') && backend.includes('"social_tag_plain_position"')],
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
