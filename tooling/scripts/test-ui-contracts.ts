import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tools as toolDefinitions, localizedTool } from "../../src/lib/tools.ts";

for (const tool of toolDefinitions) {
  const localized=localizedTool(tool,"en");
  const texts=[localized.title,localized.category,localized.description,localized.detail,...localized.fields.flatMap(field=>[field.label,field.hint??"",...(field.options??[]).map(option=>option.label)])];
  if(texts.some(text=>/[ğüşİıĞÜŞ]/u.test(text)))throw new Error(`Turkish text leaked into English tool: ${tool.id}`);
}
const interpolation=toolDefinitions.find(tool=>tool.id==="interpolation")!;
if(!localizedTool(interpolation,"tr").fields[0].hint?.includes("Dosya açıldığında"))throw new Error("Interpolation Turkish hint missing");

const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
const css = readFileSync(new URL("../../src/styles.css", import.meta.url), "utf8");
const backend = readFileSync(new URL("../../src-tauri/src/lib.rs", import.meta.url), "utf8");
const kickLogo = readFileSync(new URL("../../src/assets/kick-mark.svg", import.meta.url), "utf8");
const twitchLogo = readFileSync(new URL("../../src/assets/twitch-mark.svg", import.meta.url), "utf8");
const tools = readFileSync(new URL("../../src/lib/tools.ts", import.meta.url), "utf8");
const socialGeometry = readFileSync(new URL("../../src/lib/socialTagGeometry.ts", import.meta.url), "utf8");
const ciWorkflow = readFileSync(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
const releaseWorkflow = readFileSync(new URL("../../.github/workflows/release.yml", import.meta.url), "utf8");
const downloader = readFileSync(new URL("../../src/lib/DownloaderWorkspace.svelte", import.meta.url), "utf8");
const buildScript = readFileSync(new URL("../../src-tauri/build.rs", import.meta.url), "utf8");
const installerHooks = readFileSync(new URL("../../src-tauri/windows/hooks.nsh", import.meta.url), "utf8");
const darkBrandSource = readFileSync(new URL("../../src-tauri/icons/container.svg", import.meta.url), "utf8");
const lightBrandSource = readFileSync(new URL("../../src-tauri/icons/container-light.svg", import.meta.url), "utf8");
const darkMark = readFileSync(new URL("../../src/public/mark-dark.svg", import.meta.url), "utf8");
const lightMark = readFileSync(new URL("../../src/public/mark-light.svg", import.meta.url), "utf8");
const appIcon = readFileSync(new URL("../../src-tauri/icons/icon.ico", import.meta.url));
const projectIcon = readFileSync(new URL("../../src-tauri/icons/project.ico", import.meta.url));
const installerIcon = readFileSync(new URL("../../src-tauri/windows/setup-dark.ico", import.meta.url));
function hasRoundedWindowsFrames(icon: Buffer): boolean {
  const sizes = [16, 24, 32, 48, 64, 128, 256];
  if (icon.readUInt16LE(2) !== 1 || icon.readUInt16LE(4) !== sizes.length) return false;
  return sizes.every((size, index) => {
    const entry = 6 + index * 16;
    const offset = icon.readUInt32LE(entry + 12);
    if ((icon[entry] || 256) !== size || icon.readUInt32LE(offset) !== 40 || icon.readUInt16LE(offset + 14) !== 32) return false;
    const alpha = (x: number, y: number) => icon[offset + 40 + ((size - 1 - y) * size + x) * 4 + 3];
    return alpha(0, 0) === 0 && alpha(size - 1, 0) === 0 && alpha(0, size - 1) === 0 && alpha(size - 1, size - 1) === 0
      && alpha(size / 2, size / 2) === 255;
  });
}
const withoutBackground = (svg: string) => svg.replace(/^  <rect x="140" y="140" width="974" height="974" rx="224" fill="#[0-9A-Fa-f]{6}"\/>\r?\n/m, "");
const stableConfig = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const devConfig = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.dev.conf.json", import.meta.url), "utf8"));
const desktopPermissions = JSON.parse(readFileSync(new URL("../../src-tauri/capabilities/default.json", import.meta.url), "utf8")).permissions;
const mediaLoad = app.slice(app.indexOf("async function loadMedia("), app.indexOf("function closeMedia()"));
const markRoot = new URL("../../src-tauri/resources/social-tags/", import.meta.url);
const markManifest = JSON.parse(readFileSync(new URL("manifest.json", markRoot), "utf8")) as Record<string, {source: string; sourceSha256: string; pngSha256: string}>;
const marksMatchSources = ["kick-plain", "kick-boxed", "twitch"].every(name => {
  const entry = markManifest[name];
  if (!entry) return false;
  const source = readFileSync(new URL(`../../src/assets/${entry.source}`, import.meta.url), "utf8").replace(/\r\n/g, "\n");
  const png = readFileSync(new URL(`${name}.png`, markRoot));
  return createHash("sha256").update(source).digest("hex") === entry.sourceSha256
    && createHash("sha256").update(png).digest("hex") === entry.pngSha256;
});

const contracts: Array<[string, boolean]> = [
  ["all UI workspaces use the new theme-aware brand mark", app.includes('src="/mark-dark.svg"') && app.includes('src="/mark-light.svg"') && downloader.includes('src="/mark-dark.svg"') && downloader.includes('src="/mark-light.svg"')],
  ["UI marks keep the exact source artwork without a boxed background", withoutBackground(darkBrandSource) === darkMark && withoutBackground(lightBrandSource) === lightMark],
  ["application and installer ICOs match while project icon is distinct", appIcon.equals(installerIcon) && !appIcon.equals(projectIcon)],
  ["every Windows app icon size keeps rounded transparent corners", hasRoundedWindowsFrames(appIcon)],
  ["only .cproj has a fresh file class and embedded project icon", stableConfig.bundle.fileAssociations.length===1 && stableConfig.bundle.fileAssociations[0].ext.join(",")==="cproj" && stableConfig.bundle.fileAssociations[0].name==="CONTAINER CPROJ" && installerHooks.includes(',-32513') && installerHooks.includes('CONTAINER CPROJ\\DefaultIcon')],
  ["only .cproj projects can be saved and opened", app.includes('||"project"}.cproj') && app.includes('extensions:["cproj"]') && app.includes('old project format is no longer supported') && backend.includes('value.eq_ignore_ascii_case("cproj")')],
  ["upgrades retire the old project association and icon files", installerHooks.includes('DeleteRegValue SHELL_CONTEXT "Software\\Classes\\.containerproject"') && installerHooks.includes('Delete "$INSTDIR\\container-project-v2.ico"')],
  ["Windows is told to flush stale document icons after registration", installerHooks.includes('SHChangeNotify(i 0x08000000, i 0x1000') && readFileSync(new URL("register-dev-association.ps1", import.meta.url),"utf8").includes('SHChangeNotify(0x08000000, 0x1000')],
  ["download history is persistent, file-backed and recyclable", downloader.includes('list_download_history') && downloader.includes('delete_download_history_entry') && backend.includes('fn existing_download_history(') && backend.includes('trash::delete(file)')],
  ["both Windows icon changes retrigger resource compilation", buildScript.includes('cargo:rerun-if-changed=icons/icon.ico') && buildScript.includes('cargo:rerun-if-changed=icons/project.ico')],
  ["Windows upgrades refresh desktop and Start menu links with the embedded app icon", installerHooks.includes('"" "$INSTDIR\\container-studio.exe" 0') && installerHooks.includes('CreateShortCut "$DESKTOP\\${PRODUCTNAME}.lnk"') && installerHooks.includes('CreateShortCut "$SMPROGRAMS\\${PRODUCTNAME}.lnk"') && installerHooks.includes('SHChangeNotify(i 0x08000000')],
  ["installer no longer copies loose icons", !/^\s*File\s.*\.ico/m.test(installerHooks) && installerHooks.includes('Delete "$INSTDIR\\container-brand-rounded-*.ico"') && installerHooks.includes('Delete "$INSTDIR\\container-cproj-v2-*.ico"')],
  ["release verifies actual embedded resources before publishing", releaseWorkflow.includes('test-windows-icons.ps1 -Executable src-tauri/target/release/container-studio.exe')],
  ["closing hides the editor in the tray and Exit really terminates", backend.includes('api.prevent_close()') && backend.includes('window.hide()') && backend.includes('"tray-exit" => app.exit(0)')],
  ["tray update action uses the existing updater UI and DEV keeps it hidden", backend.includes('"tray-updates"') && backend.includes('app.emit("tray-check-updates", ())') && backend.includes('if is_development_build()') && app.includes('listen("tray-check-updates",()=>{void checkForUpdates(true)})')],
  ["tray menu follows the selected TR/EN language", app.includes('invoke("set_tray_language",{language:next})') && backend.includes('fn set_tray_language(language: &str')],
  ["release packaging chooses the exact current executable and installer", releaseWorkflow.includes("Get-Item -LiteralPath 'src-tauri/target/release/container-studio.exe'") && releaseWorkflow.includes('Filter "CONTAINER_${version}_x64-setup.exe"')],
  ["native confirmation dialogs have their required IPC permission", desktopPermissions.includes("dialog:allow-message")],
  ["export logo assets match the preview SVGs (regenerate with tooling/scripts/generate-social-tag-marks.mjs)", marksMatchSources],
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
  ["startup and drag-drop files share project-aware routing", app.includes('async function openIncomingPath') && app.includes('async function openIncomingPaths') && app.includes('await openIncomingPath(path)') && app.includes('if(unique.length===1){await openIncomingPath(unique[0]);return}') && app.includes('void openIncomingPaths(event.payload.paths)')],
  ["failed media loads retain the open session", app.includes('if(!await loadMedia(restoredPath,true))return') && mediaLoad.includes('return false;') && !mediaLoad.includes('media = null') && !mediaLoad.includes('selected = null')],
  ["startup recovery is ordered after both startup checks", app.includes('const [path,interrupted]=await Promise.all(') && app.includes('if(mediaLoadId!==initialMediaLoadId||media||restoringSession)return') && app.includes('startupAction(path,interrupted)')],
  ["Social Tag uses the supplied Kick SVG mark", kickLogo.includes('M278.26 216.86H646.7v245.62') && app.includes('import kickMark from "./assets/kick-mark.svg"')],
  ["Social Tag uses the supplied Twitch SVG mark", twitchLogo.includes('M5.7 0L1.4 10.985V55.88') && app.includes('import twitchMark from "./assets/twitch-mark.svg"')],
  ["Social Tag defaults to 36px and Twitch purple", tools.includes('number("social_tag_size", "Social Tag size", 36,') && twitchLogo.includes('#9146FF') && css.includes('background:#9146ff') && backend.includes('"0x9146ff"')],
  ["Social Tag platform and existing style selectors remain independent", app.includes('toolValue("social_tag_platform")') && app.includes('value="twitch" disabled={toolValue("social_tag_style")==="kick_banner"}>Twitch</option>')],
  ["Social Tag positions keep independent style defaults and camera-relative anchors", tools.includes('"social_tag_boxed_position", "Boxed Social Tag position", "center"') && tools.includes('"social_tag_plain_position", "Plain Social Tag position", "center"') && app.includes('socialTagGeometry({...tagInput,seamOffset:toolNumber("social_tag_seam_offset")})') && socialGeometry.includes('cameraLeft+cameraWidth/2') && socialGeometry.includes('cameraRight') && backend.includes('"social_tag_boxed_position"') && backend.includes('"social_tag_plain_position"')],
  ["Kick.com banner keeps the supplied raster art and font byte-for-byte", [
    ["kick-banner.png","3d0ca12b5d10042da855230612fc5e9343d5fe3c84d68660723dc7879cd0e363"],
    ["gotham-xnarrow-black.otf","fa500b06c1335dec86c502507b7c0c7458c3d2e769fbb7429bb6b90f9219533c"],
  ].every(([name,hash])=>createHash("sha256").update(readFileSync(new URL(name,markRoot))).digest("hex")===hash) && tools.includes('"kick_banner","Kick.com banner"') && css.includes('font-family:"Gotham XNarrow Black"') && backend.includes('clipper_kick_banner_filter(')],
  ["Kick banner art and font are embedded, cached outside the install folder, and bundled for preview", app.includes('import kickBanner from "../src-tauri/resources/social-tags/kick-banner.png"') && css.includes('url("../src-tauri/resources/social-tags/gotham-xnarrow-black.otf")') && backend.includes('include_bytes!("../resources/social-tags/kick-banner.png")') && backend.includes('include_bytes!("../resources/social-tags/gotham-xnarrow-black.otf")') && backend.includes('app_cache_directory()?.join("social-tags")') && !Object.keys(stableConfig.bundle.resources).some(path=>path.includes("social-tags"))],
  ["plain Social Tag remains legible over white video", css.includes('-webkit-text-stroke:.055em #050505') && backend.includes('borderw=3:bordercolor=black@0.95')],
  ["CI runs UI regression contracts", ciWorkflow.includes("pnpm test:ui")],
  ["Windows CI and release gate on real Clipper preview/export parity", ciWorkflow.includes('Compare real Clipper preview with Windows export') && ciWorkflow.includes('-g "Clipper real-media preview matches exported pixels"') && releaseWorkflow.includes('pnpm test:e2e')],
  ["release runs UI regression contracts", releaseWorkflow.includes("pnpm test:ui")],
  ["portable and installer share all seven license resources", Object.keys(stableConfig.bundle.resources).length === 7 && Object.values(stableConfig.bundle.resources).every(path => typeof path === "string" && /^licenses\/[^/]+$/.test(path)) && releaseWorkflow.includes('$resources.PSObject.Properties')],
  ["packages include the bundled Montserrat license", stableConfig.bundle.resources['resources/fonts/OFL.txt'] === 'licenses/Montserrat-OFL.txt'],
  ["packages include FFmpeg and face model licenses", stableConfig.bundle.resources['resources/FFmpeg-GPLv3.txt'] === 'licenses/FFmpeg-GPLv3.txt' && stableConfig.bundle.resources['resources/face_detection_yunet-LICENSE.txt'] === 'licenses/face_detection_yunet-LICENSE.txt'],
];

for (const [name, passed] of contracts) {
  if (!passed) throw new Error(`UI contract failed: ${name}`);
}

console.log(`UI/release contracts: ${contracts.length} passed`);
