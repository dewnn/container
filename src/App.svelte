<script lang="ts">
  import "@fontsource-variable/geist";
  import "@fontsource-variable/geist-mono";
  import { onMount, tick, untrack } from "svelte";
  import { invoke, convertFileSrc, isTauri } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Image as TauriImage } from "@tauri-apps/api/image";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { confirm, open, save } from "@tauri-apps/plugin-dialog";
  import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { armCompletionSound, playCompletionSound } from "./lib/completionSound";
  import { check, Update } from "@tauri-apps/plugin-updater";
  import { localizedForSection, localizedTool, preserveToolValues, type Field, type MediaKind, type Tool } from "./lib/tools";
  import { isTextEditingTarget } from "./lib/editorInput";
  import { startupAction } from "./lib/startupRecovery";
  import { projectResources, replaceProjectResource, type ProjectResource } from "./lib/projectResources";
  import { socialTagGeometry } from "./lib/socialTagGeometry";
  import { socialBannerGeometry } from "./lib/socialBannerGeometry";
  import ClipperLayoutPreview from "./lib/ClipperLayoutPreview.svelte";
  import { clipperLayoutGeometry, type ClipperLayoutInput } from "./lib/clipperLayoutGeometry";
  import SmartCutWorkspace from "./lib/SmartCutWorkspace.svelte";
  import BatchWorkspace from "./lib/BatchWorkspace.svelte";
  import StageHistoryControl from "./lib/StageHistory.svelte";
  import { toolSummary, toolCategory } from "./lib/toolSummaries";
  import { startStages, checkpointStage, continueStage, validStages, type StageHistory } from "./lib/stageHistory";
  import DownloaderWorkspace from "./lib/DownloaderWorkspace.svelte";
  import { recoveredMediaUrl } from "./lib/recovery";
  import { updatesAllowedForVersion } from "./lib/releaseChannel";
  import { moveTimelineBoundary, type TimelineBoundary } from "./lib/timelineRange";
  import { reportProblem, type ToastDetail } from "./lib/toast";
  import {renderProblem} from "./lib/renderFeedback";
  import ProblemDetails from "./lib/ProblemDetails.svelte";
  import RenderFeedback from "./lib/RenderFeedback.svelte";
  import GeneralSettings from "./lib/GeneralSettings.svelte";
  import {containDialog} from "./lib/dialogFocus";
  import {imageOutputFormat,imageQualityAdjustable,imageTargetSupported} from "./lib/imageCompressionUi";
  import {completionAlert} from "./lib/completionAlert";
  import WatermarkControls from "./lib/WatermarkControls.svelte";
  import WatermarkPreview from "./lib/WatermarkPreview.svelte";
  import {rasterText,type TextAppearance} from "./lib/textRaster";
  import {stackOutputDimensions,scaleStackText} from "./lib/stackGeometry";
  import BlurBackdropPreview from "./lib/BlurBackdropPreview.svelte";
  import NoisePreview from "./lib/NoisePreview.svelte";
  import PremiereSend from "./lib/PremiereSend.svelte";
  import {watchPremiere} from "./lib/premiereBridge.svelte";
  import kickMark from "./assets/kick-mark.svg";
  import twitchMark from "./assets/twitch-mark.svg";
  import kickBanner from "../src-tauri/resources/social-tags/kick-banner.png";

  interface MediaInfo {
    path: string;
    name: string;
    kind: MediaKind;
    duration: number | null;
    width: number | null;
    height: number | null;
    fps: number | null;
    codec: string;
    audio_codec: string | null;
    audio_tracks: { index:number; codec:string; channels:number|null; channel_layout:string|null; language:string|null; bitrate:number|null; is_default:boolean }[];
    pixel_format: string | null;
    bits_per_raw_sample: number | null;
    color_transfer: string | null;
    color_primaries: string | null;
    color_space: string | null;
    bitrate: number | null;
    size: number;
    start_timecode: string | null;
  }
  interface ProgressEvent { job_id?: string; percent: number; time: number; speed: string; frame: string; status: string }
  interface JobResult { output: string; elapsed: number }
  interface QualityCandidate { crf: number; vmaf: number; estimated_size_mb: number; rating: string }
  interface QualityAnalysis { recommended_crf: number; target_vmaf: number; candidates: QualityCandidate[]; sample_count: number; sampled_seconds: number; elapsed: number }
  interface CameraDetectionResult { x:number; y:number; width:number; height:number; focal_x:number; focal_y:number; confidence:number; samples:number; matched_samples:number }
  interface FfmpegStatus { ready: boolean; ffmpeg_version: string | null; ffprobe_version: string | null }
  interface FfmpegCapabilities { vidstab:boolean; subtitles:boolean; overlay:boolean; blur:boolean; concat:boolean }
  interface DownloaderStatus { ready:boolean; version:string|null }
  interface SubtitleTrack { index:number; codec:string; language:string|null; title:string|null }
  interface OutputCleanupResult { cleaned:boolean; path:string }
  interface FontOption { name:string; path:string }
  interface TextLayer { id:number; text:string; x:number; y:number; size:number; wrap_width?:number; color:string; opacity:number; align:"left"|"center"|"right"; fontName:string; font_path:string; outline:number; outline_color:string; shadow:number; shadow_color:string; background:boolean; background_color:string; background_opacity:number; background_padding:number }
  interface StackStep { id:number; tool:Tool; params:Record<string,string>; textLayers:TextLayer[]; colorEnabled:Record<string,boolean>; enabled:boolean; smartcutSession?:unknown; sourceWidth?:number }
  interface EditorSnapshot { media:MediaInfo; mediaUrl:string; selected:Tool|null; activeKind:MediaKind; output:string; outputSettingsKey?:string; outputMode?:"tool"|"stack"; renderedImageUrl:string; colorEnabled:Record<string,boolean>; colorPreviewVisible:boolean; textLayers:TextLayer[]; activeTextId:number|null; qualityAnalysis:QualityAnalysis|null; customNumberFields:Record<string,boolean>; mergeInputs?:string[]; processingStack?:StackStep[]; stackQuality?:string; editingStackStepId?:number|null }
  interface RecoverySession { version:1; savedAt:number; mediaPath:string; workspaceMode:"toolbox"|"autocut"|"batch"; toolbox:EditorSnapshot|null; autocut:unknown; batch:unknown; resources?:ProjectResource[]; stageHistory?:StageHistory<RecoverySession> }
  let stageHistory:StageHistory<RecoverySession>|null=$state(null);
  let stageNavigating=$state(false);

  const mediaDialogFilters=[{name:"Media",extensions:["mp4","mov","mkv","avi","webm","m4v","mp3","wav","m4a","aac","flac","opus","jpg","jpeg","png","webp","bmp","tif","tiff","avif","heic","heif"]}];
  const videoExtensions=new Set(["mp4","mov","mkv","avi","webm","m4v"]);

  let media: MediaInfo | null = $state(null);
  let mediaUrl = $state("");
  let selected: Tool | null = $state(null);
  let processingStack:StackStep[]=$state([]);
  let editingStackStepId:number|null=$state(null);
  let stackQuality=$state("high");
  let stackResultPreview=$state(false);
  let stackResultDuration=$state(0);
  const playbackDuration=$derived.by(()=>stackResultPreview?stackResultDuration:(media?.duration??0));
  let activeKind: MediaKind = $state("video");
  let frameAdvanced=$state(false);
  let busy = $state(false);
  let stackPreparing = false;
  let renderJobId = 0;
  let activeJobToken = "";
  let dragActive = $state(false);
  let error = $state("");
  let output = $state("");
  let outputSettingsKey=$state("");
  let outputMode:"tool"|"stack"=$state("tool");
  const outputStale=$derived(!!output&&outputSettingsKey!==renderSettingsKey(outputMode));
  $effect(()=>{if(!output||outputMode!=="stack"||outputStale)stackResultPreview=false});
  function renderSettingsKey(mode:"tool"|"stack"="tool"){return media&&mode==="stack"?JSON.stringify({source:media.path,steps:processingStack,quality:stackQuality}):media&&selected?JSON.stringify({source:media.path,operation:selected.id,params:paramsFrom(selected)}):""}
  let progress = $state(0);
  let jobStatus = $state("ready");
  let speed = $state("—");
  let frame = $state("—");
  let elapsed = $state(0);
  let search = $state("");
  let bannerFontReady = $state(false);
  let favoriteIds:string[]=$state([]);
  let favoritesOnly=$state(false);
  let workspaceMode: "toolbox" | "autocut" | "batch" = $state("toolbox");
  let batchInitialPaths:string[]=$state([]);
  let batchQueueCount=$state(0);
  let downloaderOpen = $state(false);
  let downloaderBusy = $state(false);
  let autoCutBusy=$state(false),batchBusy=$state(false);
  let autoCutWorkspace:{undo:()=>void;redo:()=>void;exportSession:()=>unknown;restoreSession:(value:any)=>void;resetPanelWidths:()=>void}|null=$state(null);
  let batchWorkspace:{undo:()=>void;redo:()=>void;exportSession:()=>unknown;restoreSession:(value:any)=>void;resetPanelWidths:()=>void;addPaths:(paths:string[])=>void}|null=$state(null);
  let autoCutSession:unknown=$state(null),batchSession:unknown=$state(null);
  let recoveryCandidate:RecoverySession|null=$state(null);
  let projectFilesOpen=$state(false);
  let projectFileChecks:{resource:ProjectResource;exists:boolean}[]=$state([]);
  let restoringSession=$state(false);
  let autoCutCanUndo=$state(false),autoCutCanRedo=$state(false);
  let batchCanUndo=$state(false),batchCanRedo=$state(false);
  let toolboxVideo: HTMLVideoElement | null = $state(null);
  let toolboxWorkspace: HTMLElement | null = $state(null);
  let toolboxWorkspaceWidth = $state(0);
  let toolboxLeftWidth = $state<number|null>(null);
  let toolboxRightWidth = $state<number|null>(null);
  let panelResetDialogOpen = $state(false);
  let toolboxStage: HTMLElement | null = $state(null);
  let toolboxCanvas: HTMLElement | null = $state(null);
  let textPreviewCanvas: HTMLCanvasElement | null = $state(null);
  let transformCanvasWidth = $state(0);
  let transformCanvasHeight = $state(0);
  let toolboxMetadataVersion=$state(0);
  let transformSourceBox: HTMLElement | null = $state(null);
  let freecamLayoutBox: HTMLElement | null = $state(null);
  let clipperGuidesActive = $state(false);
  let clipperPreviewMode = $state<"output"|"source">("source");
  const multiRegionClipper=$derived.by(()=>selected?.id==="clipper"&&["split","squares","freecam"].includes(toolValue("vertical_layout")));
  const clipperOutputVisible=$derived(multiRegionClipper&&clipperPreviewMode==="output");
  let toolboxCurrent = $state(0);
  let toolboxPlaying = $state(false);
  let toolboxVolume = $state(1);
  let renderedImageUrl = $state("");
  let renderedImageSize = $state(0);
  let temporaryImagePreviewPath = "";
  let compressionEstimate = $state<number|null>(null);
  let compressionEstimateLoading = $state(false);
  let compressionEstimateError = $state(false);
  let compressionEstimateRetry = $state(0);
  let compressionEstimateId = 0;
  let qualityAnalysis: QualityAnalysis | null = $state(null);
  let qualityAnalyzing = $state(false);
  let cameraDetecting = $state(false);
  let cameraDetectionMessage = $state("");
  let hashResult = $state("");
  let colorEnabled: Record<string,boolean> = $state({});
  let colorPreviewVisible = $state(true);
  let textLayers: TextLayer[] = $state([]);
  let textPresets: {id:string;name:string;sourceWidth?:number;layer:TextLayer}[] = $state([]);
  let textPresetName = $state("");
  let selectedTextPreset = $state("");
  let textPresetBusy = $state(false);
  let watermarkLoading=$state(false);
  let watermarkFontReady=$state("");
  let watermarkSelectionVersion=0;
  const watermarkLayer=$derived.by(()=>{try{return JSON.parse(toolValue("watermark_layer")||"null") as TextAppearance|null}catch{return null}});
  function changeWatermark(patch:Partial<TextAppearance>){if(!watermarkLayer)return;const next={...watermarkLayer,...patch};setToolValue("watermark_layer",JSON.stringify(next));setToolValue("watermark_text",next.text)}
  async function prepareWatermark(){
    if(watermarkLoading)return;const sourceTool=selected,version=watermarkSelectionVersion;watermarkLoading=true;
    try{
      const fonts=await ensureSystemFonts();const existing=watermarkLayer;
      const font=existing?fonts.find(font=>font.path===existing.font_path):fonts.find(font=>font.name.toLowerCase()==="arial")??fonts[0];
      if(!font)throw new Error("Watermark font is unavailable. Choose an installed font.");
      const fontName=await loadPreviewFont(font);if(selected!==sourceTool||version!==watermarkSelectionVersion)return;
      const layer:TextAppearance=existing?{...existing,fontName}:{text:toolValue("watermark_text"),x:50,y:50,size:toolNumber("watermark_size")||32,color:"#ffffff",opacity:toolNumber("watermark_opacity")||85,align:"center",fontName,font_path:font.path,outline:0,outline_color:"#000000",shadow:0,shadow_color:"#000000",background:toolValue("watermark_background")==="true",background_color:"#000000",background_opacity:65,background_padding:12};
      setToolValue("watermark_layer",JSON.stringify(layer));watermarkFontReady=font.path;
    }catch(reason){reportProblem(reason)}finally{
      watermarkLoading=false;
      if(selected!==sourceTool&&selected?.id==="clipper"&&toolValue("watermark_enabled")==="true"&&(!watermarkLayer||watermarkFontReady!==watermarkLayer.font_path))void prepareWatermark();
    }
  }
  async function chooseWatermarkFont(path:string){const sourceTool=selected,version=++watermarkSelectionVersion;try{const font=(await ensureSystemFonts()).find(font=>font.path===path);if(!font)return;const fontName=await loadPreviewFont(font);if(selected===sourceTool&&version===watermarkSelectionVersion){changeWatermark({font_path:path,fontName});watermarkFontReady=path}}catch(reason){if(version===watermarkSelectionVersion)reportProblem(reason)}}
  async function applyWatermarkPreset(id:string){const preset=textPresets.find(item=>item.id===id);if(!preset)return;const sourceTool=selected,version=++watermarkSelectionVersion;try{const font=(await ensureSystemFonts()).find(font=>font.path.toLowerCase()===preset.layer.font_path.toLowerCase());if(!font)throw new Error("Preset font is unavailable.");const fontName=await loadPreviewFont(font);if(selected!==sourceTool||version!==watermarkSelectionVersion)return;const scale=preset.sourceWidth?(toolNumber("output_width")||1080)/preset.sourceWidth:1;const layer={...preset.layer,font_path:font.path,fontName,size:preset.layer.size*scale,outline:preset.layer.outline*scale,shadow:preset.layer.shadow*scale,background_padding:preset.layer.background_padding*scale};setToolValue("watermark_layer",JSON.stringify(layer));setToolValue("watermark_text",layer.text);watermarkFontReady=font.path;clipperPreviewMode="output"}catch(reason){if(version===watermarkSelectionVersion)reportProblem(reason)}}
  $effect(()=>{if(selected?.id==="clipper"&&toolValue("watermark_enabled")==="true"&&(!watermarkLayer||watermarkFontReady!==watermarkLayer.font_path))untrack(()=>void prepareWatermark())});
  let activeTextId: number | null = $state(null);
  let systemFonts: FontOption[] = $state([]);
  let systemFontsLoad: Promise<FontOption[]> | null = null;
  const previewFontLoads = new Map<string,Promise<string>>();
  const fontSelectionVersions = new Map<number,number>();
  let textMeasureCanvas: HTMLCanvasElement | null = null;
  let qualityAdvanced = $state(localStorage.getItem("container-quality-mode")==="advanced");
  let nextTextId = 1;
  let imageCompare = $state(50);
  let imageViewport: HTMLElement | null = $state(null);
  let imageZoom = $state(1);
  let imageBaseScale = $state(1);
  let imagePanX = $state(0);
  let imagePanY = $state(0);
  let imageDragging = $state(false);
  let imageViewInitialized = $state(false);
  let customNumberFields: Record<string, boolean> = $state({});
  let toolboxFilmstripUrl = $state("");
  let toolboxFilmstripLoading = $state(false);
  let filmstripLoadId=0,subtitleLoadId=0;
  let toolboxTimeline: HTMLElement | null = $state(null);
  let timelineHover = $state<number|null>(null);
  let playerSeekHover:{percent:number;time:number;precision:boolean}|null=$state(null);
  let cutStartInput = $state("0:00:00");
  let cutEndInput = $state("0:00:10");
  let cutTimeEditing:"start"|"end"|null=$state(null);
  let language: "tr" | "en" = $state("en");
  let theme: "dark" | "light" = $state(document.documentElement.dataset.theme === "light" ? "light" : "dark");
  let availableEncoders: string[] | null = $state(null);
  let autoEncoderTuning = $state(false);
  let ffmpegStatus: FfmpegStatus | null = $state(null);
  let ffmpegCapabilities:FfmpegCapabilities|null=$state(null);
  let downloaderStatus:DownloaderStatus|null=$state(null);
  let overlayPreviewImage:HTMLImageElement|null=$state(null);
  let mergeInputs:string[]=$state([]);
  let subtitleTracks:SubtitleTrack[]=$state([]);
  let dependencyChecking = $state(false);
  let dependencyPanel = $state(false);
  let runtimeMigrationError = $state("");
  let appVersion = $state("");
  const updaterEnabled=$derived(updatesAllowedForVersion(appVersion));
  const devVersion=$derived(/-dev(?:\.|$)/i.test(appVersion) ? appVersion : "");
  let availableUpdate: Update | null = $state(null);
  let updatePanel = $state(false);
  let outputCleanupOpen = $state(false);
  let outputCleaning = $state(false);
  let outputCleanupMessage = $state("");
  let outputCleanupMessageTimer:number|undefined;
  let toastMessage=$state("");
  let toastKind:"error"|"info"|"success"=$state("error");
  let toastTimer:number|undefined;
  let updateChecking = $state(false);
  let updateInstalling = $state(false);
  let updateStatus = $state("");
  let editHistory: EditorSnapshot[] = $state([]);
  let editHistoryIndex = $state(-1);
  let historyApplying = false;
  let updateDownloaded = $state(0);
  let updateTotal = $state(0);
  const messages:Record<"tr"|"en",Record<string,string>>={
    tr:{tagline:"FFMPEG MEDYA ARAÇLARI",close:"kapat",drop:"dosyanı buraya bırak",browse:"ya da seçmek için tıkla",landingTitle:"tek yerde. tüm araçlar.",landingCopy:"Videonu, sesini veya görselini aç; ihtiyacın olan araçlar ve tüm ayarlar burada.",local:"işlemler cihazında yapılır",untouched:"kaynak dosyaların değişmez",tools:"ARAÇLAR",available:"araç",video:"video",audio:"ses",image:"görsel",search:"araç ara...",preview:"ÖNİZLEME",original:"ORİJİNAL",rendered:"İŞLENMİŞ",process:"İŞLEM",frame:"kare",speed:"hız",elapsed:"geçen süre",showOutput:"klasörde göster",cancelJob:"işlemi iptal et",parameters:"AYARLAR",defaults:"varsayılanlar",what:"NE İŞE YARAR?",forVideo:"BU VİDEODA",choose:"dosya seç...",custom:"Özel…",render:"işle",outputNote:"Çıktın İndirilenler/CONTAINER Output klasörüne kaydedilir. Kaynak dosyan değişmez.",selectTool:"Bir araç seç",dropOpen:"açmak için bırak",ready:"hazır",toolbox:"ARAÇ KUTUSU"},
    en:{tagline:"FFMPEG MEDIA TOOLBOX",close:"close",drop:"drop media here",browse:"or click to browse files",landingTitle:"one place. every tool.",landingCopy:"All CONTAINER FFmpeg operations in one workspace with detailed controls and live progress.",local:"local processing only",untouched:"original files stay untouched",tools:"TOOLS",available:"available",video:"video",audio:"audio",image:"image",search:"search tools...",preview:"PREVIEW",original:"ORIGINAL",rendered:"RENDERED",process:"PROCESS",frame:"frame",speed:"speed",elapsed:"elapsed",showOutput:"show in folder",cancelJob:"cancel job",parameters:"PARAMETERS",defaults:"defaults",what:"WHAT DOES IT DO?",forVideo:"FOR THIS VIDEO",choose:"choose file...",custom:"Custom…",render:"render",outputNote:"Output is written to Downloads/CONTAINER Output. The source file is not changed.",selectTool:"Select a tool",dropOpen:"drop to open",ready:"ready",toolbox:"TOOLBOX"}
  };
  const t=(key:string)=>messages[language][key]??key;
  function friendlyProblem(reason:unknown){
    const raw=String(reason??"").trim();
    const renderHint=renderProblem(raw,language);if(renderHint)return renderHint;
    if(language==="tr"){
      if(/valid HTTPS|video link/i.test(raw))return "Geçerli bir HTTPS video bağlantısı girip tekrar dene.";
      if(/yt-dlp.*not ready|yt-dlp gerekli/i.test(raw))return "İndirme bileşeni hazır değil. Önce resmî yt-dlp.exe dosyasını seç.";
      if(/ffmpeg.*(?:not found|missing|could not start)/i.test(raw))return "FFmpeg bileşeni bulunamadı. Uygulamayı yeniden kurup tekrar dene.";
      if(/font|yazı tipi/i.test(raw))return `Yazı tipi önizlemesi yüklenemedi: ${raw}`;
      if(/network|connect|timed? out|internet/i.test(raw))return "Bağlantı kurulamadı. İnternet bağlantını kontrol edip tekrar dene.";
      if(/permission|access denied/i.test(raw))return "Dosyaya erişilemedi. Klasör izinlerini kontrol edip tekrar dene.";
      if(/download failed/i.test(raw))return "İndirme tamamlanamadı. Bağlantıyı veya seçilen formatı kontrol et.";
      if(/sign in|login required|confirm you.?re not a bot/i.test(raw))return "Bu video giriş doğrulaması istiyor ve şu anda indirilemiyor.";
      if(/HTTP Error 40[134]|forbidden/i.test(raw))return "Kaynak indirmeye izin vermedi. Biraz sonra veya başka bir formatla tekrar dene.";
      if(/video unavailable|private video/i.test(raw))return "Bu video kullanılamıyor veya özel olarak ayarlanmış.";
      if(/format.*(?:unavailable|not available|unsupported)/i.test(raw))return "Seçilen format bu video için kullanılamıyor. Başka bir kalite seç.";
      return raw||"Beklenmeyen bir sorun oluştu. Lütfen tekrar dene.";
    }
    if(/valid HTTPS|video link/i.test(raw))return "Enter a valid HTTPS video link and try again.";
    if(/yt-dlp.*not ready/i.test(raw))return "The download component is not ready. Select the official yt-dlp executable first.";
    if(/ffmpeg.*(?:not found|missing|could not start)/i.test(raw))return "FFmpeg is unavailable. Reinstall the application and try again.";
    if(/font/i.test(raw))return `The font preview could not be loaded: ${raw}`;
    if(/network|connect|timed? out|internet/i.test(raw))return "Could not connect. Check your internet connection and try again.";
    if(/permission|access denied/i.test(raw))return "The file could not be accessed. Check the folder permissions and try again.";
    if(/download failed/i.test(raw))return "The download could not be completed. Check the link or selected format.";
    if(/sign in|login required|confirm you.?re not a bot/i.test(raw))return "This video requires sign-in verification and cannot currently be downloaded.";
    if(/HTTP Error 40[134]|forbidden/i.test(raw))return "The source refused the download. Try again later or choose another format.";
    if(/video unavailable|private video/i.test(raw))return "This video is unavailable or private.";
    if(/format.*(?:unavailable|not available|unsupported)/i.test(raw))return "That format is unavailable for this video. Choose another quality.";
    return raw||"An unexpected problem occurred. Please try again.";
  }
  function showToast(reason:unknown,kind:"error"|"info"|"success"="error"){
    toastMessage=kind==="error"?friendlyProblem(reason):String(reason);
    toastKind=kind;
    window.clearTimeout(toastTimer);
    toastTimer=window.setTimeout(()=>toastMessage="",kind==="error"?10000:5000);
  }
  const kindTools=(kind:MediaKind)=>localizedForSection(kind,media?.kind??kind,language).filter(tool=>{
    if(!ffmpegCapabilities)return true;
    if(tool.id==="stabilizer")return ffmpegCapabilities.vidstab;
    if(tool.id==="image_overlay")return ffmpegCapabilities.overlay;
    if(tool.id==="blur_pixelate")return ffmpegCapabilities.blur&&ffmpegCapabilities.overlay;
    if(tool.id==="merge_videos")return ffmpegCapabilities.concat;
    return true;
  });
  const timelineTool = $derived.by(()=>!stackResultPreview&&media?.kind==="video"&&selected ? ["cut","screenshot","gif","image_overlay"].includes(selected.id) : false);
  const operationBusy=$derived(busy||qualityAnalyzing||cameraDetecting||autoCutBusy||batchBusy||downloaderBusy||restoringSession||stageNavigating);
  const canUndo = $derived(!operationBusy&&(workspaceMode==="toolbox"?editHistoryIndex>0:workspaceMode==="autocut"?autoCutCanUndo:batchCanUndo));
  const canRedo = $derived(!operationBusy&&(workspaceMode==="toolbox"?editHistoryIndex>=0&&editHistoryIndex<editHistory.length-1:workspaceMode==="autocut"?autoCutCanRedo:batchCanRedo));
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenDrop: UnlistenFn | null = null;
  let timelineIgnoreClickUntil = 0;
  let mediaLoadId = 0;
  let workspaceSwitchId = 0;

  function cloneEditorValue<T>(value:T):T{return JSON.parse(JSON.stringify(value)) as T}

  const recoveryKey="container-recovery-v1";
  function validRecovery(value:unknown):value is RecoverySession{
    if(!value||typeof value!=="object")return false;
    const candidate=value as Partial<RecoverySession>;
    return candidate.version===1&&typeof candidate.savedAt==="number"&&typeof candidate.mediaPath==="string"&&candidate.mediaPath.length>0&&["toolbox","autocut","batch"].includes(candidate.workspaceMode??"");
  }
  function persistRecovery(){
    if(!media||restoringSession||stageNavigating)return;
    const value=currentSession();
    try{localStorage.setItem(recoveryKey,JSON.stringify(value))}catch{showToast(language==="tr"?"Otomatik kayıt alanı dolu. Projeyi dosya olarak kaydet.":"Recovery storage is full. Save your project to a file.","info")}
  }
  function currentSession(includeStages=true):RecoverySession|null{
    if(!media)return null;
    return {version:1,savedAt:Date.now(),mediaPath:media.path,workspaceMode,toolbox:captureEditorSnapshot(),autocut:workspaceMode==="autocut"&&autoCutWorkspace?autoCutWorkspace.exportSession():autoCutSession,batch:workspaceMode==="batch"&&batchWorkspace?batchWorkspace.exportSession():batchSession,...(includeStages&&stageHistory?{stageHistory:cloneEditorValue(stageHistory)}:{})};
  }
  async function saveProject(){
    const session=currentSession();if(!session||operationBusy)return;
    session.resources=projectResources(session,true);
    const path=await save({defaultPath:`${media?.name.replace(/\.[^.]+$/,"")||"project"}.cproj`,filters:[{name:"CONTAINER Project",extensions:["cproj"]}]});
    if(!path)return;
    try{
      const checks=await inspectProjectFiles(session);
      if(checks.some(item=>!item.exists)){
        projectFileChecks=checks;projectFilesOpen=true;
        const proceed=await confirm(language==="tr"?"Bazı kaynak dosyalar bulunamadı. Projeyi yine de kaydetmek ister misin?":"Some source files are missing. Save the project anyway?",{title:language==="tr"?"Eksik proje dosyaları":"Missing project files",kind:"warning"});
        if(!proceed)return;
      }
      await invoke("write_project",{path,contents:JSON.stringify(session,null,2)});
      jobStatus=language==="tr"?"proje kaydedildi":"project saved";
    }catch(reason){reportProblem(reason)}
  }
  async function inspectProjectFiles(session:RecoverySession){
    return Promise.all(projectResources(session).map(async resource=>({resource,exists:await invoke<boolean>("project_media_available",{path:resource.path}).catch(()=>false)})));
  }
  async function loadProjectPath(path:string){
    const loadId=++mediaLoadId;
    const contents=await invoke<string>("read_project",{path});
    if(loadId!==mediaLoadId)return;
    const saved=JSON.parse(contents);
    if(!validRecovery(saved))throw new Error(language==="tr"?"Geçersiz CONTAINER proje dosyası.":"Invalid CONTAINER project file.");
    recoveryCandidate=saved;await restorePreviousSession();
  }
  async function openIncomingPath(path:string){
    if(operationBusy)return;
    try{
      if(path.toLowerCase().endsWith(".cproj"))await loadProjectPath(path);
      else if(path.toLowerCase().endsWith(".containerproject"))throw new Error(language==="tr"?"Bu eski proje biçimi artık desteklenmiyor. Yalnızca .cproj dosyaları açılabilir.":"This old project format is no longer supported. Only .cproj files can be opened.");
      else await loadMedia(path);
    }catch(reason){reportProblem(reason)}
  }
  async function openIncomingPaths(paths:string[]){
    if(operationBusy)return;
    const unique=[...new Map(paths.filter(Boolean).map(path=>[path.toLowerCase(),path])).values()];
    if(unique.length===0)return;
    if(workspaceMode==="batch"&&batchWorkspace&&unique.every(path=>videoExtensions.has(path.split(".").pop()?.toLowerCase()??""))){batchWorkspace.addPaths(unique);return}
    if(unique.length===1){await openIncomingPath(unique[0]);return}
    if(unique.some(path=>!videoExtensions.has(path.split(".").pop()?.toLowerCase()??""))){
      showToast(language==="tr"?"Çoklu içe aktarma yalnızca videoları kabul eder. Ses/görsel dosyalarını ayrı açabilirsin.":"Multi-import accepts videos only. Open audio and images separately.");
      return;
    }
    if(!await loadMedia(unique[0]))return;
    if(media?.kind!=="video"){
      showToast(language==="tr"?"Seçilen dosyalar video olmalı.":"Selected files must be videos.");
      return;
    }
    batchInitialPaths=unique;
    batchQueueCount=unique.length;
    workspaceMode="batch";
  }
  async function openDownloadedMedia(path:string){
    if(operationBusy)return;
    if(!await loadMedia(path))return;
    if(media?.kind==="video"){
      const cutTool=kindTools("video").find(tool=>tool.id==="cut");
      if(cutTool)chooseTool(cutTool);
    }
    downloaderOpen=false;
  }
  async function openProject(){
    if(operationBusy)return;
    const path=await open({multiple:false,filters:[{name:"CONTAINER Project",extensions:["cproj"]}]});if(typeof path!=="string")return;
    await openIncomingPath(path);
  }
  function discardRecovery(){localStorage.removeItem(recoveryKey);recoveryCandidate=null}
  async function cleanOutputFolder(){
    if(outputCleaning)return;
    outputCleaning=true;outputCleanupMessage="";
    try{
      const result=await invoke<OutputCleanupResult>("clean_output_folder");
      outputCleanupMessage=result.cleaned
        ? (language==="tr"?"Çıktılar Geri Dönüşüm Kutusu’na taşındı.":"Output was moved to the Recycle Bin.")
        : (language==="tr"?"Temizlenecek çıktı bulunamadı.":"There was no output to clean.");
      outputCleanupOpen=false;
      window.clearTimeout(outputCleanupMessageTimer);
      outputCleanupMessageTimer=window.setTimeout(()=>outputCleanupMessage="",2500);
    }catch(reason){outputCleanupMessage="";reportProblem(reason)}finally{outputCleaning=false}
  }
  async function restorePreviousSession(){
    let saved=recoveryCandidate;if(!saved||restoringSession)return;
    restoringSession=true;error="";
    try{
      let restoredPath=saved.mediaPath;
      const sourceAvailable=await invoke<boolean>("project_media_available",{path:saved.mediaPath});
      if(!sourceAvailable){
        const missingMessage=language==="tr"
          ? `Bu projenin kaynak dosyası taşınmış veya silinmiş:\n${saved.mediaPath}\n\nProjeyi geri yüklemek için dosyanın yeni konumunu seçmek ister misin?`
          : `This project's source file was moved or deleted:\n${saved.mediaPath}\n\nWould you like to choose its new location and restore the project?`;
        const locate=await confirm(missingMessage,{title:language==="tr"?"Kaynak dosya bulunamadı":"Source file not found",kind:"warning",okLabel:language==="tr"?"DOSYAYI BUL":"LOCATE FILE",cancelLabel:language==="tr"?"İPTAL":"CANCEL"});
        if(!locate){
          error=language==="tr"?"Proje açılamadı: kaynak medya taşınmış veya silinmiş.":"Project could not be opened because its source media was moved or deleted.";
          jobStatus="source missing";
          return;
        }
        const replacement=await open({multiple:false,filters:mediaDialogFilters});
        if(typeof replacement!=="string"){
          error=language==="tr"?"Yeni kaynak dosya seçilmedi. Proje değiştirilmedi.":"No replacement source was selected. The project was not changed.";
          jobStatus="source missing";
          return;
        }
        saved=replaceProjectResource(saved,saved.mediaPath,replacement);
        recoveryCandidate=saved;
        restoredPath=replacement;
      }
      for(const resource of projectResources(saved).filter(item=>item.path!==restoredPath)){
        if(await invoke<boolean>("project_media_available",{path:resource.path}))continue;
        const locate=await confirm(language==="tr"?`${resource.label} bulunamadı:\n${resource.path}\n\nYeni konumunu seçmek ister misin?`:`${resource.label} was not found:\n${resource.path}\n\nChoose its new location?`,{title:language==="tr"?"Eksik proje kaynağı":"Missing project resource",kind:"warning",okLabel:language==="tr"?"DOSYAYI BUL":"LOCATE FILE",cancelLabel:language==="tr"?"İPTAL":"CANCEL"});
        if(!locate){error=language==="tr"?"Proje açılmadı: gerekli kaynak dosyası eksik.":"Project was not opened because a required source file is missing.";return}
        const replacement=await open({multiple:false});
        if(typeof replacement!=="string"){error=language==="tr"?"Yeni kaynak dosya seçilmedi.":"No replacement source was selected.";return}
        saved=replaceProjectResource(saved,resource.path,replacement);
        recoveryCandidate=saved;
      }
      if(!await loadMedia(restoredPath,true))return;
      saved.mediaPath=restoredPath;
      if(saved.toolbox){
        const preparedMediaUrl=mediaUrl;
        applyEditorSnapshot(saved.toolbox,"redo",true);
        // Saved asset:// URLs belong to the previous WebView session. Keep the
        // freshly authorized URL from loadMedia and make the source identity new
        // so Chromium cannot retain the empty/failed media element from startup.
        mediaUrl=recoveredMediaUrl(preparedMediaUrl);
        renderedImageUrl="";
        if(media?.kind==="image"&&output){
          try{await invoke("authorize_media_preview",{path:output});renderedImageUrl=recoveredMediaUrl(convertFileSrc(output))}catch{ /* A missing old output must not prevent restoring its editable source. */ }
        }
        await tick();
        if(media?.kind==="video"){
          toolboxVideo?.load();
        }
        await restorePreviewFonts();resetEditorHistory();
      }
      workspaceMode=saved.workspaceMode;
      autoCutSession=saved.autocut;batchSession=saved.batch;
      await tick();
      if(saved.workspaceMode==="autocut"&&saved.autocut)autoCutWorkspace?.restoreSession(saved.autocut);
      if(saved.workspaceMode==="batch"&&saved.batch)batchWorkspace?.restoreSession(saved.batch);
      stageHistory=validStages(saved.stageHistory,validRecovery)?saved.stageHistory:startStages(currentSession(false)!,stageLabel());
      recoveryCandidate=null;
      return true;
    }catch(reason){reportProblem(reason)}finally{restoringSession=false;persistRecovery()}
  }

  function captureEditorSnapshot():EditorSnapshot|null{
    if(!media)return null;
    return cloneEditorValue({media,mediaUrl,selected,activeKind,output,outputSettingsKey,outputMode,renderedImageUrl,colorEnabled,colorPreviewVisible,textLayers,activeTextId,qualityAnalysis,customNumberFields,mergeInputs,processingStack,stackQuality,editingStackStepId});
  }
  function snapshotSignature(snapshot:EditorSnapshot){return JSON.stringify(snapshot)}
  function resetEditorHistory(){const snapshot=captureEditorSnapshot();editHistory=snapshot?[snapshot]:[];editHistoryIndex=snapshot?0:-1}
  function commitEditorSnapshot(snapshot:EditorSnapshot){
    if(historyApplying)return;
    if(editHistoryIndex>=0&&snapshotSignature(editHistory[editHistoryIndex])===snapshotSignature(snapshot))return;
    editHistory=[...editHistory.slice(0,editHistoryIndex+1),snapshot].slice(-80);
    editHistoryIndex=editHistory.length-1;
  }
  function flushEditorSnapshot(){const snapshot=captureEditorSnapshot();if(snapshot)commitEditorSnapshot(snapshot)}
  function restoreToolSnapshot(saved:Tool|null):Tool|null{
    if(!saved)return null;
    const current=kindTools(activeKind).find(tool=>tool.id===saved.id);
    if(!current)return kindTools(activeKind)[0]??null;
    const restored=preserveToolValues(current,saved),savedFields=new Map(saved.fields.map(field=>[field.key,field]));
    if(restored.id==="gif"&&!savedFields.has("end")){
      const start=Number(savedFields.get("start")?.value??0),duration=Number(savedFields.get("duration")?.value??5);
      const end=restored.fields.find(field=>field.key==="end");
      if(end)end.value=Math.min(media?.duration??86400,start+Math.max(.01,duration));
    }
    populateAudioTrackOptions(restored,true);
    return restored;
  }
  function applyEditorSnapshot(snapshot:EditorSnapshot,direction:"undo"|"redo",preserveLoadedMedia=false){
    stackResultPreview=false;
    historyApplying=true;
    toolboxVideo?.pause();
    if(!preserveLoadedMedia){media=cloneEditorValue(snapshot.media);mediaUrl=snapshot.mediaUrl}
    activeKind=snapshot.activeKind;
    selected=restoreToolSnapshot(snapshot.selected);
    // Loading a historical source clears derived previews. Restoring its tool
    // bypasses selectTool, so request the filmstrip here as well.
    if(selected&&["cut","screenshot","gif","image_overlay"].includes(selected.id))void loadToolboxFilmstrip();
    output=snapshot.output;outputSettingsKey=snapshot.outputSettingsKey??"";outputMode=snapshot.outputMode??"tool";renderedImageUrl=snapshot.renderedImageUrl;colorEnabled=cloneEditorValue(snapshot.colorEnabled);colorPreviewVisible=snapshot.colorPreviewVisible;textLayers=cloneEditorValue(snapshot.textLayers);activeTextId=snapshot.activeTextId;qualityAnalysis=cloneEditorValue(snapshot.qualityAnalysis);customNumberFields=cloneEditorValue(snapshot.customNumberFields);mergeInputs=cloneEditorValue(snapshot.mergeInputs??(media?[media.path]:[]));processingStack=cloneEditorValue(snapshot.processingStack??[]);stackQuality=snapshot.stackQuality??"high";editingStackStepId=processingStack.some(step=>step.id===snapshot.editingStackStepId&&step.tool.id===selected?.id)?snapshot.editingStackStepId??null:null;toolboxPlaying=false;toolboxCurrent=0;error="";jobStatus=language==="tr"?(direction==="undo"?"geri alındı":"ileri alındı"):(direction==="undo"?"undone":"redone");
    requestAnimationFrame(()=>historyApplying=false);
  }
  function undoEditor(){if(operationBusy)return;if(workspaceMode==="autocut"){autoCutWorkspace?.undo();return}if(workspaceMode==="batch"){batchWorkspace?.undo();return}flushEditorSnapshot();if(editHistoryIndex<=0)return;editHistoryIndex-=1;applyEditorSnapshot(editHistory[editHistoryIndex],"undo")}
  function redoEditor(){if(operationBusy)return;if(workspaceMode==="autocut"){autoCutWorkspace?.redo();return}if(workspaceMode==="batch"){batchWorkspace?.redo();return}if(editHistoryIndex>=editHistory.length-1)return;editHistoryIndex+=1;applyEditorSnapshot(editHistory[editHistoryIndex],"redo")}

  $effect(()=>{
    const snapshot=captureEditorSnapshot();
    if(!snapshot||historyApplying)return;
    const signature=snapshotSignature(snapshot);
    const timer=window.setTimeout(()=>{const current=captureEditorSnapshot();if(!historyApplying&&current&&signature===snapshotSignature(current))commitEditorSnapshot(snapshot)},280);
    return()=>window.clearTimeout(timer);
  });

  $effect(()=>{
    const snapshot=captureEditorSnapshot();
    if(!snapshot||historyApplying||restoringSession)return;
    workspaceMode;autoCutSession;batchSession;
    const timer=window.setTimeout(persistRecovery,500);
    return()=>window.clearTimeout(timer);
  });

  const categories = $derived.by(() => {
    const list = kindTools(activeKind).filter((tool) => (!favoritesOnly||favoriteIds.includes(tool.id))&&`${tool.title} ${tool.description}`.toLocaleLowerCase(language).includes(search.toLocaleLowerCase(language)));
    const map = new Map<string, Tool[]>();
    for (const tool of list) map.set(tool.category, [...(map.get(tool.category) ?? []), tool]);
    const entries=[...map.entries()];
    if(activeKind==="image")entries.sort(([left],[right])=>left==="Utilities"?1:right==="Utilities"?-1:0);
    return entries;
  });
  const visibleFavoriteCount = $derived(kindTools(activeKind).filter(tool=>favoriteIds.includes(tool.id)).length);

  function toggleFavorite(id:string){favoriteIds=favoriteIds.includes(id)?favoriteIds.filter(value=>value!==id):[...favoriteIds,id];localStorage.setItem("container-favorites",JSON.stringify(favoriteIds))}

  const formatBytes = (value: number) => {
    if (!Number.isFinite(value)) return "—";
    const units = ["B", "KB", "MB", "GB"];
    let amount = value;
    let unit = 0;
    while (amount >= 1024 && unit < units.length - 1) { amount /= 1024; unit++; }
    return `${amount.toFixed(unit ? 2 : 0)} ${units[unit]}`;
  };
  const formatDuration = (value: number | null) => value == null ? "—" : `${value.toFixed(2)}s`;
  const rangePercent = (value:number,min:number,max:number) => Math.max(0,Math.min(100,(value-min)/Math.max(.000001,max-min)*100));
  const playerTime = (value: number) => {
    const safe = Math.max(0, Number(value) || 0);
    const hours = Math.floor(safe / 3600);
    const minutes = Math.floor((safe % 3600) / 60);
    const seconds = Math.floor(safe % 60);
    const millis = Math.floor((safe % 1) * 1000);
    return `${hours ? `${String(hours).padStart(2,"0")}:` : ""}${String(minutes).padStart(2,"0")}:${String(seconds).padStart(2,"0")}.${String(millis).padStart(3,"0")}`;
  };
  const editableTime = (value:number) => {
    const safe=Math.max(0,Number(value)||0),hours=Math.floor(safe/3600),minutes=Math.floor((safe%3600)/60),seconds=safe%60;
    const secondText=Math.abs(seconds-Math.round(seconds))<.0005?String(Math.round(seconds)).padStart(2,"0"):seconds.toFixed(3).padStart(6,"0");
    return `${hours}:${String(minutes).padStart(2,"0")}:${secondText}`;
  };
  const rangeTimelineTool = () => ["cut","gif","image_overlay"].includes(selected?.id??"");
  const timelineTime = (value:number) => rangeTimelineTool()?editableTime(value):playerTime(value);
  function parseTimecode(value:string){
    const parts=value.trim().split(":");
    if(parts.length<1||parts.length>3||parts.some(part=>part===""||!/^\d+(?:\.\d{1,3})?$/.test(part)))return null;
    const numbers=parts.map(Number);let seconds=0;
    if(numbers.length===1)seconds=numbers[0];
    else if(numbers.length===2){if(numbers[1]>=60)return null;seconds=numbers[0]*60+numbers[1]}
    else{if(numbers[1]>=60||numbers[2]>=60)return null;seconds=numbers[0]*3600+numbers[1]*60+numbers[2]}
    return Number.isFinite(seconds)?seconds:null;
  }
  function setScreenshotTime(value:string){
    const seconds=parseTimecode(value),duration=media?.duration??0;
    if(seconds===null||seconds>duration){showToast(language==="tr"?"Video süresi içinde geçerli bir zaman gir.":"Enter a valid timestamp within the video duration.","error");return}
    setToolNumber("timestamp",seconds);seekToolbox(seconds);
  }
  function setTimelineRange(start:number,end:number){
    setToolNumber("start",start);
    setToolNumber("end",end);
  }
  function setTimelineBoundary(key:TimelineBoundary,seconds:number){
    if(!media?.duration)return;
    const next=moveTimelineBoundary(timelineBounds(),key,seconds,media.duration);
    setTimelineRange(next.start,next.end);
  }
  function setCutTime(key:"start"|"end",value:string){
    const seconds=parseTimecode(value),duration=media?.duration??0;
    if(seconds===null||seconds<0||seconds>duration+.001){error=language==="tr"?"Geçerli bir zaman gir (S, M:S veya H:M:S).":"Enter a valid time (S, M:S or H:M:S).";return}
    const clamped=Math.min(seconds,duration);
    setTimelineBoundary(key,clamped);seekToolbox(clamped);error="";
  }
  function commitCutTime(key:"start"|"end"){
    const value=key==="start"?cutStartInput:cutEndInput;
    setCutTime(key,value);
    const bounds=timelineBounds(),formatted=editableTime(key==="start"?bounds.start:bounds.end);
    if(key==="start")cutStartInput=formatted;else cutEndInput=formatted;
    cutTimeEditing=null;
  }
  function handleCutTimeKey(event:KeyboardEvent,key:"start"|"end"){
    if(event.key==="Enter"){event.preventDefault();(event.currentTarget as HTMLInputElement).blur();return}
    if(event.key==="Escape"){event.preventDefault();const bounds=timelineBounds();if(key==="start")cutStartInput=editableTime(bounds.start);else cutEndInput=editableTime(bounds.end);(event.currentTarget as HTMLInputElement).blur()}
  }
  function markCutAtPlayhead(key:"start"|"end"){
    if(!rangeTimelineTool()||!media?.duration)return;
    const duration=media.duration,at=Math.max(0,Math.min(duration,toolboxCurrent));
    setTimelineBoundary(key,at);error="";
  }
  const basename = (path: string) => path.split(/[\\/]/).pop() ?? path;
  function previewSourceDimensions(){
    const current=toolboxVideo?.getAttribute("src")===mediaUrl&&toolboxVideo.videoWidth&&toolboxVideo.videoHeight;
    return current?{width:toolboxVideo!.videoWidth,height:toolboxVideo!.videoHeight}:{width:media?.width??0,height:media?.height??0};
  }
  const upscaleStandards = [720,1080,1440,2160,4320];

  function upscaleDimensions(targetEdge:number){
    if(!media?.width||!media?.height)return null;
    const {width:sourceWidth,height:sourceHeight}=previewSourceDimensions();
    const landscape=sourceWidth>=sourceHeight,ratio=sourceWidth/sourceHeight;
    const even=(value:number)=>Math.max(2,Math.round(value/2)*2);
    const width=landscape?even(targetEdge*ratio):even(targetEdge);
    const height=landscape?even(targetEdge):even(targetEdge/ratio);
    return {width,height,targetEdge};
  }
  function upscaleTargets(){
    if(!media?.width||!media?.height)return [];
    const source=previewSourceDimensions(),sourceEdge=Math.min(source.width,source.height);
    const names:Record<number,string>={720:"720p HD",1080:"1080p Full HD",1440:"1440p / 2K QHD",2160:"2160p / 4K UHD",4320:"4320p / 8K UHD"};
    return upscaleStandards.flatMap(targetEdge=>{
      const dimensions=upscaleDimensions(targetEdge);
      return targetEdge>sourceEdge&&dimensions&&Math.max(dimensions.width,dimensions.height)<=7680
        ? [{value:String(targetEdge),label:`${names[targetEdge]} · ${dimensions.width}×${dimensions.height}`}]
        : [];
    });
  }
  function configureUpscale(tool:Tool){
    if(tool.id!=="upscale")return;
    const field=tool.fields.find(item=>item.key==="target_edge");if(!field)return;
    const options=upscaleTargets();
    field.options=options.length?options:[{value:String(Math.min(media?.width??4320,media?.height??4320)),label:language==="tr"?"Daha yüksek standart hedef yok":"No higher standard target"}];
    field.value=field.options[0].value;
  }
  function configureTimelineFields(tool:Tool){
    if(!media?.duration)return;
    const mediaDuration=media.duration;
    for(const field of tool.fields){
      if(field.key==="end")field.value=["cut","image_overlay"].includes(tool.id)?mediaDuration:Math.min(5,mediaDuration);
      if(["start","end","duration","timestamp"].includes(field.key))field.max=mediaDuration;
    }
    if(["cut","gif","image_overlay"].includes(tool.id)){
      const startField=tool.fields.find(field=>field.key==="start");
      const endField=tool.fields.find(field=>field.key==="end");
      let start=Math.max(0,Math.min(mediaDuration,Number(startField?.value??0)));
      let end=Number(endField?.value??mediaDuration);
      // A five-second GIF is practically invisible on a long filmstrip and
      // looks like a closed/disabled selection. Give every range tool a clear
      // initial block while preserving precise short ranges the user creates.
      const visibleSpan=Math.min(mediaDuration,Math.max(5,mediaDuration*.08));
      if(end-start<visibleSpan){
        end=Math.min(mediaDuration,start+visibleSpan);
        start=Math.max(0,end-visibleSpan);
      }
      if(startField)startField.value=Math.round(start*1000)/1000;
      if(endField)endField.value=Math.min(Math.round(end*1000)/1000,mediaDuration);
    }
  }

  function populateAudioTrackOptions(tool:Tool,preserveValue=false){
    const field=tool.fields.find(item=>item.key==="audio_track");
    if(!field||!media)return;
    const previous=String(field.value);
    field.options=media.audio_tracks.map((track,position)=>{
      const languageLabel=track.language?` · ${track.language.toUpperCase()}`:"";
      const channelLabel=track.channel_layout??(track.channels?`${track.channels} ch`:"audio");
      const bitrateLabel=track.bitrate?` · ${Math.round(track.bitrate/1000)} kbps`:"";
      const defaultLabel=track.is_default?(language==="tr"?" · varsayılan":" · default"):"";
      return {value:String(track.index),label:`${language==="tr"?"Parça":"Track"} ${position+1} · ${track.codec.toUpperCase()} · ${channelLabel}${languageLabel}${bitrateLabel}${defaultLabel}`};
    });
    if(!preserveValue||!field.options.some(option=>option.value===previous))field.value=field.options[0]?.value??previous;
  }

  function restrictEncoderOptions(tool:Tool){
    if(tool.id!=="encode"||!availableEncoders)return;
    const field=tool.fields.find(item=>item.key==="encoder");
    if(!field)return;
    field.options=(field.options??[]).filter(option=>availableEncoders!.includes(option.value));
    if(!field.options.some(option=>option.value===String(field.value)))field.value=field.options[0]?.value??"libx264";
  }

  function chooseTool(tool: Tool) {
    stackResultPreview=false;
    editingStackStepId=null;
    const changed = selected?.id !== tool.id;
    if(changed){
      colorEnabled={};
      colorPreviewVisible=true;
      textLayers=[];
      activeTextId=null;
    }
    selected = localizedTool(tool,language);
    frameAdvanced=false;
    if(selected.id==="clipper"){clipperPreviewMode="source";setCropPreset("9:16",true);centerContentRegion()}
    if(selected.id==="merge_videos"&&media)mergeInputs=[media.path];
    if(selected.id==="subtitles"&&media)void loadSubtitleTracks();
    if(selected.id==="text")void ensureSystemFonts();
    configureUpscale(selected);
    configureTimelineFields(selected);
    if(selected.id==="frame_extractor"&&media?.duration){const end=selected.fields.find(field=>field.key==="end");if(end)end.value=Math.min(86400,media.duration)}
    restrictEncoderOptions(selected);
    error = "";
    output = "";
    renderedImageSize = 0;
    hashResult = "";
    if (changed) qualityAnalysis = null;
    if (media && selected.id === "interpolation" && media.fps) {
      const field = selected.fields.find((item) => item.key === "fps");
      if (field) field.value = Math.min(2400, Math.max(60, Math.ceil((media.fps + 0.001) / 60) * 60));
    }
    if (media && selected.id === "frame_blend" && media.fps) {
      const field = selected.fields.find((item) => item.key === "fps");
      const choices = numericPresets("frame_blend", field);
      if (field && Number(field.value) >= media.fps) field.value = choices.at(-1) ?? Math.max(1, Math.floor(media.fps / 2));
    }
    if(rangeTimelineTool()){const bounds=timelineBounds();cutStartInput=editableTime(bounds.start);cutEndInput=editableTime(bounds.end);cutTimeEditing=null}
    populateAudioTrackOptions(selected);
    if (["cut","screenshot","gif","image_overlay"].includes(selected.id)) void loadToolboxFilmstrip();
  }
  function resetSelectedTool(){
    if(!selected)return;
    const source=kindTools(activeKind).find(item=>item.id===selected?.id);
    if(source){
      const layout=selected.id==="clipper"?toolValue("vertical_layout"):null;
      selected=localizedTool(source,language);
      if(layout){const field=toolField("vertical_layout");if(field)field.value=layout}
      configureTimelineFields(selected);
      if(selected.id==="clipper"){setCropPreset("9:16",true);centerContentRegion()}
    }
    colorEnabled={};colorPreviewVisible=true;textLayers=[];activeTextId=null;qualityAnalysis=null;error="";
  }
  function toolField(key:string){return selected?.fields.find(field=>field.key===key)}
  function fieldLivesOnTimeline(key:string){return ["cut","gif","image_overlay"].includes(selected?.id??"")?["start","end"].includes(key):selected?.id==="screenshot"?key==="timestamp":false}
  function fieldVisible(key:string){
    if(selected?.id==="audio_lab"){
      if(key==="lufs")return toolValue("preset")==="custom";
      if(key==="format")return media?.kind==="audio";
    }
    if(selected?.id==="frame_extractor"){
      if(!frameAdvanced)return false;
      const mode=toolValue("mode");
      if(key==="interval")return mode==="seconds";
      if(key==="every_frames")return mode==="frames";
      if(key==="scene_threshold")return mode==="scene";
      if(key==="count")return mode==="even"||mode==="sheet";
      if(key==="columns")return mode==="sheet";
    }
    if(["transform","clipper"].includes(selected?.id??"") && key!=="crf") return false;
    if(selected?.id==="color" || selected?.id==="text") return false;
    if(selected?.id==="compression"){
      if(!qualityAdvanced)return false;
      if(toolValue("mode")==="bitrate")return ["mode","mbps"].includes(key);
      return key!=="mbps";
    }
    if(selected?.id==="merge_videos")return ["container","mode"].includes(key);
    if(selected?.id==="blur_pixelate"&&key.startsWith("region_"))return false;
    if(selected?.id==="image_overlay")return media?.kind==="image"?["image_path","opacity"].includes(key):["image_path","opacity","start","end"].includes(key);
    if(selected?.id==="image_compressor"){
      if(key==="quality")return toolValue("mode")==="quality"&&imageQualityAdjustable(imageOutputFormat(media?.path??"",toolValue("format")));
      if(key==="target_kb")return toolValue("mode")==="target";
      if(key==="png_mode")return imageOutputFormat(media?.path??"",toolValue("format"))==="png";
      if(key==="jpeg_background")return imageOutputFormat(media?.path??"",toolValue("format"))==="jpg";
    }
    if(selected?.id==="subtitles"){
      const action=toolValue("action");
      if(key==="subtitle_path")return ["add","burn"].includes(action);
      if(key==="container")return action==="add";
      if(key==="subtitle_track")return action==="extract";
    }
    if(key==="audio_track") return String(toolField("audio_mode")?.value)==="selected";
    if(selected?.id==="cut"&&key==="crf") return false;
    if(selected?.id==="speed"&&key==="crf") return String(toolField("speed_mode")?.value)!=="lossless_video";
    if(selected?.id==="potatoify"&&["fps","video_badness","audio_badness","shrink"].includes(key)) return String(toolField("profile")?.value)==="custom";
    if(selected?.id==="image_potatoify"&&["quality","times","scale"].includes(key)) return String(toolField("profile")?.value)==="custom";
    return true;
  }
  function toolNumber(key:string){return Number(toolField(key)?.value??0)}
  function setToolNumber(key:string,value:number){const field=toolField(key);if(field)field.value=key==="end"&&media?.duration?Math.min(Math.round(value*1000)/1000,media.duration):Math.round(value*1000)/1000}
  function toolValue(key:string){return String(toolField(key)?.value??"")}
  function setToolValue(key:string,value:string){const field=toolField(key);if(field)field.value=value}
  $effect(()=>{
    if(!rangeTimelineTool())return;
    const {start,end}=timelineBounds();
    if(cutTimeEditing!=="start")cutStartInput=editableTime(start);
    if(cutTimeEditing!=="end")cutEndInput=editableTime(end);
  });
  async function loadSubtitleTracks(){
    if(!media)return;
    const path=media.path,id=++subtitleLoadId;
    try{
      const tracks=await invoke<SubtitleTrack[]>("probe_subtitles",{path});
      if(id!==subtitleLoadId||media?.path!==path||selected?.id!=="subtitles")return;
      subtitleTracks=tracks;
      const field=toolField("subtitle_track");
      if(field){field.options=subtitleTracks.map((track,index)=>({value:String(track.index),label:`${language==="tr"?"Parça":"Track"} ${index+1} · ${track.codec.toUpperCase()}${track.language?` · ${track.language.toUpperCase()}`:""}${track.title?` · ${track.title}`:""}`}));field.value=field.options[0]?.value??""}
    }catch(reason){if(id===subtitleLoadId&&media?.path===path){subtitleTracks=[];reportProblem(reason)}}
  }
  async function addMergeVideos(){
    const paths=await open({multiple:true,filters:[{name:"Video",extensions:["mp4","mkv","mov","avi","webm","m4v"]}]});
    if(Array.isArray(paths))mergeInputs=[...mergeInputs,...paths.filter(path=>!mergeInputs.includes(path))];
  }
  function moveMergeVideo(index:number,delta:number){const target=index+delta;if(target<0||target>=mergeInputs.length)return;const copy=[...mergeInputs];[copy[index],copy[target]]=[copy[target],copy[index]];mergeInputs=copy}
  function removeMergeVideo(index:number){mergeInputs=mergeInputs.filter((_,position)=>position!==index)}
  function resetColorFilters(){
    if(selected?.id!=="color")return;
    const source=kindTools(activeKind).find(tool=>tool.id==="color");
    if(source)selected=localizedTool(source,language);
    colorEnabled={};
    colorPreviewVisible=true;
  }
  function colorOn(key:string){return !!colorEnabled[key]}
  function neutralColorValue(key:string){return ["contrast","saturation","gamma"].includes(key)?100:key==="temperature"?6500:0}
  function displayedColorValue(key:string){return colorOn(key)?toolNumber(key):neutralColorValue(key)}
  function adjustColor(key:string,value:number){setToolNumber(key,value);colorEnabled={...colorEnabled,[key]:value!==neutralColorValue(key)}}
  function resetColorKey(key:string){
    setToolNumber(key,neutralColorValue(key));
    colorEnabled={...colorEnabled,[key]:false};
  }
  function colorValueLabel(key:string){const value=displayedColorValue(key);return key==="temperature"?`${value} K`:key==="hue"?`${value}°`:`${value}%`}
  function colorPreviewStyle(){
    if(selected?.id!=="color"||!colorPreviewVisible)return "";
    const filters:string[]=[];
    if(colorOn("brightness"))filters.push(`brightness(${Math.max(0,1+toolNumber("brightness")/100)})`);
    if(colorOn("contrast"))filters.push(`contrast(${toolNumber("contrast")/100})`);
    if(colorOn("saturation"))filters.push(`saturate(${toolNumber("saturation")/100})`);
    if(colorOn("gamma"))filters.push(`brightness(${Math.pow(toolNumber("gamma")/100,.55)})`);
    if(colorOn("hue"))filters.push(`hue-rotate(${toolNumber("hue")}deg)`);
    if(colorOn("temperature")){
      const warmth=Math.max(-1,Math.min(1,(6500-toolNumber("temperature"))/5500));
      if(warmth>0)filters.push(`sepia(${warmth*.28}) saturate(${1+warmth*.18})`);
      else if(warmth<0)filters.push(`sepia(${-warmth*.12}) hue-rotate(175deg) saturate(${1-warmth*.1})`);
    }
    if(colorOn("sharpen"))filters.push(`contrast(${1+toolNumber("sharpen")/500})`);
    if(colorOn("blur"))filters.push(`blur(${toolNumber("blur")/20}px)`);
    if(toolValue("denoise")!=="off")filters.push(`blur(${({low:.2,medium:.45,high:.8} as Record<string,number>)[toolValue("denoise")]??0}px)`);
    if(colorOn("vignette"))filters.push(`brightness(${1-toolNumber("vignette")/700})`);
    if(toolValue("grayscale")==="on")filters.push("grayscale(1)");
    return filters.length?`filter:${filters.join(" ")}`:"";
  }
  function neutralPreviewStyle(){
    const box=mediaDisplayBox();if(!box)return "";
    return `position:absolute;left:50%;top:50%;width:${box.width}px;height:${box.height}px;transform:translate(-50%,-50%)`;
  }
  function previewVideoStyle(){
    if(clipperOutputVisible)return "position:absolute;opacity:0;pointer-events:none;width:1px;height:1px";
    if(selected?.id==="clipper"&&["original","blur","fill"].includes(toolValue("vertical_layout"))){
      const box=verticalOutputBox();if(!box)return "";
      if(toolValue("vertical_layout")==="fill"){
        const maxX=Math.max(0,100-toolNumber("crop_w")),maxY=Math.max(0,100-toolNumber("crop_h"));
        const positionX=maxX?toolNumber("crop_x")/maxX*100:50,positionY=maxY?toolNumber("crop_y")/maxY*100:50;
        const zoom=toolNumber("clipper_zoom")/100;
        return `position:absolute;left:${-(zoom-1)*toolNumber("clipper_x")}%;top:${-(zoom-1)*toolNumber("clipper_y")}%;width:100%;height:100%;max-width:none;max-height:none;object-fit:cover;object-position:${positionX}% ${positionY}%;background:#000;transform:scale(${zoom});transform-origin:top left`;
      }
      if(["original","blur"].includes(toolValue("vertical_layout"))){
        const frame=blurForegroundGeometry();if(!frame)return "display:none";
        return `position:absolute;left:${-(frame.scaledWidth-frame.box.width)*toolNumber("clipper_x")/100}px;top:${-(frame.scaledHeight-frame.height)*toolNumber("clipper_y")/100}px;width:${frame.scaledWidth}px;height:${frame.scaledHeight}px;max-width:none;max-height:none;object-fit:fill`;
      }
      return "";
    }
    const geometry=["transform","clipper"].includes(selected?.id??"")?transformPreviewStyle():neutralPreviewStyle();
    return `${geometry};${colorPreviewStyle()}`;
  }

  function verticalOutputBox(){
    if(!toolboxCanvas)return null;
    const stageWidth=transformCanvasWidth||toolboxCanvas.clientWidth,stageHeight=transformCanvasHeight||toolboxCanvas.clientHeight;
    const availableWidth=Math.max(1,stageWidth-16),availableHeight=Math.max(1,stageHeight-16),ratio=9/16;
    let width=availableHeight*ratio,height=availableHeight;if(width>availableWidth){width=availableWidth;height=width/ratio}
    return {left:(stageWidth-width)/2,top:(stageHeight-height)/2,width,height};
  }
  function blurForegroundGeometry(){
    const box=verticalOutputBox();if(!box)return null;
    const source=previewSourceDimensions(),zoom=toolNumber("clipper_zoom")/100;
    const naturalHeight=box.width*source.height/Math.max(1,source.width);
    const height=Math.min(box.height,naturalHeight);
    const scaledWidth=box.width*zoom,scaledHeight=naturalHeight*zoom;
    const top=box.top+(box.height-height)/2;
    return {box,scaledWidth,scaledHeight,height,top};
  }
  function blurForegroundBoxStyle(){
    if(selected?.id!=="clipper")return "display:contents";
    if(toolValue("vertical_layout")==="fill"){
      const box=verticalOutputBox();return box?`position:absolute;z-index:2;overflow:hidden;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px`:"display:none";
    }
    if(!["original","blur"].includes(toolValue("vertical_layout")))return "display:contents";
    const frame=blurForegroundGeometry();if(!frame)return "display:none";
    return `position:absolute;z-index:2;overflow:hidden;left:${frame.box.left}px;top:${frame.top}px;width:${frame.box.width}px;height:${frame.height}px`;
  }
  function blurPanLayerStyle(){
    const frame=blurForegroundGeometry();if(!frame)return "display:none";
    return `left:${frame.box.left}px;top:${frame.top}px;width:${frame.box.width}px;height:${frame.height}px`;
  }
  function originalCanvasStyle(){
    const box=verticalOutputBox();if(!box)return "display:none";
    const background=toolValue("canvas_background"),color=background==="white"?"#fff":background==="custom"?toolValue("canvas_color"):"#000";
    return `position:absolute;z-index:1;pointer-events:none;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;background:${color}`;
  }

  function activeText(){return textLayers.find(layer=>layer.id===activeTextId)??null}
  function persistTextPresets(next:typeof textPresets){
    try{localStorage.setItem("container-text-presets-v1",JSON.stringify(next));textPresets=next;return true}
    catch{showToast(language==="tr"?"Yazı preseti kaydedilemedi. Depolama alanını kontrol et.":"Could not save text presets. Check available storage.","info");return false}
  }
  function saveTextPreset(){
    const layer=activeText(),name=textPresetName.trim();if(!layer||!name)return;
    const existing=textPresets.find(preset=>preset.name.toLocaleLowerCase()===name.toLocaleLowerCase());
    if(existing){selectedTextPreset=existing.id;showToast(language==="tr"?"Bu ad zaten var. Seçili preseti güncelle veya farklı bir ad kullan.":"This name already exists. Update the selected preset or use another name.","info");return}
    const preset={id:crypto.randomUUID(),name,sourceWidth:previewSourceDimensions().width||media?.width||undefined,layer:cloneEditorValue(layer)};
    if(persistTextPresets([...textPresets,preset])){selectedTextPreset=preset.id;textPresetName="";showToast(language==="tr"?"Yazı preseti kaydedildi.":"Text preset saved.","info")}
  }
  function deleteTextPreset(){if(persistTextPresets(textPresets.filter(preset=>preset.id!==selectedTextPreset)))selectedTextPreset=""}
  function updateTextPreset(){
    const layer=activeText();if(!layer||!selectedTextPreset)return;
    if(persistTextPresets(textPresets.map(preset=>preset.id===selectedTextPreset?{...preset,sourceWidth:previewSourceDimensions().width||media?.width||undefined,layer:cloneEditorValue(layer)}:preset)))showToast(language==="tr"?"Preset güncellendi.":"Preset updated.","info");
  }
  function renameTextPreset(){
    const name=textPresetName.trim();if(!name||!selectedTextPreset)return;
    if(textPresets.some(preset=>preset.id!==selectedTextPreset&&preset.name.toLocaleLowerCase()===name.toLocaleLowerCase())){showToast(language==="tr"?"Bu ad zaten kullanılıyor. Başka bir ad gir.":"This name is already used. Enter another name.","info");return}
    if(persistTextPresets(textPresets.map(preset=>preset.id===selectedTextPreset?{...preset,name}:preset))){textPresetName="";showToast(language==="tr"?"Presetin adı değiştirildi.":"Preset renamed.","info")}
  }
  async function applyTextPreset(){
    const preset=textPresets.find(item=>item.id===selectedTextPreset);if(!preset||textPresetBusy)return;
    const sourceMedia=media;const sourceTool=selected;textPresetBusy=true;
    try{
      const fonts=await ensureSystemFonts();
      const font=fonts.find(item=>item.path.toLowerCase()===preset.layer.font_path.toLowerCase());
      if(!font){showToast(language==="tr"?"Presetin fontu bu bilgisayarda bulunamadı.":"This preset's font is unavailable on this computer.","info");return}
      const fontName=await loadPreviewFont(font);
      if(media!==sourceMedia||selected!==sourceTool)return;
      const layer={...cloneEditorValue(preset.layer),id:nextTextId++,fontName,font_path:font.path};
      // Text coordinates are percentages, but font/effect sizes are source pixels.
      // Keep their visual proportions when applying to a different resolution.
      const width=previewSourceDimensions().width||media?.width||0;
      if(Number.isFinite(preset.sourceWidth)&&preset.sourceWidth!>0&&width>0){
        const scale=width/preset.sourceWidth!;
        layer.size*=scale;layer.outline*=scale;layer.shadow*=scale;layer.background_padding*=scale;
      }else{
        showToast(language==="tr"?"Eski preset piksel boyutuyla uygulandı. Çözünürlüğe uyarlamak için doğru boyutta yeniden kaydet.":"Legacy preset applied at its saved pixel size. Save it again at the correct size to enable resolution scaling.","info");
      }
      textLayers=[...textLayers,layer];activeTextId=layer.id;
    }catch(reason){reportProblem(reason)}finally{textPresetBusy=false}
  }
  async function ensureSystemFonts(){
    if(systemFonts.length)return systemFonts;
    systemFontsLoad??=invoke<FontOption[]>("list_system_fonts")
      .then(fonts=>{systemFonts=fonts;return fonts})
      .catch(reason=>{systemFontsLoad=null;error=String(reason);reportProblem(reason);return []});
    return systemFontsLoad;
  }
  async function addTextLayer(){
    const fonts=await ensureSystemFonts();
    const font=fonts.find(item=>item.name.toLowerCase()==="impact")??fonts.find(item=>item.name.toLowerCase().startsWith("arial"))??fonts[0];
    if(!font){error=language==="tr"?"Bilgisayarda kullanılabilir font bulunamadı.":"No usable system font was found.";return}
    let fontName:string;
    try{fontName=await loadPreviewFont(font)}catch(reason){error=String(reason);reportProblem(reason);return}
    const layer:TextLayer={id:nextTextId++,text:`${language==="tr"?"Yazı":"Text"} ${textLayers.length+1}`,x:50,y:50,size:64,color:"#ffffff",opacity:100,align:"center",fontName,font_path:font.path,outline:0,outline_color:"#000000",shadow:0,shadow_color:"#000000",background:false,background_color:"#000000",background_opacity:65,background_padding:12};
    textLayers=[...textLayers,layer];activeTextId=layer.id;
  }
  function updateTextLayer(patch:Partial<TextLayer>){textLayers=textLayers.map(layer=>layer.id===activeTextId?{...layer,...patch}:layer)}
  function resizeTextLayer(size:number){const layer=activeText();if(layer)updateTextLayer({size,wrap_width:layer.wrap_width??textLayerAvailableWidth(layer)/layer.size})}
  function removeTextLayer(id:number){fontSelectionVersions.delete(id);textLayers=textLayers.filter(layer=>layer.id!==id);if(activeTextId===id)activeTextId=textLayers[0]?.id??null}
  function textLayerStyle(layer:TextLayer){
    const box=mediaDisplayBox();if(!box||!media?.width)return "display:none";
    const scale=box.width/(previewSourceDimensions().width||media.width);
    const outline=Math.max(0,layer.outline*scale),shadow=Math.max(0,layer.shadow*scale),padding=Math.max(0,layer.background_padding*scale);
    const translate=layer.align==="left"?"0":layer.align==="right"?"-100%":"-50%";
    return `left:${(box.stageWidth-box.width)/2+box.width*layer.x/100}px;top:${(box.stageHeight-box.height)/2+box.height*layer.y/100}px;transform:translate(${translate},-50%);text-align:${layer.align};font-size:${Math.max(1,layer.size*scale)}px;color:${hexWithAlpha(layer.color,layer.opacity)};font-family:${JSON.stringify(layer.fontName)};font-weight:400;font-style:normal;-webkit-text-stroke:${outline}px ${hexWithAlpha(layer.outline_color,layer.opacity)};paint-order:stroke fill;text-shadow:${shadow?`${shadow}px ${shadow}px 0 ${hexWithAlpha(layer.shadow_color,layer.opacity*.75)}`:"none"};background:${layer.background?hexWithAlpha(layer.background_color,layer.background_opacity):"transparent"};padding:${layer.background?`${padding}px`:"0"}`;
  }
  function textPreviewCanvasStyle(){
    const box=mediaDisplayBox();if(!box)return "display:none";
    return `left:${(box.stageWidth-box.width)/2}px;top:${(box.stageHeight-box.height)/2}px;width:${box.width}px;height:${box.height}px`;
  }
  function textLayerAvailableWidth(layer:TextLayer,canvasWidth=previewSourceDimensions().width||media?.width||1){
    const sourceWidth=canvasWidth,x=Math.max(0,Math.min(1,layer.x/100));
    const anchorWidth=layer.align==="left"?1-x:layer.align==="right"?x:2*Math.min(x,1-x);
    const safeWidth=sourceWidth*Math.min(.9,Math.max(.05,anchorWidth));
    const effects=(layer.background?layer.background_padding*2:0)+layer.outline+Math.max(0,layer.shadow);
    return layer.wrap_width?Math.max(layer.size,layer.wrap_width*layer.size):Math.max(layer.size,safeWidth-effects);
  }
  function wrappedText(layer:TextLayer,canvasWidth?:number){
    textMeasureCanvas??=document.createElement("canvas");
    const context=textMeasureCanvas.getContext("2d");
    if(!context)return layer.text;
    context.font=`400 ${layer.size}px ${JSON.stringify(layer.fontName)}, "Segoe UI Emoji", sans-serif`;
    const maxWidth=textLayerAvailableWidth(layer,canvasWidth),lines:string[]=[];
    const fits=(value:string)=>context.measureText(value).width<=maxWidth;
    for(const paragraph of layer.text.replace(/\r\n?/g,"\n").split("\n")){
      if(!paragraph){lines.push("");continue}
      let line="";
      for(const word of paragraph.trim().split(/\s+/)){
        const candidate=line?`${line} ${word}`:word;
        if(fits(candidate)){line=candidate;continue}
        if(line){lines.push(line);line=""}
        // Usernames and other unbroken words remain intact. Wrap only at
        // whitespace; explicit Enter breaks are preserved by the outer loop.
        line=word;
      }
      lines.push(line);
    }
    return lines.join("\n");
  }
  function canvasColor(color:string,opacity:number){
    if(!/^#[0-9a-f]{6}$/i.test(color))return "rgba(0,0,0,0)";
    const value=parseInt(color.slice(1),16),red=value>>16,green=value>>8&255,blue=value&255;
    return `rgba(${red},${green},${blue},${Math.max(0,Math.min(100,opacity))/100})`;
  }
  function renderTextPreview(){
    const canvas=textPreviewCanvas,source=previewSourceDimensions();
    if(!canvas||selected?.id!=="text"||!source.width||!source.height)return;
    const width=Math.max(1,Math.round(source.width)),height=Math.max(1,Math.round(source.height));
    drawTextLayers(canvas,textLayers,width,height);
  }
  function drawTextLayers(canvas:HTMLCanvasElement,layers:TextLayer[],width:number,height:number){
    if(canvas.width!==width)canvas.width=width;if(canvas.height!==height)canvas.height=height;
    const context=canvas.getContext("2d");if(!context)return;
    context.clearRect(0,0,width,height);
    for(const layer of layers){
      const lines=wrappedText(layer,width).split("\n"),lineHeight=layer.size*1.05,totalHeight=lineHeight*lines.length;
      context.font=`400 ${layer.size}px ${JSON.stringify(layer.fontName)}, "Segoe UI Emoji", sans-serif`;
      context.textAlign=layer.align;context.textBaseline="middle";
      const anchorX=width*layer.x/100,centerY=height*layer.y/100;
      const widest=Math.max(0,...lines.map(line=>context.measureText(line).width));
      const left=layer.align==="left"?anchorX:layer.align==="right"?anchorX-widest:anchorX-widest/2;
      if(layer.background){
        context.fillStyle=canvasColor(layer.background_color,layer.background_opacity);
        context.fillRect(left-layer.background_padding,centerY-totalHeight/2-layer.background_padding,widest+layer.background_padding*2,totalHeight+layer.background_padding*2);
      }
      context.shadowOffsetX=layer.shadow;context.shadowOffsetY=layer.shadow;context.shadowBlur=0;
      context.shadowColor=canvasColor(layer.shadow_color,layer.opacity*.75);
      context.lineJoin="round";context.miterLimit=2;context.lineWidth=layer.outline;
      context.strokeStyle=canvasColor(layer.outline_color,layer.opacity);
      context.fillStyle=canvasColor(layer.color,layer.opacity);
      for(const [index,line] of lines.entries()){
        const y=centerY-totalHeight/2+lineHeight*(index+.5);
        if(layer.outline>0)context.strokeText(line,anchorX,y);
        context.fillText(line,anchorX,y);
      }
      context.shadowColor="transparent";
    }
  }
  $effect(()=>{
    textLayers;selected?.id;transformCanvasWidth;transformCanvasHeight;media?.width;media?.height;
    queueMicrotask(renderTextPreview);
  });
  function previewFontAlias(path:string){let hash=2166136261;for(const character of path){hash^=character.charCodeAt(0);hash=Math.imul(hash,16777619)}return `container-font-${(hash>>>0).toString(16)}`}
  async function loadPreviewFont(font:FontOption){
    const alias=previewFontAlias(font.path);
    const existing=previewFontLoads.get(alias);if(existing)return existing;
    const loading=(async()=>{const source=await invoke<string>("font_preview_data",{path:font.path});const encoded=source.slice(source.indexOf(",")+1),binary=atob(encoded),bytes=new Uint8Array(binary.length);for(let index=0;index<binary.length;index++)bytes[index]=binary.charCodeAt(index);const face=await new FontFace(alias,bytes.buffer).load();document.fonts.add(face);return alias})().catch(reason=>{previewFontLoads.delete(alias);throw new Error(`${font.name}: ${String(reason)}`)});
    previewFontLoads.set(alias,loading);return loading;
  }
  async function restorePreviewFonts(){
    const layers=[...textLayers];
    if(!layers.length)return;
    const fonts=await ensureSystemFonts();
    const fallback=fonts.find(item=>item.name.toLowerCase()==="impact")??fonts.find(item=>item.name.toLowerCase().startsWith("arial"))??fonts[0];
    const restored=await Promise.all(layers.map(async layer=>{
      const font=fonts.find(candidate=>candidate.path.toLowerCase()===layer.font_path.toLowerCase())??fallback;
      if(!font)return layer;
      try{return {...layer,fontName:await loadPreviewFont(font),font_path:font.path}}catch(reason){reportProblem(reason);return layer}
    }));
    if(JSON.stringify(layers)===JSON.stringify(textLayers)){
      textLayers=restored;
      nextTextId=Math.max(0,...restored.map(layer=>layer.id))+1;
    }
  }
  async function chooseTextFont(path:string){
    const font=systemFonts.find(item=>item.path===path),layerId=activeTextId;
    if(!font||layerId===null)return;
    const version=(fontSelectionVersions.get(layerId)??0)+1;
    fontSelectionVersions.set(layerId,version);
    try{
      const fontName=await loadPreviewFont(font);
      if(fontSelectionVersions.get(layerId)!==version||!textLayers.some(layer=>layer.id===layerId))return;
      textLayers=textLayers.map(layer=>layer.id===layerId?{...layer,fontName,font_path:font.path}:layer);
      error="";
    }catch(reason){
      if(fontSelectionVersions.get(layerId)!==version)return;
      error=String(reason);reportProblem(reason);
    }
  }
  function positionText(position:string){
    const layer=activeText();if(!layer)return;
    const [vertical,horizontal]=position.split("-");
    const x=horizontal==="left"?7:horizontal==="right"?93:50,y=vertical==="top"?8:vertical==="bottom"?92:50;
    updateTextLayer({x,y,align:horizontal==="left"?"left":horizontal==="right"?"right":"center"});
  }
  function setTextColor(value:string){if(/^#[0-9a-f]{6}$/i.test(value))updateTextLayer({color:value.toLowerCase()})}
  function hexWithAlpha(color:string,opacity:number){return /^#[0-9a-f]{6}$/i.test(color)?`${color}${Math.round(Math.max(0,Math.min(100,opacity))*2.55).toString(16).padStart(2,"0")}`:"transparent"}
  const textColors=["#ffffff","#000000","#00f1ff","#38d67a","#e7c84f","#fa646d","#6ba8ff","#d85cff"];
  function applyColorPreset(preset:string){
    resetColorFilters();
    const apply=(values:Record<string,number>)=>{for(const [key,value] of Object.entries(values)){setToolNumber(key,value);colorEnabled={...colorEnabled,[key]:true}}};
    if(preset==="natural")apply({contrast:105,saturation:105,sharpen:18});
    if(preset==="cinematic")apply({contrast:112,saturation:88,temperature:5600,vignette:28});
    if(preset==="warm")apply({temperature:5000,saturation:108,contrast:104});
    if(preset==="cold")apply({temperature:8500,saturation:103,contrast:106});
    if(preset==="bw"){apply({contrast:112});setToolValue("grayscale","on")}
  }
  function setQualityMode(advanced:boolean){qualityAdvanced=advanced;localStorage.setItem("container-quality-mode",advanced?"advanced":"simple")}
  function applyQualityProfile(profile:"high"|"balanced"|"small"){
    const values={high:{crf:16,preset:"slow",goal:"high"},balanced:{crf:20,preset:"veryfast",goal:"balanced"},small:{crf:24,preset:"veryfast",goal:"small"}}[profile];
    setToolNumber("crf",values.crf);setToolValue("preset",values.preset);setToolValue("goal",values.goal);qualityAnalysis=null;
  }
  function trackEditorPointer(event:PointerEvent,move:(event:PointerEvent)=>void,onStop?:()=>void){
    const pointerId=event.pointerId,target=event.currentTarget as HTMLElement;
    target.setPointerCapture?.(pointerId);
    const onMove=(next:PointerEvent)=>{if(next.pointerId===pointerId)move(next)};
    const stop=(next?:PointerEvent)=>{
      if(next&&next.pointerId!==pointerId)return;
      window.removeEventListener("pointermove",onMove);
      window.removeEventListener("pointerup",stop);
      window.removeEventListener("pointercancel",stop);
      window.removeEventListener("blur",onBlur);
      if(target.hasPointerCapture?.(pointerId))target.releasePointerCapture(pointerId);
      onStop?.();
    };
    const onBlur=()=>stop();
    window.addEventListener("pointermove",onMove);
    window.addEventListener("pointerup",stop);
    window.addEventListener("pointercancel",stop);
    window.addEventListener("blur",onBlur);
  }
  function toolboxPanelSizes(){
    const compact=toolboxWorkspaceWidth<=950,padding=compact?7:10;
    const available=Math.max(0,toolboxWorkspaceWidth-padding*2-16);
    const minLeft=compact?185:230,minRight=compact?225:280,minCenter=compact?320:360;
    const left=Math.max(minLeft,Math.min(560,available-minRight-minCenter,toolboxLeftWidth??available*.19));
    const right=Math.max(minRight,Math.min(620,available-left-minCenter,toolboxRightWidth??available*.24));
    return {left,right,available,minLeft,minRight,minCenter};
  }
  $effect(()=>{
    const element=toolboxWorkspace;if(!element)return;
    const update=()=>{toolboxWorkspaceWidth=element.clientWidth};
    update();const observer=new ResizeObserver(update);observer.observe(element);
    return ()=>observer.disconnect();
  });
  function saveToolboxPanelWidths(){
    try{localStorage.setItem("container-toolbox-panel-widths",JSON.stringify({left:toolboxLeftWidth,right:toolboxRightWidth}))}catch{}
  }
  function setToolboxPanelWidth(side:"left"|"right",value:number){
    const sizes=toolboxPanelSizes();
    if(side==="left")toolboxLeftWidth=Math.round(Math.max(sizes.minLeft,Math.min(560,sizes.available-sizes.right-sizes.minCenter,value)));
    else toolboxRightWidth=Math.round(Math.max(sizes.minRight,Math.min(620,sizes.available-sizes.left-sizes.minCenter,value)));
  }
  function startToolboxPanelResize(event:PointerEvent,side:"left"|"right"){
    if(event.button!==0||!toolboxWorkspace)return;
    event.preventDefault();const target=event.currentTarget as HTMLElement,pointerId=event.pointerId;
    const startX=event.clientX,sizes=toolboxPanelSizes(),initial=side==="left"?sizes.left:sizes.right;
    toolboxLeftWidth=sizes.left;toolboxRightWidth=sizes.right;
    target.setPointerCapture?.(pointerId);
    const oldCursor=document.body.style.cursor,oldSelection=document.body.style.userSelect;
    document.body.style.cursor="col-resize";document.body.style.userSelect="none";
    const move=(next:PointerEvent)=>{if(next.pointerId===pointerId)setToolboxPanelWidth(side,initial+(next.clientX-startX)*(side==="left"?1:-1))};
    const stop=(next?:PointerEvent)=>{
      if(next&&next.pointerId!==pointerId)return;
      window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",stop);window.removeEventListener("pointercancel",stop);window.removeEventListener("blur",onBlur);
      if(target.hasPointerCapture?.(pointerId))target.releasePointerCapture(pointerId);
      document.body.style.cursor=oldCursor;document.body.style.userSelect=oldSelection;
      saveToolboxPanelWidths();
    };
    const onBlur=()=>stop();
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",stop);window.addEventListener("pointercancel",stop);window.addEventListener("blur",onBlur);
  }
  function toolboxPanelKey(event:KeyboardEvent,side:"left"|"right"){
    const sizes=toolboxPanelSizes();let value=side==="left"?sizes.left:sizes.right;
    if(event.key==="Home")value=side==="left"?sizes.minLeft:sizes.minRight;
    else if(event.key==="End")value=side==="left"?Math.min(560,sizes.available-sizes.right-sizes.minCenter):Math.min(620,sizes.available-sizes.left-sizes.minCenter);
    else if(event.key==="ArrowLeft"||event.key==="ArrowRight")value+=(event.key==="ArrowRight"?1:-1)*(side==="left"?1:-1)*(event.shiftKey?50:20);
    else return;
    event.preventDefault();setToolboxPanelWidth(side,value);saveToolboxPanelWidths();
  }
  function resetToolboxPanelWidths(){toolboxLeftWidth=null;toolboxRightWidth=null;saveToolboxPanelWidths()}
  function mountPanelResetDialog(node:HTMLDialogElement){
    node.showModal();
    node.querySelector<HTMLButtonElement>(".panel-reset-cancel")?.focus();
    return {destroy(){if(node.open)node.close()}};
  }
  function confirmResetPanelWidths(){
    if(workspaceMode==="autocut")autoCutWorkspace?.resetPanelWidths();
    else if(workspaceMode==="batch")batchWorkspace?.resetPanelWidths();
    else resetToolboxPanelWidths();
    panelResetDialogOpen=false;
  }
  function startTextDrag(event:PointerEvent,layer:TextLayer,resizeDirection:-1|0|1=0,verticalDirection:-1|0|1=0){
    if(!toolboxCanvas||!media?.width)return;event.preventDefault();event.stopPropagation();activeTextId=layer.id;
    const box=mediaDisplayBox();if(!box)return;const startX=event.clientX,startY=event.clientY,origin={...layer};
    const rect=(event.currentTarget as HTMLElement).closest(".preview-text")?.getBoundingClientRect();
    const sourceLeft=(box.stageWidth-box.width)/2+toolboxCanvas.getBoundingClientRect().left;
    const alignment=layer.align==="left"?0:layer.align==="right"?1:.5;
    const wrapWidth=layer.wrap_width??textLayerAvailableWidth(layer)/layer.size;
    const move=(moveEvent:PointerEvent)=>{
      if(resizeDirection&&rect){
        const proposedWidth=Math.max(1,rect.width+(moveEvent.clientX-startX)*resizeDirection+(moveEvent.clientY-startY)*verticalDirection*.5);
        const size=Math.max(8,Math.min(600,origin.size*proposedWidth/Math.max(1,rect.width)));
        const newWidth=rect.width*size/origin.size;
        const anchor=resizeDirection>0?rect.left+alignment*newWidth:rect.right-(1-alignment)*newWidth;
        updateTextLayer({size,wrap_width:wrapWidth,x:Math.max(0,Math.min(100,(anchor-sourceLeft)/box.width*100))});
        return;
      }
      let dx=(moveEvent.clientX-startX)/box.width*100,dy=(moveEvent.clientY-startY)/box.height*100;
      if(moveEvent.shiftKey){if(Math.abs(dx)>=Math.abs(dy))dy=0;else dx=0}
      updateTextLayer({x:Math.max(0,Math.min(100,origin.x+dx)),y:Math.max(0,Math.min(100,origin.y+dy))});
    };
    trackEditorPointer(event,move);
  }
  const transformPresets = ["off","free","16:9","9:16","1:1","4:5","5:4","4:3","3:4","2:3","3:2","191:100"];
  const transformHandles = ["nw","n","ne","e","se","s","sw","w"] as const;
  const colorGroups=[
    {title:"Color Adjustments",keys:["brightness","contrast","saturation","gamma"]},
    {title:"Tone",keys:["hue","temperature"]},
    {title:"Detail",keys:["sharpen","blur"]},
    {title:"Cleanup",keys:["deband"]},
    {title:"Style",keys:["vignette"]},
  ];
  const colorLabels:Record<string,string>={brightness:"Brightness",contrast:"Contrast",saturation:"Saturation",gamma:"Gamma",hue:"Hue",temperature:"Temperature",sharpen:"Sharpen",blur:"Gaussian Blur",deband:"Deband",vignette:"Vignette"};
  function mediaDisplayBox(){
    toolboxMetadataVersion;
    if(!toolboxCanvas||!media?.width||!media?.height)return null;
    const {width:videoWidth,height:videoHeight}=previewSourceDimensions();
    const stageWidth=transformCanvasWidth||toolboxCanvas.clientWidth,stageHeight=transformCanvasHeight||toolboxCanvas.clientHeight,ratio=videoWidth/videoHeight;
    const availableWidth=Math.max(1,stageWidth-16),availableHeight=Math.max(1,stageHeight-16);
    let width=availableWidth,height=width/ratio;if(height>availableHeight){height=availableHeight;width=height*ratio}
    return {stageWidth,stageHeight,width,height,swapped:false};
  }
  $effect(()=>{
    const canvas=toolboxCanvas;
    if(!canvas){transformCanvasWidth=0;transformCanvasHeight=0;return}
    const update=()=>{transformCanvasWidth=canvas.clientWidth;transformCanvasHeight=canvas.clientHeight};
    update();
    const observer=new ResizeObserver(update);observer.observe(canvas);
    return()=>observer.disconnect();
  });
  function transformDisplayBox(){
    toolboxMetadataVersion;
    if(!toolboxCanvas||!media?.width||!media?.height)return null;
    const stageWidth=transformCanvasWidth||toolboxCanvas.clientWidth,stageHeight=transformCanvasHeight||toolboxCanvas.clientHeight,rotation=Number(toolValue("rotate")),swapped=rotation===90||rotation===270;
    const {width:sourceWidth,height:sourceHeight}=previewSourceDimensions();
    const ratio=swapped?sourceHeight/sourceWidth:sourceWidth/sourceHeight;
    // Keep a small interaction gutter so crop borders and resize handles remain
    // fully visible even when the source aspect ratio fills one stage axis.
    const availableWidth=Math.max(1,stageWidth-16),availableHeight=Math.max(1,stageHeight-16);
    let width=availableWidth,height=width/ratio;
    if(height>availableHeight){height=availableHeight;width=height*ratio}
    return {stageWidth,stageHeight,width,height,swapped};
  }
  function transformBoxStyle(){
    const box=transformDisplayBox();if(!box)return "inset:0";
    return `left:${(box.stageWidth-box.width)/2}px;top:${(box.stageHeight-box.height)/2}px;width:${box.width}px;height:${box.height}px`;
  }
  function mediaBoxStyle(){
    const box=mediaDisplayBox();if(!box)return "inset:0";
    return `left:${(box.stageWidth-box.width)/2}px;top:${(box.stageHeight-box.height)/2}px;width:${box.width}px;height:${box.height}px`;
  }
  function transformPreviewStyle(){
    if(!["transform","clipper"].includes(selected?.id??""))return "";
    const box=transformDisplayBox();if(!box)return "";
    const rotation=Number(toolValue("rotate"));
    if(media?.kind==="image"&&rotation===0&&toolValue("fit_mode")==="contain"&&!['off','free'].includes(toolValue("crop_mode"))){
      const [rw,rh]=toolValue("crop_mode").split(":").map(Number),ratio=rw/rh;
      const availableWidth=Math.max(1,box.stageWidth-16),availableHeight=Math.max(1,box.stageHeight-16);
      let width=availableWidth,height=width/ratio;if(height>availableHeight){height=availableHeight;width=height*ratio}
      const background=toolValue("canvas_background"),color=background==="black"?"#000":background==="white"?"#fff":background==="custom"?toolValue("canvas_color"):"repeating-conic-gradient(#25252a 0 25%,#161619 0 50%) 0/12px 12px";
      const flipX=toolValue("flip_h")==="true"?-1:1,flipY=toolValue("flip_v")==="true"?-1:1;
      return `position:absolute;left:50%;top:50%;width:${width}px;height:${height}px;object-fit:contain;background:${color};transform:translate(-50%,-50%) scale(${flipX},${flipY})`;
    }
    const width=box.swapped?box.height:box.width,height=box.swapped?box.width:box.height;
    const flipX=toolValue("flip_h")==="true"?-1:1,flipY=toolValue("flip_v")==="true"?-1:1;
    return `position:absolute;left:50%;top:50%;width:${width}px;height:${height}px;transform:translate(-50%,-50%) scale(${flipX},${flipY}) rotate(${rotation}deg)`;
  }
  const socialOutputSizes:Record<string,[number,number]>={"9:16":[1080,1920],"16:9":[1920,1080],"1:1":[1080,1080],"4:5":[1080,1350]};
  function setVerticalLayout(layout:"original"|"blur"|"fill"|"split"|"squares"|"freecam"){
    flushEditorSnapshot();
    cameraDetectionMessage="";
    setToolValue("vertical_layout",layout);
    setCropPreset("9:16",true);
    if(layout!=="fill"){
      setToolNumber("crop_x",0);setToolNumber("crop_y",0);setToolNumber("crop_w",100);setToolNumber("crop_h",100);
      if(toolValue("canvas_background")==="transparent")setToolValue("canvas_background","black");
    }
    if(["split","squares","freecam"].includes(layout))centerContentRegion();
    flushEditorSnapshot();
  }
  function contentTargetDimensions(){
    const layout=toolValue("vertical_layout");
    if(layout==="squares")return {width:1080,height:960};
    if(layout==="split"){
      const top=Math.round(1920*toolNumber("region_a_height")/100),contentOnTop=toolValue("region_order")==="b_first";
      return {width:1080,height:contentOnTop?top:1920-top};
    }
    return {width:1080,height:1920};
  }
  function centerContentRegion(){
    if(selected?.id!=="clipper")return;
    const source=previewSourceDimensions(),target=contentTargetDimensions(),sourceRatio=(source.width||16)/(source.height||9),targetRatio=target.width/target.height;
    let width=100,height=100;
    if(sourceRatio>targetRatio)width=targetRatio/sourceRatio*100;else height=sourceRatio/targetRatio*100;
    setToolNumber("region_b_x",(100-width)/2);setToolNumber("region_b_y",(100-height)/2);setToolNumber("region_b_w",width);setToolNumber("region_b_h",height);
  }
  async function autoDetectCamera(){
    if(cameraDetecting||selected?.id!=="clipper"||!media?.duration)return;
    const source=previewSourceDimensions();
    if(!source.width||!source.height)return;
    const mediaPath=media.path,layout=toolValue("vertical_layout");
    if(!["split","squares","freecam"].includes(layout))return;
    cameraDetecting=true;cameraDetectionMessage=language==="tr"?"Video örnekleniyor…":"Sampling video…";error="";
    try{
      const result=await invoke<CameraDetectionResult>("detect_camera_region",{input:mediaPath,duration:media.duration,sourceWidth:Math.round(source.width),sourceHeight:Math.round(source.height)});
      if(media?.path!==mediaPath||selected?.id!=="clipper")return;
      setToolNumber("region_a_x",result.x);setToolNumber("region_a_y",result.y);
      setToolNumber("region_a_w",result.width);setToolNumber("region_a_h",result.height);
      const confidence=result.confidence>=.78?(language==="tr"?"yüksek":"high"):result.confidence>=.55?(language==="tr"?"orta":"medium"):(language==="tr"?"düşük":"low");
      cameraDetectionMessage=language==="tr"?`Kamera uygulandı · ${confidence} güven · ${result.matched_samples}/${result.samples} kare`:`Camera applied · ${confidence} confidence · ${result.matched_samples}/${result.samples} frames`;
    }catch(reason){
      cameraDetectionMessage=language==="tr"?"Kamera bulunamadı; manuel seçim korunuyor.":"No camera found; manual selection is unchanged.";
      showToast(reason);
    }finally{cameraDetecting=false}
  }
  function setSplitOrder(value:string){setToolValue("region_order",value);centerContentRegion()}
  function setSplitHeight(value:number){setToolNumber("region_a_height",value);centerContentRegion()}
  function setCropPreset(mode:string,applyOutputDefault=true){
    setToolValue("crop_mode",mode);
    if(mode==="off"){setToolNumber("crop_x",0);setToolNumber("crop_y",0);setToolNumber("crop_w",100);setToolNumber("crop_h",100);return}
    if(mode==="free"){
      setToolNumber("crop_x",0);setToolNumber("crop_y",0);setToolNumber("crop_w",100);setToolNumber("crop_h",100);
      return;
    }
    const [rw,rh]=mode.split(":").map(Number),rotation=Number(toolValue("rotate"));
    const source=previewSourceDimensions(),sourceWidth=source.width||16,sourceHeight=source.height||9;
    const sourceRatio=rotation===90||rotation===270?sourceHeight/sourceWidth:sourceWidth/sourceHeight,target=rw/rh;
    let width=100,height=100;
    if(sourceRatio>target)width=target/sourceRatio*100;else height=sourceRatio/target*100;
    setToolNumber("crop_x",(100-width)/2);setToolNumber("crop_y",(100-height)/2);setToolNumber("crop_w",width);setToolNumber("crop_h",height);
    const outputSize=socialOutputSizes[mode];
    if(applyOutputDefault&&outputSize){
      setToolValue("size_mode","exact");
      setToolNumber("output_width",outputSize[0]);
      setToolNumber("output_height",outputSize[1]);
    }
  }
  function setTransformRotation(value:number){
    const mode=toolValue("crop_mode");setToolValue("rotate",String((value+360)%360));
    if(!["off","free"].includes(mode))setCropPreset(mode,false);
  }
  function rotateTransform(delta:number){setTransformRotation(Number(toolValue("rotate"))+delta)}
  function startTransformCrop(event:PointerEvent,mode:"move"|"n"|"s"|"e"|"w"|"nw"|"ne"|"sw"|"se"){
    if(!transformSourceBox||toolValue("crop_mode")==="off")return;
    event.preventDefault();event.stopPropagation();
    const bounds=transformSourceBox.getBoundingClientRect(),startX=event.clientX,startY=event.clientY;
    const initial={x:toolNumber("crop_x"),y:toolNumber("crop_y"),w:toolNumber("crop_w"),h:toolNumber("crop_h")};
    if(mode!=="move")setToolValue("crop_mode","free");
    const move=(moveEvent:PointerEvent)=>{
      const dx=(moveEvent.clientX-startX)/bounds.width*100,dy=(moveEvent.clientY-startY)/bounds.height*100,min=5;
      let {x,y,w,h}=initial;
      if(mode==="move"){x=Math.max(0,Math.min(100-w,x+dx));y=Math.max(0,Math.min(100-h,y+dy))}
      else{
        if(mode.includes("e"))w=Math.max(min,Math.min(100-x,initial.w+dx));
        if(mode.includes("s"))h=Math.max(min,Math.min(100-y,initial.h+dy));
        if(mode.includes("w")){x=Math.max(0,Math.min(initial.x+initial.w-min,initial.x+dx));w=initial.w+(initial.x-x)}
        if(mode.includes("n")){y=Math.max(0,Math.min(initial.y+initial.h-min,initial.y+dy));h=initial.h+(initial.y-y)}
      }
      setToolNumber("crop_x",x);setToolNumber("crop_y",y);setToolNumber("crop_w",w);setToolNumber("crop_h",h);
    };
    trackEditorPointer(event,move);
  }
  function startTransformRegion(event:PointerEvent,region:"a"|"b",mode:"move"|"n"|"s"|"e"|"w"|"nw"|"ne"|"sw"|"se"){
    if(!transformSourceBox||!["split","squares","freecam"].includes(toolValue("vertical_layout")))return;
    event.preventDefault();event.stopPropagation();
    const bounds=transformSourceBox.getBoundingClientRect(),startX=event.clientX,startY=event.clientY,prefix=`region_${region}_`;
    const initial={x:toolNumber(`${prefix}x`),y:toolNumber(`${prefix}y`),w:toolNumber(`${prefix}w`),h:toolNumber(`${prefix}h`)};
    const move=(moveEvent:PointerEvent)=>{
      const dx=(moveEvent.clientX-startX)/bounds.width*100,dy=(moveEvent.clientY-startY)/bounds.height*100,min=5;
      let {x,y,w,h}=initial;
      if(mode==="move"){x=Math.max(0,Math.min(100-w,x+dx));y=Math.max(0,Math.min(100-h,y+dy))}
      else{
        if(mode.includes("e"))w=Math.max(min,Math.min(100-x,initial.w+dx));
        if(mode.includes("s"))h=Math.max(min,Math.min(100-y,initial.h+dy));
        if(mode.includes("w")){x=Math.max(0,Math.min(initial.x+initial.w-min,initial.x+dx));w=initial.w+(initial.x-x)}
        if(mode.includes("n")){y=Math.max(0,Math.min(initial.y+initial.h-min,initial.y+dy));h=initial.h+(initial.y-y)}
      }
      setToolNumber(`${prefix}x`,x);setToolNumber(`${prefix}y`,y);setToolNumber(`${prefix}w`,w);setToolNumber(`${prefix}h`,h);
    };
    trackEditorPointer(event,move);
  }
  function startFillPan(event:PointerEvent){
    const box=verticalOutputBox();if(!box||toolValue("vertical_layout")!=="fill")return;
    event.preventDefault();event.stopPropagation();
    clipperGuidesActive=true;
    if(toolNumber("clipper_zoom")>100){
      const startX=event.clientX,startY=event.clientY,initialX=toolNumber("clipper_x"),initialY=toolNumber("clipper_y"),travel=toolNumber("clipper_zoom")/100-1;
      trackEditorPointer(event,(moveEvent)=>{
        setToolNumber("clipper_x",snapClipperCenter(initialX-(moveEvent.clientX-startX)/box.width/travel*100));
        setToolNumber("clipper_y",snapClipperCenter(initialY-(moveEvent.clientY-startY)/box.height/travel*100));
      },()=>clipperGuidesActive=false);
      return;
    }
    const startX=event.clientX,startY=event.clientY,initialX=toolNumber("crop_x"),initialY=toolNumber("crop_y"),width=toolNumber("crop_w"),height=toolNumber("crop_h");
    const move=(moveEvent:PointerEvent)=>{
      const spanX=100-width,spanY=100-height;
      const rawX=Math.max(0,Math.min(spanX,initialX-(moveEvent.clientX-startX)/box.width*width));
      const rawY=Math.max(0,Math.min(spanY,initialY-(moveEvent.clientY-startY)/box.height*height));
      const x=spanX*snapClipperCenter(spanX?rawX/spanX*100:50)/100;
      const y=spanY*snapClipperCenter(spanY?rawY/spanY*100:50)/100;
      setToolNumber("crop_x",x);setToolNumber("crop_y",y);
    };
    trackEditorPointer(event,move,()=>clipperGuidesActive=false);
  }
  function snapClipperCenter(value:number){
    const clamped=Math.max(0,Math.min(100,value));
    return Math.abs(clamped-50)<=2.5?50:clamped;
  }
  function clipperGuideAxes(){
    if(toolValue("vertical_layout")==="fill"&&toolNumber("clipper_zoom")===100){
      const spanX=100-toolNumber("crop_w"),spanY=100-toolNumber("crop_h");
      return {x:Math.abs((spanX?toolNumber("crop_x")/spanX*100:50)-50)<.1,y:Math.abs((spanY?toolNumber("crop_y")/spanY*100:50)-50)<.1};
    }
    return {x:Math.abs(toolNumber("clipper_x")-50)<.1,y:Math.abs(toolNumber("clipper_y")-50)<.1};
  }
  function wheelClipperZoom(event:WheelEvent){
    if(selected?.id!=="clipper"||!["original","blur","fill"].includes(toolValue("vertical_layout")))return;
    event.preventDefault();event.stopPropagation();
    if(!event.deltaY)return;
    setToolNumber("clipper_zoom",Math.max(100,Math.min(300,toolNumber("clipper_zoom")+(event.deltaY<0?10:-10))));
  }
  function startBlurPan(event:PointerEvent){
    const frame=blurForegroundGeometry();if(!frame||!["original","blur"].includes(toolValue("vertical_layout")))return;
    event.preventDefault();event.stopPropagation();
    clipperGuidesActive=true;
    const startX=event.clientX,startY=event.clientY,initialX=toolNumber("clipper_x"),initialY=toolNumber("clipper_y");
    trackEditorPointer(event,(moveEvent)=>{
      const horizontal=frame.scaledWidth-frame.box.width,vertical=frame.scaledHeight-frame.height;
      if(horizontal>0)setToolNumber("clipper_x",snapClipperCenter(initialX-(moveEvent.clientX-startX)/horizontal*100));
      if(vertical>0)setToolNumber("clipper_y",snapClipperCenter(initialY-(moveEvent.clientY-startY)/vertical*100));
    },()=>clipperGuidesActive=false);
  }
  function clipperLayoutInput():ClipperLayoutInput {
    const source=previewSourceDimensions();
    const region=(id:string)=>({x:toolNumber(`region_${id}_x`),y:toolNumber(`region_${id}_y`),width:toolNumber(`region_${id}_w`),height:toolNumber(`region_${id}_h`)});
    return {width:toolNumber("output_width")||1080,height:toolNumber("output_height")||1920,sourceWidth:source.width,sourceHeight:source.height,layout:toolValue("vertical_layout"),regionAHeight:toolNumber("region_a_height"),regionOrder:toolValue("region_order"),camera:region("a"),content:region("b"),freecamSize:toolNumber("freecam_size"),freecamX:toolNumber("freecam_x"),freecamY:toolNumber("freecam_y")};
  }
  function freecamPlacement(){
    const input=clipperLayoutInput(),{camera}=clipperLayoutGeometry(input);
    return {width:camera.width/input.width*100,height:camera.height/input.height*100,left:camera.x/input.width*100,top:camera.y/input.height*100};
  }
  async function playRenderedOutput(){
    if(!output||operationBusy)return;
    if(outputStale)showToast(language==="tr"?"Son render oynatılıyor; yeni ayarlar henüz işlenmedi.":"Playing the last render; the new settings have not been rendered yet.","info");
    try{await openPath(output)}catch(error){reportProblem(error)}
  }
  function clipperWatermarkPreviewStyle(background=false){
    const box=verticalOutputBox();if(!box)return "display:none";
    const layout=toolValue("vertical_layout"),outputWidth=Math.max(2,toolNumber("output_width")||1080),fontSize=Math.max(6,toolNumber("watermark_size")*box.width/outputWidth);
    const barHeight=fontSize*1.5,safeTop=box.top+box.height*.08+barHeight/2,safeBottom=box.top+box.height*.78-barHeight/2;
    let x=box.left+box.width/2,y=safeBottom;
    if(layout==="split"){
      y=box.top+box.height*toolNumber("region_a_height")/100;
    }
    else if(layout==="squares")y=box.top+box.height/2;
    else if(layout==="freecam"){
      const camera=freecamPlacement();x=box.left+box.width*(camera.left+camera.width/2)/100;y=box.top+box.height*(camera.top+camera.height)/100;
    }
    const camera=layout==="freecam"?freecamPlacement():null,backgroundEnabled=toolValue("watermark_background")==="true"&&["split","squares","freecam"].includes(layout);
    const backgroundX=camera?box.left+box.width*(camera.left+camera.width/2)/100:box.left+box.width/2;
    const backgroundWidth=camera?box.width*camera.width/100:box.width;
    x=Math.max(box.left+box.width*.10,Math.min(box.left+box.width*.90,x));
    y=Math.max(safeTop,Math.min(safeBottom,y));
    if(background)return backgroundEnabled?`display:block;left:${backgroundX}px;top:${y}px;width:${backgroundWidth}px;height:${barHeight}px`:`display:none`;
    const opacity=Math.max(.1,Math.min(1,toolNumber("watermark_opacity")/100));
    return `left:${x}px;top:${y}px;font-size:${fontSize}px;color:rgba(255,255,255,${opacity});text-shadow:0 1px 2px rgba(0,0,0,${opacity}),0 0 3px rgba(0,0,0,${opacity})`;
  }
  function socialTagTextUnits(username:string){
    const context=document.createElement("canvas").getContext("2d");
    if(!context||!username)return undefined;
    context.font='900 100px "Arial Black", Arial, sans-serif';
    const units=context.measureText(username).width/100;
    return Number.isFinite(units)&&units>0?units:undefined;
  }
  function socialBannerTextUnits(username:string){
    void bannerFontReady;
    const context=document.createElement("canvas").getContext("2d");
    if(!context||!username)return undefined;
    context.font='100px "Gotham XNarrow Black"';
    const units=context.measureText(username.toUpperCase()).width/100;
    return Number.isFinite(units)&&units>0?units:undefined;
  }
  function socialBannerPreviewStyle(){
    const box=verticalOutputBox();if(!box)return "display:none";
    const width=Math.max(2,toolNumber("output_width")||1080),height=Math.max(2,toolNumber("output_height")||1920);
    const geometry=socialBannerGeometry({width,height,sourceWidth:media?.width??1920,sourceHeight:media?.height??1080,layout:toolValue("vertical_layout"),seamOffset:toolNumber("social_tag_seam_offset"),regionAHeight:toolNumber("region_a_height"),regionOrder:toolValue("region_order"),regionAWidth:toolNumber("region_a_w"),regionARegionHeight:toolNumber("region_a_h"),freecamSize:toolNumber("freecam_size"),freecamX:toolNumber("freecam_x"),freecamY:toolNumber("freecam_y"),username:toolValue("social_tag_username").trim(),size:toolNumber("social_tag_size"),textUnits:socialBannerTextUnits(toolValue("social_tag_username").trim())});
    const left=box.left+geometry.x*box.width/width,top=box.top+geometry.y*box.height/height,previewWidth=geometry.width*box.width/width;
    const scale=previewWidth/geometry.width;
    return `left:${left}px;top:${top}px;width:${previewWidth}px;height:${geometry.height*scale}px;--banner-bar-height:${geometry.barHeight*scale}px;--banner-logo-left:${(geometry.logoX-geometry.x)*scale}px;--banner-logo-width:${geometry.logoWidth*scale}px;--banner-logo-art-width:${geometry.logoArtWidth*scale}px;--banner-logo-image-bottom:${geometry.logoImageBottom*scale}px;--banner-prefix-left:${(geometry.prefixX-geometry.x)*scale}px;--banner-prefix-width:${geometry.prefixWidth*scale}px;--banner-prefix-art-width:${geometry.prefixArtWidth*scale}px;--banner-prefix-image-left:${geometry.prefixImageLeft*scale}px;--banner-prefix-image-bottom:${geometry.prefixImageBottom*scale}px;--banner-text-left:${(geometry.textX-geometry.x)*scale}px;--banner-text-top:${(geometry.textCenterY-geometry.y)*scale}px;--banner-text-size:${geometry.textSize*scale}px`;
  }
  function socialTagPreviewStyle(){
    const box=verticalOutputBox();if(!box)return "display:none";
    const width=Math.max(2,toolNumber("output_width")||1080),height=Math.max(2,toolNumber("output_height")||1920);
    const boxed=toolValue("social_tag_style")==="boxed";
    const position=(boxed?"center":toolValue("social_tag_plain_position")) as "left"|"center"|"right";
    const username=toolValue("social_tag_username").trim();
    const tagInput={width,height,sourceWidth:media?.width??1920,sourceHeight:media?.height??1080,layout:toolValue("vertical_layout"),style:boxed?"boxed" as const:"plain" as const,position,username,size:toolNumber("social_tag_size"),textUnits:socialTagTextUnits(username),regionAHeight:toolNumber("region_a_height"),regionOrder:toolValue("region_order"),regionAWidth:toolNumber("region_a_w"),regionARegionHeight:toolNumber("region_a_h"),freecamSize:toolNumber("freecam_size"),freecamX:toolNumber("freecam_x"),freecamY:toolNumber("freecam_y")};
    const geometry=socialTagGeometry({...tagInput,seamOffset:toolNumber("social_tag_seam_offset")});
    const scaleX=box.width/width,scaleY=box.height/height;
    const left=box.left+geometry.anchorX*scaleX;
    const top=box.top+geometry.centerY*scaleY;
    const boundsLeft=box.left,boundsRight=box.left+box.width;
    const shift=position==="left"?"0":position==="right"?"-100%":"-50%";
    const available=position==="left"?boundsRight-left:position==="right"?left-boundsLeft:2*Math.min(left-boundsLeft,boundsRight-left);
    // Rasterize the small overlay at 2× and downscale it in the compositor so
    // the preview glyphs stay legible without changing their on-screen bounds.
    const rasterScale=2;
    return `left:${left}px;top:${top}px;font-size:${geometry.fontSize*scaleX*rasterScale}px;max-width:${Math.max(0,available)*rasterScale}px;transform-origin:0 0;transform:scale(${1/rasterScale}) translate(${shift},-50%)`;
  }
  function startFreecamPlacement(event:PointerEvent,mode:"move"|"resize"){
    if(!freecamLayoutBox)return;
    event.preventDefault();event.stopPropagation();
    const bounds=freecamLayoutBox.getBoundingClientRect(),startX=event.clientX,startY=event.clientY,initial=freecamPlacement();
    const move=(moveEvent:PointerEvent)=>{
      if(mode==="resize"){
        const width=Math.max(15,Math.min(90,initial.width+(moveEvent.clientX-startX)/bounds.width*100));
        setToolNumber("freecam_size",width);
        return;
      }
      const left=Math.max(0,Math.min(100-initial.width,initial.left+(moveEvent.clientX-startX)/bounds.width*100));
      const top=Math.max(0,Math.min(100-initial.height,initial.top+(moveEvent.clientY-startY)/bounds.height*100));
      setToolNumber("freecam_x",100-initial.width>0?left/(100-initial.width)*100:0);
      setToolNumber("freecam_y",100-initial.height>0?top/(100-initial.height)*100:0);
    };
    trackEditorPointer(event,move);
  }
  function startEffectRegion(event:PointerEvent,mode:"move"|"n"|"s"|"e"|"w"|"nw"|"ne"|"sw"|"se"){
    if(!transformSourceBox||selected?.id!=="blur_pixelate")return;
    event.preventDefault();event.stopPropagation();
    const bounds=transformSourceBox.getBoundingClientRect(),startX=event.clientX,startY=event.clientY;
    const initial={x:toolNumber("region_x"),y:toolNumber("region_y"),w:toolNumber("region_w"),h:toolNumber("region_h")};
    const move=(moveEvent:PointerEvent)=>{
      const dx=(moveEvent.clientX-startX)/bounds.width*100,dy=(moveEvent.clientY-startY)/bounds.height*100,min=3;
      let {x,y,w,h}=initial;
      if(mode==="move"){x=Math.max(0,Math.min(100-w,x+dx));y=Math.max(0,Math.min(100-h,y+dy))}
      else{
        if(mode.includes("e"))w=Math.max(min,Math.min(100-x,initial.w+dx));
        if(mode.includes("s"))h=Math.max(min,Math.min(100-y,initial.h+dy));
        if(mode.includes("w")){x=Math.max(0,Math.min(initial.x+initial.w-min,initial.x+dx));w=initial.w+(initial.x-x)}
        if(mode.includes("n")){y=Math.max(0,Math.min(initial.y+initial.h-min,initial.y+dy));h=initial.h+(initial.y-y)}
      }
      setToolNumber("region_x",x);setToolNumber("region_y",y);setToolNumber("region_w",w);setToolNumber("region_h",h);
    };
    trackEditorPointer(event,move);
  }
  function overlayPreviewStyle(){
    const box=mediaDisplayBox();if(!box||selected?.id!=="image_overlay")return "display:none";
    const width=box.width*toolNumber("size")/100;
    const aspect=(overlayPreviewImage?.naturalWidth||1)/(overlayPreviewImage?.naturalHeight||1),height=width/aspect;
    const left=(box.stageWidth-box.width)/2,top=(box.stageHeight-box.height)/2,margin=toolNumber("margin")/100;
    const position=toolValue("position");
    let x=left+box.width*toolNumber("x")/100-width/2,y=top+box.height*toolNumber("y")/100-height/2;
    if(position==="top_left"){x=left+box.width*margin;y=top+box.height*margin}
    if(position==="top_right"){x=left+box.width-width-box.width*margin;y=top+box.height*margin}
    if(position==="bottom_left"){x=left+box.width*margin;y=top+box.height-height-box.height*margin}
    if(position==="bottom_right"){x=left+box.width-width-box.width*margin;y=top+box.height-height-box.height*margin}
    if(position==="center"){x=left+(box.width-width)/2;y=top+(box.height-height)/2}
    return `position:absolute;z-index:4;left:${x}px;top:${y}px;width:${width}px`;
  }
  function startOverlayDrag(event:PointerEvent,resizeDirection:-1|0|1=0,verticalDirection:-1|0|1=0){
    if(!toolboxCanvas||selected?.id!=="image_overlay")return;event.preventDefault();event.stopPropagation();
    const box=mediaDisplayBox();if(!box)return;const startX=event.clientX,startY=event.clientY,originSize=toolNumber("size");
    const rect=(event.currentTarget as HTMLElement).closest(".overlay-preview-box")?.getBoundingClientRect();
    if(!rect)return;
    const stage=toolboxCanvas.getBoundingClientRect(),sourceLeft=stage.left+(box.stageWidth-box.width)/2,sourceTop=stage.top+(box.stageHeight-box.height)/2;
    const originX=(rect.left+rect.width/2-sourceLeft)/box.width*100,originY=(rect.top+rect.height/2-sourceTop)/box.height*100;
    const aspect=(overlayPreviewImage?.naturalWidth||1)/(overlayPreviewImage?.naturalHeight||1);
    setToolNumber("x",originX);setToolNumber("y",originY);setToolValue("position","custom");
    const move=(moveEvent:PointerEvent)=>{
      if(resizeDirection){
        const proposedWidth=rect.width+(moveEvent.clientX-startX)*resizeDirection+(moveEvent.clientY-startY)*verticalDirection*.5;
        const size=Math.max(1,Math.min(100,originSize*proposedWidth/Math.max(1,rect.width)));
        const newWidth=box.width*size/100;
        const center=resizeDirection>0?rect.left+newWidth/2:rect.right-newWidth/2;
        setToolNumber("size",size);
        setToolNumber("x",Math.max(0,Math.min(100,(center-sourceLeft)/box.width*100)));
        return;
      }
      const size=toolNumber("size"),halfW=size/2,halfH=(box.width*size/100/aspect)/box.height*50;
      setToolNumber("x",Math.max(halfW,Math.min(100-halfW,originX+(moveEvent.clientX-startX)/box.width*100)));
      setToolNumber("y",Math.max(halfH,Math.min(100-halfH,originY+(moveEvent.clientY-startY)/box.height*100)));
    };
    trackEditorPointer(event,move);
  }
  function timelineBounds(){
    if(!selected)return {start:0,end:0};
    if(selected.id==="screenshot"){const at=toolNumber("timestamp");return {start:at,end:at}}
    return {start:toolNumber("start"),end:toolNumber("end")};
  }
  async function loadToolboxFilmstrip(){
    if(!media||media.kind!=="video"||toolboxFilmstripLoading||toolboxFilmstripUrl)return;
    const path=media.path,id=++filmstripLoadId;toolboxFilmstripLoading=true;
    try{const result=await invoke<string>("compute_video_filmstrip",{path});if(id===filmstripLoadId&&media?.path===path)toolboxFilmstripUrl=result}catch(reason){if(id===filmstripLoadId&&media?.path===path){error=String(reason);reportProblem(reason)}}finally{if(id===filmstripLoadId)toolboxFilmstripLoading=false}
  }
  function timelineAt(clientX:number){if(!toolboxTimeline||!media?.duration)return 0;const rect=toolboxTimeline.getBoundingClientRect();return Math.max(0,Math.min(media.duration,(clientX-rect.left)/rect.width*media.duration))}
  function applyTimelineClick(at:number){
    seekToolbox(at);
    if(selected?.id==="screenshot"){setToolNumber("timestamp",at);return}
    // Range tools share Cut Video's behavior: clicking the filmstrip only
    // seeks. IN/OUT or the green handles are the only ways to change a range.
  }
  function seekTimeline(event:MouseEvent){
    if(performance.now()<timelineIgnoreClickUntil||(event.target as HTMLElement).closest(".timeline-selection,.timeline-point"))return;
    applyTimelineClick(timelineAt(event.clientX));
  }
  function hoverTimeline(event:PointerEvent){if(!toolboxTimeline)return;const rect=toolboxTimeline.getBoundingClientRect();timelineHover=Math.max(0,Math.min(100,(event.clientX-rect.left)/rect.width*100))}
  function startToolTimelineDrag(event:PointerEvent,mode:"start"|"end"|"point"|"range"){
    event.preventDefault();event.stopPropagation();if(!selected||!media?.duration)return;
    const initial=timelineBounds(),pointerStart=timelineAt(event.clientX),span=initial.end-initial.start;
    const originX=event.clientX,originY=event.clientY;
    let dragged=mode!=="range";
    const update=(moveEvent:PointerEvent)=>{
      const raw=timelineAt(moveEvent.clientX),duration=media?.duration??0;
      const anchor=mode==="end"?initial.end:initial.start;
      const at=moveEvent.shiftKey?Math.max(0,Math.min(duration,anchor+(raw-pointerStart)*.1)):raw;
      if(mode==="point"){setToolNumber("timestamp",at);seekToolbox(at);return}
      let start=initial.start,end=initial.end;
      if(mode==="start")start=Math.min(end-.01,at);
      else if(mode==="end")end=Math.max(start+.01,at);
      else{start=Math.max(0,Math.min(duration-span,initial.start+(at-pointerStart)));end=start+span}
      setTimelineRange(start,end);
      seekToolbox(mode==="end"?end:start);
    };
    if(mode!=="range")update(event);
    const move=(moveEvent:PointerEvent)=>{
      if(mode==="range"&&!dragged){
        if(Math.hypot(moveEvent.clientX-originX,moveEvent.clientY-originY)<4)return;
        dragged=true;
      }
      update(moveEvent);
    };
    const stop=(upEvent:PointerEvent)=>{
      if(mode==="range"&&!dragged)applyTimelineClick(timelineAt(upEvent.clientX));
      timelineIgnoreClickUntil=performance.now()+250;window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",stop);window.removeEventListener("pointercancel",stop)
    };
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",stop);window.addEventListener("pointercancel",stop)
  }
  function timelineHandleKey(event:KeyboardEvent,mode:"start"|"end"|"point"){
    if(event.key!=="ArrowLeft"&&event.key!=="ArrowRight"||!media?.duration)return;
    event.preventDefault();event.stopPropagation();const delta=(event.shiftKey?.01:.1)*(event.key==="ArrowRight"?1:-1),bounds=timelineBounds();
    if(mode==="point"){const value=Math.max(0,Math.min(media.duration,bounds.start+delta));setToolNumber("timestamp",value);seekToolbox(value);return}
    const start=mode==="start"?Math.max(0,Math.min(bounds.end-.01,bounds.start+delta)):bounds.start;
    const end=mode==="end"?Math.min(media.duration,Math.max(start+.01,bounds.end+delta)):bounds.end;
    setTimelineRange(start,end);
    seekToolbox(mode==="start"?start:end)
  }

  function switchKind(kind:MediaKind){
    if(!media)return;
    const allowed=kind===media.kind||(media.kind==="video"&&kind==="audio");
    if(!allowed)return;
    activeKind=kind; search="";
    const first=kindTools(kind)[0];
    if(first)chooseTool(first);else selected=null;
  }
  function captureWorkspaceSessions(){
    if(workspaceMode==="autocut"&&autoCutWorkspace)autoCutSession=autoCutWorkspace.exportSession();
    if(workspaceMode==="batch"&&batchWorkspace)batchSession=batchWorkspace.exportSession();
  }
  async function setWorkspaceMode(mode:"toolbox"|"autocut"|"batch"){
    if(operationBusy)return;
    if(downloaderOpen){downloaderOpen=false;if(mode===workspaceMode)return}
    if(mode===workspaceMode)return;
    captureWorkspaceSessions();
    const switchId=++workspaceSwitchId,loadId=mediaLoadId;
    const session=mode==="autocut"?autoCutSession:mode==="batch"?batchSession:null;
    downloaderOpen=false;
    toolboxVideo?.pause();
    workspaceMode=mode;
    await tick();
    if(switchId!==workspaceSwitchId||loadId!==mediaLoadId||workspaceMode!==mode)return;
    if(session&&mode==="autocut")autoCutWorkspace?.restoreSession(session);
    if(session&&mode==="batch")batchWorkspace?.restoreSession(session);
  }
  function encoderQualityMode(){
    const encoder=toolValue("encoder");
    if(encoder.includes("amf"))return "CQP";
    if(encoder.includes("nvenc"))return "CQ";
    if(encoder.includes("qsv"))return "Global Quality";
    return "CRF";
  }

  function numericPresets(toolId: string, field?: Field): number[] {
    if (!field || field.type !== "number") return [];
    let values: number[] = [];
    if (field.key === "fps") {
      if (toolId === "interpolation") values = Array.from({ length: 40 }, (_, index) => (index + 1) * 60).filter(value => !media?.fps || value > media.fps);
      else values = [5,10,12,15,20,23.976,24,25,29.97,30,48,50,59.94,60,90,120,144,180,240].filter(value => toolId !== "frame_blend" || !media?.fps || value < media.fps);
    } else if (field.key === "crf") values = [0,14,16,18,20,22,24,26,28,30];
    else if (field.key === "quality" && toolId === "image_potatoify") values = [1,2,3,4,5,6,7,8,9,10];
    else if ((toolId === "resize" && field.key === "size") || field.key === "height") values = [360,480,720,1080,1440,2160,4320];
    else if (field.key === "mbps") values = [0.5,1,2,3,5,8,10,15,20,35,50,80,120];
    else if (field.key === "target_mb" && toolId === "discord_compressor") values = [20,50,100,500];
    else if (field.key === "opacity") values = [10,25,50,65,75,85,100];
    else if (["times"].includes(field.key)) values = [1,2,3,5,10,20,50,100];
    else if (["scale","shrink"].includes(field.key)) values = [1,2,3,4,5,8,10];
    else if (["video_badness","audio_badness","level"].includes(field.key)) values = Array.from({length: Math.min(20, Math.max(1, Math.floor(field.max ?? 10)))},(_,index)=>index+1);
    return values.filter(value => value >= (field.min ?? -Infinity) && value <= (field.max ?? Infinity));
  }

  function numberFieldKey(toolId: string, field: Field) { return `${toolId}:${field.key}`; }
  function numberIsCustom(toolId: string, field: Field) {
    const values = numericPresets(toolId, field);
    return !!customNumberFields[numberFieldKey(toolId, field)] || !values.some(value => Math.abs(value - Number(field.value)) < 0.00001);
  }
  function chooseNumberPreset(toolId: string, field: Field, value: string) {
    const key = numberFieldKey(toolId, field);
    if (value === "__custom__") customNumberFields = {...customNumberFields, [key]: true};
    else {
      field.value = Number(value);
      customNumberFields = {...customNumberFields, [key]: false};
    }
  }

  async function selectMedia() {
    const paths = await open({
      multiple: true,
      filters: mediaDialogFilters,
    });
    if (typeof paths === "string") await openIncomingPath(paths);
    else if(Array.isArray(paths))await openIncomingPaths(paths);
  }

  function releaseTemporaryImagePreview(path:string){
    if(!path)return;
    if(temporaryImagePreviewPath===path)temporaryImagePreviewPath="";
    void invoke<boolean>("remove_image_preview",{path}).catch(reportProblem);
  }

  async function loadMedia(path: string,internal=false):Promise<boolean> {
    if (operationBusy&&!internal) return false;
    const loadId=++mediaLoadId;
    const dependency = ffmpegStatus ?? await refreshFfmpegStatus();
    if(loadId!==mediaLoadId)return false;
    if (!dependency.ready) {
      dependencyPanel = true;
      error = language === "tr" ? "FFmpeg ve FFprobe bulunamadı. Devam etmek için ikisini PATH içine kur." : "FFmpeg and FFprobe were not found. Install both on PATH to continue.";
      jobStatus = "ffmpeg missing";
      return false;
    }
    error = "";
    jobStatus = "probing media";
    let preparedPreview="";
    try {
      const loaded=await invoke<MediaInfo>("probe_media", { path });
      if(loadId!==mediaLoadId)return false;
      const extension=path.split(".").pop()?.toLowerCase()??"";
      if(loaded.kind==="image"&&["heic","heif"].includes(extension)){
        jobStatus=language==="tr"?"HEIC önizleme hazırlanıyor":"preparing HEIC preview";
        preparedPreview=await invoke<string>("prepare_image_preview",{path});
        if(loadId!==mediaLoadId){releaseTemporaryImagePreview(preparedPreview);return false}
      }
      const previewPath=preparedPreview||path;
      await invoke("authorize_media_preview",{path:previewPath});
      if(loadId!==mediaLoadId){if(preparedPreview)releaseTemporaryImagePreview(preparedPreview);return false}
      const nextMediaUrl=convertFileSrc(previewPath);
      const previousPreview=temporaryImagePreviewPath;
      temporaryImagePreviewPath=preparedPreview;
      media = loaded;
      processingStack=[];editingStackStepId=null;stackQuality="high";
      clipperPreviewMode = "source";
      mergeInputs=[];
      activeKind = media.kind;
      mediaUrl = nextMediaUrl;
      output = "";
      if(previousPreview&&previousPreview!==preparedPreview)releaseTemporaryImagePreview(previousPreview);
      workspaceMode = "toolbox";
      batchInitialPaths=[];
      batchQueueCount=0;
      autoCutSession = null;
      batchSession = null;
      toolboxCurrent = 0;
      toolboxPlaying = false;
      renderedImageUrl = "";
      renderedImageSize = 0;
      qualityAnalysis = null;
      imageCompare = 50;
      imageZoom = 1;
      imageBaseScale = 1;
      imagePanX = 0;
      imagePanY = 0;
      imageDragging = false;
      imageViewInitialized = false;
      filmstripLoadId++;subtitleLoadId++;toolboxFilmstripUrl="";toolboxFilmstripLoading=false;subtitleTracks=[];
      const first = kindTools(activeKind)[0];
      if (first) chooseTool(first);
      progress = 0;
      jobStatus = "ready";
      resetEditorHistory();
      if(!internal)stageHistory=startStages(currentSession(false)!,stageLabel());
      return true;
    } catch (reason) {
      if(preparedPreview)releaseTemporaryImagePreview(preparedPreview);
      if(loadId!==mediaLoadId)return false;
      error = String(reason);
      jobStatus = media?"ready":"error";
      reportProblem(reason);
      return false;
    }
  }

  function closeMedia() {
    if (operationBusy) return;
    mediaLoadId++;
    filmstripLoadId++;subtitleLoadId++;
    media = null;
    selected = null;
    mediaUrl = "";
    releaseTemporaryImagePreview(temporaryImagePreviewPath);
    output = "";
    error = "";
    progress = 0;
    jobStatus = "ready";
    toolboxCurrent = 0;
    toolboxPlaying = false;
    renderedImageUrl = "";
    renderedImageSize = 0;
    qualityAnalysis = null;
    imageCompare = 50;
    imageZoom = 1;
    imageBaseScale = 1;
    imagePanX = 0;
    imagePanY = 0;
    imageDragging = false;
    imageViewInitialized = false;
    toolboxFilmstripUrl="";toolboxFilmstripLoading=false;
    editHistory = [];
    editHistoryIndex = -1;
    autoCutSession=null;batchSession=null;discardRecovery();
    batchInitialPaths=[];batchQueueCount=0;
    stageHistory=null;
  }

  function stageLabel(){if(workspaceMode==="toolbox"&&processingStack.length)return `${language==="tr"?"İşlem listesi":"Processing Stack"} · ${processingStack.filter(step=>step.enabled).length} ${language==="tr"?"adım":"steps"}`;return workspaceMode==="autocut"?"SmartCut":workspaceMode==="batch"?"Batch":selected?.title??media?.name??"Media"}
  async function continueEditingOutput(path=output){
    if(!path||operationBusy)return;
    if(workspaceMode==="toolbox"&&outputStale){showToast(language==="tr"?"Ayarlar değişti. Devam etmeden önce yeniden işle.":"Settings changed. Render again before continuing.","info");return}
    const session=currentSession(false);if(!session)return;
    const previous=checkpointStage(stageHistory,session,stageLabel());
    stageNavigating=true;
    try{
      if(await loadMedia(path,true))stageHistory=continueStage(previous,currentSession(false)!,stageLabel());
    }finally{stageNavigating=false;persistRecovery()}
  }
  async function selectStage(id:number){
    if(operationBusy||!stageHistory||id===stageHistory.current)return;
    const target=stageHistory.entries.find(entry=>entry.id===id),session=currentSession(false);
    if(!target||!session)return;
    const next=checkpointStage(stageHistory,session,stageLabel(),id);next.current=id;
    const previousCandidate=recoveryCandidate;
    recoveryCandidate={...cloneEditorValue(target.session),stageHistory:next};
    if(!await restorePreviousSession())recoveryCandidate=previousCandidate;
  }

  function seekToolbox(value: number) {
    if (!toolboxVideo) return;
    const duration = Number.isFinite(toolboxVideo.duration) ? toolboxVideo.duration : playbackDuration;
    toolboxVideo.currentTime = Math.max(0, Math.min(duration, value));
    toolboxCurrent = toolboxVideo.currentTime;
  }
  function seekToolboxBy(seconds:number){
    if(!toolboxVideo)return;
    seekToolbox(toolboxVideo.currentTime+seconds);
  }
  function playerSeekPosition(event:PointerEvent){
    const rect=(event.currentTarget as HTMLInputElement).getBoundingClientRect(),thumbWidth=11;
    const visualPercent=Math.max(0,Math.min(100,(event.clientX-rect.left)/Math.max(1,rect.width)*100));
    const valuePercent=Math.max(0,Math.min(100,(event.clientX-rect.left-thumbWidth/2)/Math.max(1,rect.width-thumbWidth)*100));
    const time=playbackDuration*valuePercent/100;
    return {percent:visualPercent,time:event.shiftKey?Math.floor(time):time,precision:event.shiftKey};
  }
  function hoverPlayerSeek(event:PointerEvent){playerSeekHover=playerSeekPosition(event)}
  function precisionPlayerSeek(event:PointerEvent){
    if(!event.shiftKey)return;
    event.preventDefault();event.stopPropagation();
    const position=playerSeekPosition(event);seekToolbox(position.time);playerSeekHover=position;
  }
  function wheelPlayerSeek(event:WheelEvent){
    event.preventDefault();
    seekToolboxBy((event.deltaY>0?1:-1)*(event.ctrlKey?5:1));
  }

  function handleToolboxMetadata(){
    toolboxMetadataVersion++;

    if(selected?.id==="upscale"&&selected)configureUpscale(selected);
    const duration=toolboxVideo?.duration;
    if(!media||!duration||!Number.isFinite(duration)||duration<=0)return;
    if(!media.duration||Math.abs(media.duration-duration)>.05)media={...media,duration};
    if(selected)for(const field of selected.fields)if(["start","end","duration","timestamp"].includes(field.key))field.max=duration;
  }

  function toggleToolboxPlayer() {
    if (!toolboxVideo) return;
    if (toolboxVideo.paused) toolboxVideo.play().catch(() => {});
    else toolboxVideo.pause();
  }

  async function fullscreenToolboxPlayer() {
    if (!toolboxStage) return;
    if (document.fullscreenElement) await document.exitFullscreen();
    else await toolboxStage.requestFullscreen();
  }

  function startImageCompare(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const handle = event.currentTarget as HTMLElement;
    const comparison = handle.closest<HTMLElement>(".image-compare");
    if (!comparison) return;
    const rectangle = comparison.getBoundingClientRect();
    const pointerId = event.pointerId;
    const update = (clientX: number) => {
      imageCompare = Math.max(0, Math.min(100, (clientX - rectangle.left) / rectangle.width * 100));
    };
    update(event.clientX);
    const move = (moveEvent: PointerEvent) => {
      if (moveEvent.pointerId === pointerId) update(moveEvent.clientX);
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  }

  function imageCompareKey(event: KeyboardEvent) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    imageCompare = Math.max(0, Math.min(100, imageCompare + (event.key === "ArrowRight" ? 2 : -2)));
  }

  function resetImageView() {
    imageZoom = 1;
    imagePanX = 0;
    imagePanY = 0;
    const rectangle = imageViewport?.getBoundingClientRect();
    if (!rectangle || !media?.width || !media.height) {
      imageBaseScale = 1;
      return;
    }
    const fittedPixelScale = Math.min(rectangle.width / media.width, rectangle.height / media.height);
    imageBaseScale = fittedPixelScale > 1 ? 1 / fittedPixelScale : 1;
  }

  function initializeImageView() {
    if (imageViewInitialized) return;
    imageViewInitialized = true;
    resetImageView();
  }

  function imageTransform() {
    return `translate3d(${imagePanX}px, ${imagePanY}px, 0) scale(${imageBaseScale * imageZoom})`;
  }

  function setImageZoom(nextZoom: number, clientX?: number, clientY?: number) {
    const next = Math.max(0.1, Math.min(12, nextZoom));
    if (Math.abs(next - imageZoom) < 0.0001) return;
    const rectangle = imageViewport?.getBoundingClientRect();
    if (rectangle && clientX != null && clientY != null) {
      const anchorX = clientX - rectangle.left - rectangle.width / 2;
      const anchorY = clientY - rectangle.top - rectangle.height / 2;
      const ratio = next / imageZoom;
      imagePanX = anchorX - (anchorX - imagePanX) * ratio;
      imagePanY = anchorY - (anchorY - imagePanY) * ratio;
    }
    imageZoom = next;
  }

  function zoomImage(event: WheelEvent) {
    event.preventDefault();
    const factor = Math.exp(-event.deltaY * 0.0015);
    setImageZoom(imageZoom * factor, event.clientX, event.clientY);
  }

  function startImagePan(event: PointerEvent) {
    if (event.button !== 0) return;
    const target = event.target as Element;
    if (target.closest(".compare-handle, .image-view-controls")) return;
    event.preventDefault();
    const startX = event.clientX;
    const startY = event.clientY;
    const originX = imagePanX;
    const originY = imagePanY;
    imageDragging = true;
    const move = (moveEvent: PointerEvent) => {
      imagePanX = originX + moveEvent.clientX - startX;
      imagePanY = originY + moveEvent.clientY - startY;
    };
    const stop = () => {
      imageDragging = false;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  }

  function paramsFrom(tool: Tool) {
    const params=Object.fromEntries(tool.fields.map((field) => [field.key, String(field.value)]));
    if(tool.id==="clipper"&&params.social_tag_enabled==="true"){
      const units=params.social_tag_style==="kick_banner"?socialBannerTextUnits(params.social_tag_username.trim()):socialTagTextUnits(params.social_tag_username.trim());
      if(units!==undefined)params.social_tag_text_units=units.toFixed(5);
    }
    if(tool.id==="merge_videos")params.inputs=JSON.stringify(mergeInputs);
    if(tool.id==="color")for(const key of ["brightness","contrast","saturation","gamma","hue","temperature","sharpen","blur","deband","vignette"])params[`${key}_enabled`]=String(colorOn(key));
    if(tool.id==="text")params.layers=JSON.stringify(textLayers.map(layer=>({...layer,text:wrappedText(layer)})));
    return params;
  }
  const stackToolIds=new Set(["cut","transform","clipper","color","text","image_overlay","audio_lab"]);
  function captureStackStep(id:number):StackStep{
    if(!selected)throw new Error("Select a tool first.");
    return cloneEditorValue({id,tool:selected,params:paramsFrom(selected),textLayers,colorEnabled,enabled:true,sourceWidth:previewSourceDimensions().width});
  }
  function addOrUpdateStackStep(){
    if(!media||!selected||media.kind!=="video"||!stackToolIds.has(selected.id)||operationBusy)return;
    const issue=validate(selected);if(issue){error=issue;return}
    if(editingStackStepId!==null){const previous=processingStack.find(step=>step.id===editingStackStepId);if(previous)processingStack=processingStack.map(step=>step.id===editingStackStepId?{...captureStackStep(step.id),enabled:step.enabled}:step)}
    else{const id=Math.max(0,...processingStack.map(step=>step.id))+1;processingStack=[...processingStack,captureStackStep(id)];editingStackStepId=id}
    error="";flushEditorSnapshot();persistRecovery();
  }
  async function addSmartCutStack(value:{cuts:{start:number;end:number;enabled:boolean}[];resolution:string}){
    if(!media||operationBusy||!value.cuts.some(cut=>cut.enabled))return;
    if(stackDraftDirty()){showToast(language==="tr"?"Toolbox'taki adımın ayarları değişti. Önce Seçili adımı güncelle'ye bas.":"Your Toolbox step has unapplied changes. Click Update selected step before adding cuts.","info");return;}
    const previous=processingStack.find(step=>step.tool.id==="smartcut");
    const tool:Tool={id:"smartcut",title:"SmartCut",category:"SmartCut",kind:["video"],description:"",detail:"",fields:[]};
    const step:StackStep={id:previous?.id??Math.max(0,...processingStack.map(step=>step.id))+1,tool,params:{cuts:JSON.stringify(value.cuts),resolution:"source",source_path:media.path},textLayers:[],colorEnabled:{},enabled:previous?.enabled??true,smartcutSession:cloneEditorValue(value)};
    // SmartCut timestamps refer to the original source, before any timeline-changing step.
    processingStack=[step,...processingStack.filter(item=>item.tool.id!=="smartcut")];
    await setWorkspaceMode("toolbox");flushEditorSnapshot();persistRecovery();
  }
  async function selectStackStep(step:StackStep){if(operationBusy)return;if(step.tool.id==="smartcut"){await setWorkspaceMode("autocut");if(step.smartcutSession)autoCutWorkspace?.restoreSession(cloneEditorValue(step.smartcutSession));return}editingStackStepId=step.id;selected=restoreToolSnapshot(step.tool);textLayers=cloneEditorValue(step.textLayers);colorEnabled=cloneEditorValue(step.colorEnabled);activeTextId=textLayers[0]?.id??null;void restorePreviewFonts()}
  function moveStackStep(index:number,change:number){const next=index+change;if(next<0||next>=processingStack.length||operationBusy||processingStack[index].tool.id==="smartcut"||processingStack[next].tool.id==="smartcut")return;const steps=[...processingStack];[steps[index],steps[next]]=[steps[next],steps[index]];processingStack=steps;flushEditorSnapshot();persistRecovery()}
  function removeStackStep(id:number){if(operationBusy)return;processingStack=processingStack.filter(step=>step.id!==id);if(editingStackStepId===id)editingStackStepId=null;flushEditorSnapshot();persistRecovery()}
  function stackDraftDirty(){const step=processingStack.find(item=>item.id===editingStackStepId);return !!step&&!!selected&&JSON.stringify(paramsFrom(selected))!==JSON.stringify(step.params)}
  async function runProcessingStack(){
    if(!media||media.kind!=="video"||operationBusy||!processingStack.some(step=>step.enabled))return;
    if(stackDraftDirty()){error=language==="tr"?"Önce seçili adımı güncelle.":"Update the selected step before rendering.";return}
    armCompletionSound();
    busy=true;stackPreparing=true;++renderJobId;const jobId=crypto.randomUUID();activeJobToken=jobId;error="";output="";progress=0;elapsed=0;speed="—";frame="—";jobStatus=language==="tr"?"işlem listesi işleniyor":"processing stack";
    const source=media.path,started=performance.now();
    try{
      const steps=processingStack.map(step=>({operation:step.tool.id,params:{...step.params},enabled:step.enabled}));
      let width=media.width??1920,height=media.height??1080;
      for(let index=0;index<steps.length;index++){
        const step=steps[index],saved=processingStack[index];if(!step.enabled)continue;
        if(step.operation==="text"){
          const fonts=await ensureSystemFonts();const layers=await Promise.all(saved.textLayers.map(async original=>{const layer=scaleStackText(original,saved.sourceWidth??width,width);const font=fonts.find(item=>item.path.toLowerCase()===layer.font_path.toLowerCase());return font?{...layer,fontName:await loadPreviewFont(font)}:layer}));
          step.params.layers=JSON.stringify(layers.map(layer=>({...layer,text:wrappedText(layer,width)})));
          if(layers.some(layer=>/[\p{Extended_Pictographic}\p{Regional_Indicator}\p{Emoji_Modifier}\u20e3\ufe0f]/u.test(layer.text))){const raster=document.createElement("canvas");drawTextLayers(raster,layers,width,height);step.params.text_raster_png=raster.toDataURL("image/png");}
        }
        if(step.operation==="clipper"&&step.params.watermark_enabled==="true"&&step.params.watermark_layer){
          const layer=JSON.parse(step.params.watermark_layer) as TextAppearance;
          const fonts=await ensureSystemFonts(),font=fonts.find(item=>item.path.toLowerCase()===layer.font_path.toLowerCase());
          if(!font)throw new Error("Watermark font is unavailable.");
          const raster=document.createElement("canvas");rasterText(raster,{...layer,fontName:await loadPreviewFont(font)},Number(step.params.output_width)||1080,Number(step.params.output_height)||1920);step.params.text_raster_png=raster.toDataURL("image/png");
        }
        ({width,height}=stackOutputDimensions(step.operation,step.params,{width,height}));
      }
      const settings=renderSettingsKey("stack");
      if(activeJobToken!==jobId)return;
      stackPreparing=false;
      const result=await invoke<JobResult>("run_operation",{jobId,request:{input:source,operation:"processing_stack",params:{steps:JSON.stringify(steps),quality:stackQuality}}});
      if(activeJobToken!==jobId||media?.path!==source)return;
      output=result.output;outputSettingsKey=settings;outputMode="stack";progress=100;elapsed=result.elapsed;jobStatus="complete";void completionAlert(language==="tr"?"İşlem listesi tamamlandı.":"Processing Stack complete.");await playCompletionSound();
    }catch(reason){if(activeJobToken===jobId){error=String(reason);elapsed=(performance.now()-started)/1000;jobStatus=String(reason).toLowerCase().includes("cancel")?"cancelled":"failed";reportProblem(reason)}}finally{if(activeJobToken===jobId){busy=false;stackPreparing=false}}
  }
  function hasEmojiText(){return textLayers.some(layer=>/[\p{Extended_Pictographic}\p{Regional_Indicator}\p{Emoji_Modifier}\u20e3\ufe0f]/u.test(layer.text))}

  $effect(()=>{
    const path=media?.path,id=selected?.id,mode=toolValue("mode"),format=toolValue("format"),quality=toolNumber("quality"),background=toolValue("jpeg_background"),pngMode=toolValue("png_mode");
    compressionEstimateRetry;
    const requestId=++compressionEstimateId;
    compressionEstimate=null;
    compressionEstimateError=false;
    compressionEstimateLoading=false;
    if(!path||id!=="image_compressor"||mode!=="quality"||busy)return;
    compressionEstimateLoading=true;
    const timer=window.setTimeout(async()=>{
      try{
        const size=await invoke<number>("estimate_image_compression",{request:{input:temporaryImagePreviewPath||path,operation:"image_compressor",params:{mode,format,png_mode:pngMode||"lossless",quality:String(quality),target_kb:"1",jpeg_background:background||"#ffffff",...(temporaryImagePreviewPath?{__source_path:path}:{})}}});
        if(requestId===compressionEstimateId){if(!Number.isFinite(size)||size<=0)throw new Error("Invalid image size estimate");compressionEstimate=size;}
      }catch{
        if(requestId===compressionEstimateId){compressionEstimate=null;compressionEstimateError=true;}
      }finally{
        if(requestId===compressionEstimateId)compressionEstimateLoading=false;
      }
    },400);
    return()=>window.clearTimeout(timer);
  });

  async function analyzeCompression(){
    if(!media||selected?.id!=="compression"||qualityAnalyzing||busy)return;
    const analyzedPath=media.path;
    qualityAnalyzing=true;error="";qualityAnalysis=null;jobStatus=language==="tr"?"kalite analiz ediliyor":"analyzing quality";
    activeJobToken=crypto.randomUUID();
    try{
      const result=await invoke<QualityAnalysis>("analyze_quality",{jobId:activeJobToken,request:{input:analyzedPath,goal:toolValue("goal")||"balanced",sample_duration:toolNumber("sample_duration")||2}});
      if(selected?.id==="compression"&&media?.path===analyzedPath){qualityAnalysis=result;elapsed=result.elapsed;progress=100;jobStatus=language==="tr"?"analiz tamamlandı":"analysis complete"}
    }catch(reason){error=String(reason);jobStatus="failed";reportProblem(reason)}finally{qualityAnalyzing=false}
  }
  function applyQualityRecommendation(){if(qualityAnalysis)setToolNumber("crf",qualityAnalysis.recommended_crf)}

  function discordBudget(){
    if(!media?.duration)return null;
    const targetMb=toolNumber("target_mb");
    const totalBps=targetMb*1024*1024*.96*8/media.duration;
    const audioChoice=String(toolField("audio_kbps")?.value??"auto");
    const audioKbps=!media.audio_codec?0:audioChoice==="auto"?(totalBps<250000?64:totalBps<500000?96:128):Number(audioChoice);
    const calculatedVideoKbps=Math.max(0,Math.floor(totalBps/1000-audioKbps));
    const videoKbps=media.bitrate&&media.bitrate>0?Math.min(calculatedVideoKbps,Math.floor(media.bitrate/1000)):calculatedVideoKbps;
    const sourceBpp=media.width&&media.height&&media.fps&&media.bitrate?media.bitrate/(media.width*media.height*media.fps):Infinity;
    const screenLike=!!(media.width&&media.height&&media.width>=1280&&media.height>=720&&sourceBpp<=.015);
    const autoHeight=screenLike?0:videoKbps<180?240:videoKbps<350?360:videoKbps<750?480:videoKbps<1800?720:videoKbps<3500?1080:0;
    const autoFps=screenLike?0:videoKbps<180?15:videoKbps<350?20:videoKbps<750?24:videoKbps<3500?30:0;
    return {targetMb,totalBps,audioKbps,videoKbps,autoHeight,autoFps,screenLike};
  }

  function validate(tool: Tool): string | null {
    if(tool.id==="image_compressor"&&toolValue("mode")==="target"&&!imageTargetSupported(imageOutputFormat(media?.path??"",toolValue("format"))))return language==="tr"?"Hedef boyut için WebP veya JPEG seç.":"Choose WebP or JPEG for a target size.";
    const params = paramsFrom(tool);
    if(tool.id==="clipper"&&params.watermark_enabled==="true"&&!params.watermark_text.trim())return language==="tr"?"Watermark açıkken bir yazı gir.":"Enter watermark text or turn the watermark off.";
    if(tool.id==="clipper"&&params.social_tag_enabled==="true"&&(!params.social_tag_username.trim()||Array.from(params.social_tag_username.trim()).length>32||/[\r\n]/.test(params.social_tag_username)))return language==="tr"?"Social Tag için tek satırda en fazla 32 karakterlik bir kullanıcı adı gir.":"Enter a Social Tag username of up to 32 characters on one line.";
    if (tool.id === "interpolation" && media?.fps) {
      const fps = Number(params.fps);
      if (fps <= media.fps || fps > 2400 || fps % 60 !== 0) return `Interpolation FPS ${media.fps.toFixed(2)} değerinden yüksek, 60'ın katı ve en fazla 2400 olmalı.`;
    }
    if (tool.id === "frame_blend" && media?.fps && Number(params.fps) >= media.fps) return `Frame Blending hedefi ${media.fps.toFixed(2)} FPS değerinden düşük olmalı.`;
    if (tool.id === "upscale" && media?.width && media?.height && Number(params.target_edge) <= Math.min(previewSourceDimensions().width,previewSourceDimensions().height)) return language==="tr"?"Bu kaynak için daha yüksek bir standart çözünürlük hedefi yok.":"There is no higher standard resolution target for this source.";
    if (["cut", "gif"].includes(tool.id) && Number(params.start) >= Number(params.end ?? Number(params.start) + Number(params.duration))) {
      if (tool.id === "cut") return "Bitiş zamanı başlangıçtan büyük olmalı.";
    }
    if (tool.id === "replace_audio" && !params.audio_path) return "Önce replacement audio dosyasını seç.";
    if(tool.id==="merge_videos"&&mergeInputs.length<2)return language==="tr"?"Birleştirmek için en az iki video seç.":"Choose at least two videos to merge.";
    if(tool.id==="image_overlay"&&!params.image_path)return language==="tr"?"Önce bir kaplama görseli seç.":"Choose an overlay image first.";
    if(tool.id==="image_overlay"&&media?.kind==="video"&&Number(params.end)<=Number(params.start))return language==="tr"?"Bitiş zamanı başlangıçtan sonra olmalı.":"End time must be later than start time.";
    if(tool.id==="subtitles"&&["add","burn"].includes(params.action)&&!params.subtitle_path)return language==="tr"?"Önce bir altyazı dosyası seç.":"Choose a subtitle file first.";
    if(tool.id==="subtitles"&&params.action==="extract"&&!params.subtitle_track)return language==="tr"?"Bu videoda çıkarılabilir altyazı parçası yok.":"This video has no subtitle track to extract.";
    if (tool.id === "text" && (!textLayers.length || textLayers.some(layer=>!layer.text.trim()))) return language==="tr"?"En az bir dolu yazı katmanı ekle.":"Add at least one non-empty text layer.";
    if(tool.id==="color"&&!Object.values(colorEnabled).some(Boolean)&&toolValue("denoise")==="off"&&toolValue("grayscale")!=="on"&&toolValue("deinterlace")==="off")return language==="tr"?"Önce en az bir filtreyi etkinleştir.":"Enable at least one filter.";
    if (tool.id === "discord_compressor") {
      if (!media?.duration || Number(params.target_mb) < 2) return language === "tr" ? "Discord sıkıştırması için geçerli bir süre ve en az 2 MB sınır gerekli." : "Discord compression needs a valid duration and a limit of at least 2 MB.";
      const budget=discordBudget();
      const usableKbps=(budget?.totalBps??0)/1000;
      const audioKbps=budget?.audioKbps??0;
      if(usableKbps<audioKbps+50) return language === "tr" ? `Bu süre ve boyutta ${audioKbps} kbps sesi korumak mümkün değil. Boyutu büyüt veya ses bitrate’ini düşür.` : `This duration and size cannot preserve ${audioKbps} kbps audio. Increase the size or lower the audio bitrate.`;
    }
    return null;
  }

  async function runTool() {
    if (!media || !selected || operationBusy) return;
    if(selected.id==="frame_extractor"&&toolValue("mode")==="burst"){
      setToolValue("start",String(toolboxVideo?.currentTime??toolboxCurrent));
      setToolValue("end",String(media.duration??0));
    }
    if(selected.id==="extract_audio"&&!media.audio_codec){error=language==="tr"?"Bu videoda ses yok. Ses içeren bir video açın veya Kaynak geçmişinden önceki videoya dönün.":"This video has no audio. Open a video with audio or return to the previous video in Source history.";return;}
    const validation = validate(selected);
    if (validation) { error = validation; return; }
    armCompletionSound();
    busy = true;
    ++renderJobId;
    activeJobToken=crypto.randomUUID();
    error = "";
    output = "";
    progress = 0;
    speed = "—";
    frame = "—";
    elapsed = 0;
    jobStatus = `running · ${selected.title.toLowerCase()}`;
    const started = performance.now();
    try {
      if (selected.id === "file_hash") {
        hashResult = await invoke<string>("hash_file", { path: media.path });
        elapsed = (performance.now() - started) / 1000;
        progress = 100;
        jobStatus = language === "tr" ? "SHA-256 hesaplandı" : "SHA-256 calculated";
        return;
      }
      const renderedSettingsKey=renderSettingsKey();
      const operationParams=paramsFrom(selected);
      if(selected.id==="clipper"&&toolValue("watermark_enabled")==="true"&&watermarkLayer){
        if(watermarkLoading||watermarkFontReady!==watermarkLayer.font_path)throw new Error("Watermark font is still loading. Try again when it is ready.");
        const raster=document.createElement("canvas");rasterText(raster,watermarkLayer,toolNumber("output_width")||1080,toolNumber("output_height")||1920);operationParams.text_raster_png=raster.toDataURL("image/png");
      }
      if(selected.id==="text"&&hasEmojiText()){
        renderTextPreview();
        if(!textPreviewCanvas)throw new Error(language==="tr"?"Emoji çıktısı için yazı önizlemesi hazır değil.":"Text preview is not ready for emoji export.");
        operationParams.text_raster_png=textPreviewCanvas.toDataURL("image/png");
      }
      const operationInput=temporaryImagePreviewPath||media.path;
      if(temporaryImagePreviewPath)operationParams.__source_path=media.path;
      const result = await invoke<JobResult>("run_operation", {
        jobId: activeJobToken,
        request: { input: operationInput, operation: selected.id, params: operationParams },
      });
      output = result.output;
      outputSettingsKey=renderedSettingsKey;
      outputMode="tool";
      if (media.kind === "image") {
        renderedImageUrl = `${convertFileSrc(result.output)}?render=${Date.now()}`;
        renderedImageSize = (await invoke<MediaInfo>("probe_media",{path:result.output})).size;
        imageCompare = 50;
      }
      elapsed = result.elapsed;
      progress = 100;
      jobStatus = "complete";
      void completionAlert(language==="tr"?"Render tamamlandı. Çıktı hazır.":"Render complete. Your output is ready.");
      await playCompletionSound();
    } catch (reason) {
      error = String(reason);
      elapsed = (performance.now() - started) / 1000;
      jobStatus = String(reason).toLowerCase().includes("cancel") ? "cancelled" : "failed";
      reportProblem(reason);
    } finally {
      busy = false;
    }
  }

  async function cancelJob() {
    if (!busy) return;
    const jobId=renderJobId;
    if(stackPreparing){activeJobToken="";stackPreparing=false;busy=false;jobStatus="cancelled";return}
    try {
      await invoke("cancel_job",{jobId:activeJobToken});
      if(busy&&jobId===renderJobId)jobStatus = "cancelling";
    } catch(reason) { reportProblem(reason); }
  }

  async function selectFieldFile(field: Field) {
    const path = await open({ multiple: false, filters: [{ name: field.key==="image_path"?"Image":field.key==="subtitle_path"?"Subtitle":"Audio", extensions: field.accept ?? [] }] });
    if (typeof path === "string") field.value = path;
  }

  function recommendation(): string {
    if (!selected || !media) return "";
    if (selected.id === "noise") return media.height && media.height <= 720 ? "Safe 3 · Recommended 6" : "Safe 4 · Recommended 8";
    if (selected.id === "distortion") return "Safe 1 · Recommended 3";
    if (selected.id === "discord_compressor" && media.duration) {
      const budget=discordBudget();if(!budget)return "";
      const {audioKbps,videoKbps,autoHeight,autoFps,screenLike}=budget;
      const resolution=String(toolField("resolution")?.value??"source");
      const autoNote=resolution==="auto"?(screenLike?(language==="tr"?" · Otomatik kaynak çözünürlük (ekran/yazı)":" · Auto source resolution (screen/text)"):autoHeight?` · Auto ${autoHeight}p`:""):"";
      const fpsNote=String(toolField("fps_limit")?.value)==="auto"&&autoFps?` / ${autoFps} FPS`:"";
      return language === "tr" ? `Ses: ${audioKbps} kbps · Video: ~${videoKbps} kbps${autoNote}${fpsNote} · İki geçiş` : `Audio: ${audioKbps} kbps · Video: ~${videoKbps} kbps${autoNote}${fpsNote} · Two-pass`;
    }
    return "";
  }

  function qualityRating(value:string){
    const labels:Record<string,[string,string]>={excellent:["Mükemmel — farkı görmek çok zor","Excellent — differences are very hard to see"],very_good:["Çok iyi — küçük farklar olabilir","Very good — small differences may exist"],good:["İyi — hareketli sahnelerde fark görülebilir","Good — differences may be visible in motion"],heavy_loss:["Belirgin kayıp — detaylar bozulabilir","Heavy loss — fine detail may degrade"]};
    return labels[value]?.[language==="tr"?0:1]??value;
  }

  async function refreshFfmpegStatus(showWhenMissing = false): Promise<FfmpegStatus> {
    dependencyChecking = true;
    try {
      ffmpegStatus = await invoke<FfmpegStatus>("ffmpeg_status");
    } catch {
      ffmpegStatus = { ready:false, ffmpeg_version:null, ffprobe_version:null };
    } finally {
      dependencyChecking = false;
    }
    if (ffmpegStatus?.ready && !runtimeMigrationError) dependencyPanel = false;
    else if (showWhenMissing) dependencyPanel = true;
    return ffmpegStatus;
  }

  async function repairFfmpegRuntime() {
    dependencyChecking = true;
    try {
      await invoke<boolean>("repair_ffmpeg_runtime");
      runtimeMigrationError = "";
      await refreshFfmpegStatus(true);
    } catch (reason) {
      runtimeMigrationError = String(reason);
      dependencyPanel = true;
      reportProblem(reason);
    } finally {
      dependencyChecking = false;
    }
  }

  async function checkForUpdates(manual = true) {
    if (updateChecking || updateInstalling) return;
    if (!appVersion) appVersion = await getVersion().catch(() => "");
    if (!updatesAllowedForVersion(appVersion)) return;
    if (manual) updatePanel = true;
    updateChecking = true;
    updateStatus = language === "tr" ? "Güncellemeler denetleniyor…" : "Checking for updates…";
    try {
      const result = await check({ timeout: 15000 });
      if (availableUpdate && availableUpdate !== result) await availableUpdate.close().catch(() => {});
      availableUpdate = result;
      updateStatus = result
        ? (language === "tr" ? `CONTAINER ${result.version} hazır.` : `CONTAINER ${result.version} is available.`)
        : (language === "tr" ? "CONTAINER güncel." : "CONTAINER is up to date.");
      if (result) updatePanel = true;
    } catch (reason) {
      updateStatus = language === "tr" ? "Güncelleme denetlenemedi. İnternet bağlantını kontrol et." : "Could not check for updates. Check your internet connection.";
      if (!manual) updatePanel = false;
      if(manual)reportProblem(updateStatus);
      console.warn("Update check failed", reason);
    } finally {
      updateChecking = false;
    }
  }

  async function installAvailableUpdate() {
    if (!availableUpdate || updateInstalling) return;
    updateInstalling = true;
    updateDownloaded = 0;
    updateTotal = 0;
    updateStatus = language === "tr" ? "Güncelleme indiriliyor…" : "Downloading update…";
    try {
      await availableUpdate.downloadAndInstall((event) => {
        if (event.event === "Started") updateTotal = event.data.contentLength ?? 0;
        if (event.event === "Progress") updateDownloaded += event.data.chunkLength;
        if (event.event === "Finished") updateStatus = language === "tr" ? "Güncelleme kuruluyor; CONTAINER yeniden başlayacak…" : "Installing update; CONTAINER will restart…";
      }, { timeout: 300000, restartAfterInstall: true });
    } catch (reason) {
      updateStatus = language === "tr" ? `Güncelleme kurulamadı: ${String(reason)}` : `Update could not be installed: ${String(reason)}`;
      reportProblem(updateStatus);
      updateInstalling = false;
    }
  }

  onMount(() => {
    let disposed=false;
    const stopPremiere=watchPremiere();
    void document.fonts.load('32px "Gotham XNarrow Black"').then(()=>{if(!disposed)bannerFontReady=true}).catch(()=>{});
    try{
      const savedPresets=JSON.parse(localStorage.getItem("container-text-presets-v1")??"[]");
      if(Array.isArray(savedPresets))textPresets=savedPresets.filter(p=>p&&typeof p.id==="string"&&typeof p.name==="string"&&p.layer&&typeof p.layer.text==="string"&&typeof p.layer.font_path==="string"&&["x","y","size","opacity","outline","shadow","background_opacity","background_padding"].every(key=>Number.isFinite(p.layer[key]))&&["color","outline_color","shadow_color","background_color"].every(key=>/^#[0-9a-f]{6}$/i.test(p.layer[key]))&&["left","center","right"].includes(p.layer.align));
    }catch{textPresets=[]}
    const saved=localStorage.getItem("container-language");
    language=saved==="tr"||saved==="en"?saved:navigator.language.toLowerCase().startsWith("tr")?"tr":"en";
    document.documentElement.lang=language;
    if(isTauri())void invoke("set_tray_language",{language}).catch(reportProblem);
    let unlistenTrayUpdate:UnlistenFn|undefined;
    if(isTauri())void listen("tray-check-updates",()=>{void checkForUpdates(true)}).then(fn=>{if(disposed)fn();else unlistenTrayUpdate=fn});
    let unlistenSecondOpen:UnlistenFn|undefined;
    if(isTauri())void listen<string>("container-open-path",event=>{void openIncomingPath(event.payload)}).then(fn=>{if(disposed)fn();else unlistenSecondOpen=fn});
    let unlistenTrayHidden:UnlistenFn|undefined;
    if(isTauri())void listen("container-tray-hidden",()=>{
      document.querySelectorAll<HTMLMediaElement>("video,audio").forEach(element=>element.pause());
      persistRecovery();
    }).then(fn=>{if(disposed)fn();else unlistenTrayHidden=fn});
    theme=document.documentElement.dataset.theme==="light"?"light":"dark";
    void syncWindowTheme(theme);
    try{const savedFavorites=JSON.parse(localStorage.getItem("container-favorites")??"[]");if(Array.isArray(savedFavorites))favoriteIds=savedFavorites.filter(value=>typeof value==="string")}catch{favoriteIds=[]}
    try{
      const savedPanels=JSON.parse(localStorage.getItem("container-toolbox-panel-widths")??"null");
      if(savedPanels&&typeof savedPanels==="object"){
        if(Number.isFinite(savedPanels.left)&&savedPanels.left>=185&&savedPanels.left<=560)toolboxLeftWidth=savedPanels.left;
        if(Number.isFinite(savedPanels.right)&&savedPanels.right>=225&&savedPanels.right<=620)toolboxRightWidth=savedPanels.right;
      }
    }catch{}
    void getVersion().then((version) => appVersion = version).catch(() => {});
    void (async()=>{
      const initialMediaLoadId=mediaLoadId;
      const [path,interrupted]=await Promise.all([
        invoke<string|null>("startup_media_path").catch(()=>null),
        invoke<boolean>("previous_session_interrupted").catch(()=>false),
      ]);
      // A user-selected or dropped file wins if it arrived while startup checks ran.
      if(mediaLoadId!==initialMediaLoadId||media||restoringSession)return;
      const action=startupAction(path,interrupted);
      if(action==="open-path"&&path){
        recoveryCandidate=null;
        await openIncomingPath(path);
        return;
      }
      if(action==="discard-recovery"){discardRecovery();return}
      try{const savedSession=JSON.parse(localStorage.getItem(recoveryKey)??"null");if(validRecovery(savedSession))recoveryCandidate=savedSession;else discardRecovery()}catch{discardRecovery()}
    })();
    // Run both checks on every launch. The UI stays quiet unless the user
    // needs FFmpeg or a newer signed release is available.
    void (async () => {
      runtimeMigrationError = await invoke<string | null>("ffmpeg_runtime_error").catch(() => null) ?? "";
      await refreshFfmpegStatus(true);
      ffmpegCapabilities=await invoke<FfmpegCapabilities>("ffmpeg_capabilities").catch(()=>null);
      downloaderStatus=await invoke<DownloaderStatus>("downloader_status").catch(()=>({ready:false,version:null}));
      await checkForUpdates(false);
    })();
    void (async () => {
      const configured = await invoke<boolean>("auto_encoder_configured").catch(() => true);
      autoEncoderTuning = !configured;
      try {
        availableEncoders = await invoke<string[]>("available_encoders");
        await invoke<string>("warm_up_auto_encoder");
        if(selected?.id==="encode")restrictEncoderOptions(selected);
      } catch {
        availableEncoders = null;
      } finally {
        autoEncoderTuning = false;
      }
    })();
    const playerKeys = (event: KeyboardEvent) => {
      if(event.defaultPrevented||document.querySelector("dialog[open]"))return;
      const key = event.key.toLowerCase();
      const target=event.target as HTMLElement|null;
      const editingText=isTextEditingTarget(target?.tagName,target?.isContentEditable??false);
      // CONTAINER is a desktop editor, not a browser page. Keep the WebView
      // find overlay and next/previous-find navigation out of the UI.
      if ((event.ctrlKey || event.metaKey) && !event.altKey && ["f", "g"].includes(key)) {
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      if (event.key === "F3") {
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      if (!media||restoringSession||stageNavigating) return;
      if (event.ctrlKey && !event.altKey && key === "z" && target instanceof HTMLInputElement && target.type === "range") {
        event.preventDefault();
        if (event.shiftKey) redoEditor(); else undoEditor();
        return;
      }
      if(editingText)return;
      if (event.ctrlKey && !event.altKey && event.key.toLowerCase() === "z") {
        event.preventDefault();
        if (event.shiftKey) redoEditor(); else undoEditor();
        return;
      }
      if (workspaceMode !== "toolbox") return;
      if (media.kind !== "video" || event.ctrlKey || event.altKey || event.metaKey) return;
      if (event.code === "Space") { event.preventDefault(); toggleToolboxPlayer(); }
      else if (!stackResultPreview && rangeTimelineTool() && key === "i") { event.preventDefault(); markCutAtPlayhead("start"); }
      else if (!stackResultPreview && rangeTimelineTool() && key === "o") { event.preventDefault(); markCutAtPlayhead("end"); }
      else if (event.key === "ArrowLeft") seekToolboxBy(-5);
      else if (event.key === "ArrowRight") seekToolboxBy(5);
    };
    const blockBrowserMenu = (event: MouseEvent) => event.preventDefault();
    const toastEvent=(event:Event)=>{const detail=(event as CustomEvent<ToastDetail>).detail;if(detail?.message)showToast(detail.message,detail.kind??"error")};
    const browserError=(event:ErrorEvent)=>showToast(event.error??event.message);
    const rejected=(event:PromiseRejectionEvent)=>showToast(event.reason);
    window.addEventListener("keydown", playerKeys);
    window.addEventListener("contextmenu", blockBrowserMenu);
    window.addEventListener("beforeunload", persistRecovery);
    window.addEventListener("container-toast",toastEvent);
    window.addEventListener("error",browserError);
    window.addEventListener("unhandledrejection",rejected);
    listen<ProgressEvent>("container-progress", (event) => {
      if(!busy&&!qualityAnalyzing)return;
      if(event.payload.job_id&&event.payload.job_id!==activeJobToken)return;
      progress = Math.max(0, Math.min(100, event.payload.percent));
      speed = event.payload.speed || "—";
      frame = event.payload.frame || "—";
      elapsed = event.payload.time;
      jobStatus = event.payload.status || jobStatus;
    }).then((fn) => {if(disposed)fn();else unlistenProgress=fn});

    getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") dragActive = true;
      if (event.payload.type === "leave") dragActive = false;
      if (event.payload.type === "drop") {
        dragActive = false;
        void openIncomingPaths(event.payload.paths);
      }
    }).then((fn) => {if(disposed)fn();else unlistenDrop=fn});

    return () => { disposed=true;stopPremiere();unlistenProgress?.(); unlistenDrop?.(); unlistenTrayUpdate?.(); unlistenSecondOpen?.(); unlistenTrayHidden?.(); window.clearTimeout(outputCleanupMessageTimer);window.clearTimeout(toastTimer); window.removeEventListener("keydown", playerKeys); window.removeEventListener("contextmenu", blockBrowserMenu); window.removeEventListener("beforeunload", persistRecovery);window.removeEventListener("container-toast",toastEvent);window.removeEventListener("error",browserError);window.removeEventListener("unhandledrejection",rejected); };
  });

  function setLanguage(next:"tr"|"en"){
    if(next===language)return;
    const previous=selected;
    language=next; localStorage.setItem("container-language",next); document.documentElement.lang=next;
    if(isTauri())void invoke("set_tray_language",{language:next}).catch(reportProblem);
    if(previous){selected=restoreToolSnapshot(previous);if(selected)restrictEncoderOptions(selected)}
  }
  function setTheme(next:"dark"|"light"){
    if(next===theme)return;
    const root=document.documentElement;
    root.classList.add("theme-changing");
    root.dataset.theme=next;
    root.style.colorScheme=next;
    document.querySelector<HTMLMetaElement>('meta[name="theme-color"]')?.setAttribute("content",next==="light"?"#f3f5f8":"#09090b");
    theme=next;localStorage.setItem("container-theme",next);void syncWindowTheme(next);
    requestAnimationFrame(()=>requestAnimationFrame(()=>root.classList.remove("theme-changing")));
  }
  async function syncWindowTheme(next:"dark"|"light"){
    if(!isTauri())return;
    try{
      const appWindow=getCurrentWindow();
      await appWindow.setTheme(next);
      const logo=new Image();
      logo.src=next==="light"?"/logo-light.png":"/logo-dark.png";
      await logo.decode();
      const canvas=document.createElement("canvas");
      canvas.width=64;canvas.height=64;
      const context=canvas.getContext("2d");
      if(!context)throw new Error("Window icon could not be prepared.");
      context.drawImage(logo,0,0,64,64);
      const icon=await TauriImage.new(new Uint8Array(context.getImageData(0,0,64,64).data.buffer),64,64);
      try{await appWindow.setIcon(icon)}finally{await icon.close()}
    }catch(error){reportProblem(error)}
  }
</script>

{#snippet historyControl()}
  <StageHistoryControl history={stageHistory} {language} busy={operationBusy} currentLabel={stageLabel()} onselect={selectStage}/>
{/snippet}
<main class="shell" class:drag-active={dragActive} inert={restoringSession||stageNavigating} aria-busy={restoringSession||stageNavigating}>
  <header class="topbar">
    <span class="brand"><span class="brand-logo-stack" aria-hidden="true"><img class="brand-logo brand-logo-dark" src="/mark-dark.svg" alt="" decoding="sync" draggable="false"><img class="brand-logo brand-logo-light" src="/mark-light.svg" alt="" decoding="sync" draggable="false"></span>CONTAINER</span>
    {#if !media && devVersion}<span class="dev-version mono" aria-label={`Development build version ${devVersion}`}><span>DEV BUILD</span><b>v{devVersion}</b></span>{/if}
    {#if media}
      {@render historyControl()}
      {#if workspaceMode==="batch"&&!downloaderOpen}
      <div class="file-summary"><span class="filename mono">{language==="tr"?"TOPLU İŞLEM":"BATCH QUEUE"}</span><div class="chips mono"><span><b>{language==="tr"?"dosya":"files"}</b>{batchQueueCount}</span></div></div>
      {:else}<div class="file-summary">
      <span class="filename mono" title={`${media.name}${media.kind!=="image"?`\n${language==="tr"?"Süre":"Duration"}: ${formatDuration(media.duration)}`:""}${media.width?`\n${language==="tr"?"Çözünürlük":"Resolution"}: ${media.width}×${media.height}`:""}${media.kind==="video"&&media.fps?`\nFPS: ${media.fps.toFixed(3)}`:""}\nCodec: ${media.codec}\n${language==="tr"?"Boyut":"Size"}: ${formatBytes(media.size)}`}>{media.name}</span>
      <div class="chips mono">
        {#if media.kind!=="image"}<span><b>dur</b>{formatDuration(media.duration)}</span>{/if}
        {#if media.width}<span><b>res</b>{media.width}×{media.height}</span>{/if}
        {#if media.kind==="video"&&media.fps}<span class="media-extra"><b>fps</b>{media.fps.toFixed(3)}</span>{/if}
        <span class="media-extra"><b>codec</b>{media.codec}</span><span class="media-extra"><b>size</b>{formatBytes(media.size)}</span>
      </div>
      </div>
      {/if}
      <nav class="mode-tabs" aria-label="Workspace">
        <button class:active={!downloaderOpen&&workspaceMode === "toolbox"} onclick={() => setWorkspaceMode("toolbox")} disabled={operationBusy&&workspaceMode!=="toolbox"}>{t("toolbox")}</button>
        {#if media.kind === "video"}<button class:active={!downloaderOpen&&workspaceMode === "autocut"} onclick={() => setWorkspaceMode("autocut")} disabled={operationBusy&&workspaceMode!=="autocut"}>SMARTCUT</button>{/if}
        <button class:active={!downloaderOpen&&workspaceMode === "batch"} onclick={() => setWorkspaceMode("batch")} disabled={operationBusy&&workspaceMode!=="batch"}>{language === "tr" ? "TOPLU" : "BATCH"}</button>
      </nav>
      <div class="project-actions"><button onclick={saveProject} disabled={operationBusy}>{language==="tr"?"PROJEYİ KAYDET":"SAVE PROJECT"}</button></div>
      <button class="ghost top-cancel" onclick={closeMedia} disabled={operationBusy}>{t("close")}</button>
    {:else}
      <div class="landing-header-actions">
        <div class="language-switch landing-language"><button class:active={language==="tr"} onclick={()=>setLanguage("tr")}>TR</button><button class:active={language==="en"} onclick={()=>setLanguage("en")}>EN</button><i></i><button class="theme-button" class:active={theme==="dark"} title={language==="tr"?"Koyu tema":"Dark theme"} aria-label={language==="tr"?"Koyu tema":"Dark theme"} onclick={()=>setTheme("dark")}>☾</button><button class="theme-button" class:active={theme==="light"} title={language==="tr"?"Açık tema":"Light theme"} aria-label={language==="tr"?"Açık tema":"Light theme"} onclick={()=>setTheme("light")}>☀</button></div>
        <div class="project-actions landing-project-actions"><button onclick={openProject} disabled={operationBusy}>{language==="tr"?"PROJE AÇ":"OPEN PROJECT"}</button></div>
        {#if updaterEnabled}<button class="update-trigger" class:available={!!availableUpdate} class:checking={updateChecking} onclick={() => checkForUpdates(true)} title={language === "tr" ? "Güncellemeleri denetle" : "Check for updates"}><b>↻</b><span>{availableUpdate ? `v${availableUpdate.version}` : (language === "tr" ? "GÜNCELLE" : "UPDATE")}</span>{#if availableUpdate}<i></i>{/if}</button>{/if}
      </div>
    {/if}
    <GeneralSettings {language} {theme} onlanguage={setLanguage} ontheme={setTheme} onresetlayout={media&&!downloaderOpen?()=>panelResetDialogOpen=true:undefined} layoutBusy={operationBusy}/>
  </header>

  {#if panelResetDialogOpen}
    <dialog class="panel-reset-dialog" use:mountPanelResetDialog oncancel={()=>panelResetDialogOpen=false} aria-labelledby="panel-reset-title" aria-describedby="panel-reset-description">
      <header><span class="panel-reset-dialog-icon" aria-hidden="true">↺</span><h2 id="panel-reset-title">{language==="tr"?"Panel düzenini sıfırla":"Reset panel layout"}</h2></header>
      <p id="panel-reset-description">{workspaceMode==="autocut"?(language==="tr"?"SmartCut yan panelleri ve zaman çizelgesi varsayılan boyutlarına dönecek.":"SmartCut side panels and timeline will return to their default sizes."):workspaceMode==="batch"?(language==="tr"?"Batch kontrol paneli varsayılan genişliğine dönecek.":"The Batch controls panel will return to its default width."):(language==="tr"?"Araçlar ve Parametreler panelleri varsayılan genişliklerine dönecek.":"Tools and Parameters will return to their default widths.")} {language==="tr"?"Proje ve düzenleme geçmişin değişmeyecek.":"Your project and editing history will stay unchanged."}</p>
      <footer><button class="panel-reset-cancel" onclick={()=>panelResetDialogOpen=false}>{language==="tr"?"İPTAL":"CANCEL"}</button><button class="panel-reset-confirm" onclick={confirmResetPanelWidths}>{language==="tr"?"SIFIRLA":"RESET"}</button></footer>
    </dialog>
  {/if}

  {#if updatePanel}
    <div class="update-layer">
      <button class="update-backdrop" aria-label={language === "tr" ? "Güncelleme penceresini kapat" : "Close update dialog"} onclick={() => { if (!updateInstalling) updatePanel = false; }}></button>
      <dialog class="update-dialog panel" open use:containDialog aria-labelledby="update-title">
        <header><div><span class="status-dot"></span><h2 id="update-title">CONTAINER UPDATE</h2></div><button onclick={() => updatePanel = false} disabled={updateInstalling} aria-label={language === "tr" ? "Kapat" : "Close"}>×</button></header>
        <div class="update-version"><span>v{appVersion}</span><b>→</b><strong>{availableUpdate ? `v${availableUpdate.version}` : `v${appVersion}`}</strong></div>
        <p>{updateStatus}</p>
        {#if availableUpdate?.body}<pre>{availableUpdate.body}</pre>{/if}
        {#if updateInstalling}
          <div class="update-progress"><i style:width={`${updateTotal ? Math.min(100, updateDownloaded / updateTotal * 100) : 8}%`}></i></div>
          <small class="mono">{updateTotal ? `${(updateDownloaded/1048576).toFixed(1)} / ${(updateTotal/1048576).toFixed(1)} MB` : (language === "tr" ? "hazırlanıyor…" : "preparing…")}</small>
        {/if}
        <footer>
          <button class="ghost" onclick={() => checkForUpdates(true)} disabled={updateChecking || updateInstalling}>{language === "tr" ? "TEKRAR DENE" : "CHECK AGAIN"}</button>
          {#if availableUpdate}<button class="install-update" onclick={installAvailableUpdate} disabled={updateInstalling}>{updateInstalling ? (language === "tr" ? "KURULUYOR…" : "INSTALLING…") : (language === "tr" ? "İNDİR VE GÜNCELLE" : "DOWNLOAD & UPDATE")}</button>{/if}
        </footer>
      </dialog>
    </div>
  {/if}

  {#if projectFilesOpen}
    <div class="update-layer project-files-layer">
      <button class="update-backdrop" aria-label={language==="tr"?"Proje dosyaları penceresini kapat":"Close project files"} onclick={()=>projectFilesOpen=false}></button>
      <dialog class="update-dialog project-files-dialog panel" open use:containDialog aria-labelledby="project-files-title">
        <header><div><span class="status-dot"></span><h2 id="project-files-title">{language==="tr"?"PROJE DOSYALARI":"PROJECT FILES"}</h2></div><button aria-label={language==="tr"?"Kapat":"Close"} onclick={()=>projectFilesOpen=false}>×</button></header>
        <p>{language==="tr"?"Proje dosyası medya içermez. Projeyi başka yere taşıyacaksan kaynakları aynı klasör düzeniyle yanında tut; kayıt sırasında göreli yollar da saklanır.":"Project files do not contain media. Keep the sources in the same folder layout when moving a project; relative paths are saved as a fallback."}</p>
        <div class="project-file-list">{#each projectFileChecks as item}<div><b class:missing={!item.exists}>{item.exists?"✓":"!"}</b><span><strong>{item.resource.label}</strong><small title={item.resource.path}>{item.resource.path}</small></span></div>{/each}</div>
        {#if projectFileChecks.some(item=>!item.exists)}<p class="project-files-warning">{language==="tr"?"Eksik dosyaları yeniden bağlamadan bu proje tam olarak işlenemez.":"Missing files must be relinked before the project can be processed completely."}</p>{/if}
        <footer><button class="ghost" onclick={()=>projectFilesOpen=false}>{language==="tr"?"KAPAT":"CLOSE"}</button></footer>
      </dialog>
    </div>
  {/if}

  {#if dependencyPanel && !updatePanel}
    <div class="update-layer dependency-layer">
      <button class="update-backdrop" aria-label={language === "tr" ? "FFmpeg bildirimini kapat" : "Close FFmpeg notice"} onclick={() => dependencyPanel = false}></button>
      <dialog class="update-dialog dependency-dialog panel" open use:containDialog aria-labelledby="dependency-title">
        <header><div><span class="status-dot missing"></span><h2 id="dependency-title">{runtimeMigrationError ? (language === "tr" ? "FFMPEG GÜNCELLEMESİ TAMAMLANMADI" : "FFMPEG UPDATE DID NOT FINISH") : (language === "tr" ? "FFMPEG GEREKLİ" : "FFMPEG REQUIRED")}</h2></div><button onclick={() => dependencyPanel = false} aria-label={language === "tr" ? "Kapat" : "Close"}>×</button></header>
        <div class="dependency-message"><span>!</span><div><h3>{runtimeMigrationError ? (language === "tr" ? "SONRAKİ GÜNCELLEME İÇİN BİR ADIM GEREKİYOR" : "ONE STEP IS NEEDED FOR THE NEXT UPDATE") : (language === "tr" ? "MEDYA ARAÇLARI HENÜZ KULLANILAMAZ" : "MEDIA TOOLS ARE NOT READY YET")}</h3><p>{runtimeMigrationError ? (language === "tr" ? "CONTAINER, FFmpeg bileşenlerini güvenli güncelleme alanına hazırlayamadı. Bu sürüm çalışmaya devam eder; ancak sonraki küçük güncellemelerden önce aşağıdaki işlemi tekrar dene. Sorun sürerse bu sürümü yeniden kur." : "CONTAINER could not prepare its FFmpeg components for future lightweight updates. This version can still run; retry below before the next update. If it continues, reinstall this version.") : (language === "tr" ? "Kurulumla gelen FFmpeg bileşenleri bulunamadı veya çalıştırılamadı. CONTAINER’ı yeniden kurup tekrar dene." : "The FFmpeg components included with CONTAINER are missing or could not start. Reinstall CONTAINER and try again.")}</p>{#if runtimeMigrationError}<small class="runtime-migration-detail mono">{runtimeMigrationError}</small>{/if}</div></div>
        <footer><button class="ghost" onclick={() => dependencyPanel = false}>{language === "tr" ? "ŞİMDİ DEĞİL" : "NOT NOW"}</button><button class="dependency-check" onclick={runtimeMigrationError ? repairFfmpegRuntime : () => refreshFfmpegStatus(true)} disabled={dependencyChecking}>{dependencyChecking ? "…" : (language === "tr" ? (runtimeMigrationError ? "YENİDEN HAZIRLA" : "TEKRAR KONTROL ET") : (runtimeMigrationError ? "REPAIR RUNTIME" : "CHECK AGAIN"))}</button></footer>
      </dialog>
    </div>
  {/if}

  {#if !media && outputCleanupOpen}
    <div class="update-layer output-clean-layer">
      <button class="update-backdrop" aria-label={language==="tr"?"Çıktı temizleme penceresini kapat":"Close output cleanup dialog"} onclick={()=>{if(!outputCleaning)outputCleanupOpen=false}}></button>
      <dialog class="update-dialog output-clean-dialog panel" open use:containDialog aria-labelledby="output-clean-title">
        <header><div><span class="output-clean-dialog-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M9 7V4h6v3m3 0-1 13H7L6 7m4 4v6m4-6v6"/></svg></span><h2 id="output-clean-title">{language==="tr"?"CONTAINER OUTPUT TEMİZLE":"CLEAN CONTAINER OUTPUT"}</h2></div><button onclick={()=>outputCleanupOpen=false} disabled={outputCleaning} aria-label={language==="tr"?"Kapat":"Close"}>×</button></header>
        <p>{language==="tr"?"Downloads/CONTAINER Output içindeki tüm çıktılar Geri Dönüşüm Kutusu’na taşınacak; klasör yerinde kalacak.":"Everything inside Downloads/CONTAINER Output will be moved to the Recycle Bin; the folder itself will remain."}</p>
        {#if outputCleanupMessage}<small class="output-clean-error">{outputCleanupMessage}</small>{/if}
        <footer><button class="ghost" onclick={()=>outputCleanupOpen=false} disabled={outputCleaning}>{language==="tr"?"İPTAL":"CANCEL"}</button><button class="clean-confirm" onclick={cleanOutputFolder} disabled={outputCleaning}>{outputCleaning?"…":(language==="tr"?"ÇIKTILARI TEMİZLE":"CLEAN OUTPUT")}</button></footer>
      </dialog>
    </div>
  {/if}

  {#if downloaderOpen}
    <DownloaderWorkspace {language} hasMedia={!!media} onback={()=>downloaderOpen=false} onbusychange={(value:boolean)=>downloaderBusy=value} onopenmedia={openDownloadedMedia} />
  {:else if !media}
    <section class="landing">
      {#if autoEncoderTuning}
        <div class="first-run-tuning" role="status" aria-live="polite">
          <i aria-hidden="true"></i>
          <div><b>{language==="tr"?"PERFORMANS AYARLANIYOR":"TUNING PERFORMANCE"}</b><span>{language==="tr"?"Kısa ve sessizdir; yalnızca ekran kartı veya FFmpeg değişince tekrarlanır.":"A short silent test; it repeats only when the GPU or FFmpeg changes."}</span></div>
        </div>
      {/if}
      {#if recoveryCandidate}
        <section class="recovery-card panel">
          <div><span>↻</span><div><h3>{language==="tr"?"ÖNCEKİ ÇALIŞMA BULUNDU":"PREVIOUS WORK FOUND"}</h3><p><b>{basename(recoveryCandidate.mediaPath)}</b> · {new Date(recoveryCandidate.savedAt).toLocaleString(language==="tr"?"tr-TR":"en-US")}</p></div></div>
          <aside><button class="ghost" onclick={discardRecovery}>{language==="tr"?"VAZGEÇ":"DISCARD"}</button><button class="restore-session" onclick={restorePreviousSession} disabled={restoringSession}>{restoringSession?"…":(language==="tr"?"ÇALIŞMAYI GERİ YÜKLE":"RESTORE WORK")}</button></aside>
        </section>
      {/if}
      <button class="dropzone" class:active={dragActive} onclick={selectMedia} disabled={ffmpegStatus !== null && !ffmpegStatus.ready}>
        <span class="drop-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none">
            <path d="M12 15V4M12 4 7.8 8.2M12 4l4.2 4.2M5 14.5v3.25A2.25 2.25 0 0 0 7.25 20h9.5A2.25 2.25 0 0 0 19 17.75V14.5" />
          </svg>
        </span>
        <h1>{t("drop")}</h1>
        <p>{t("browse")}</p>
        <small class="multi-import-hint">{language==="tr"?"Birden fazla video seç → toplu işlem":"Select multiple videos → batch processing"}</small>
        <div class="format-row"><span>{t("video")}</span><span>{t("audio")}</span><span>{t("image")}</span></div>
      </button>
      {#if ffmpegStatus && !ffmpegStatus.ready}
        <section class="dependency-card">
          <div><span>!</span><div><h3>{language === "tr" ? "FFMPEG BİLEŞENİ EKSİK" : "FFMPEG COMPONENT MISSING"}</h3><p>{language === "tr" ? "CONTAINER kurulumuna dahil olan medya bileşenleri bulunamadı. Uygulamayı yeniden kurup tekrar dene." : "Media components included with CONTAINER were not found. Reinstall the application and try again."}</p></div></div>
          <aside><button class="dependency-check" onclick={() => refreshFfmpegStatus(true)} disabled={dependencyChecking}>{dependencyChecking ? "…" : (language === "tr" ? "TEKRAR KONTROL ET" : "CHECK AGAIN")}</button></aside>
        </section>
      {/if}
      <div class="landing-copy motto-only">
        <h2>{t("landingTitle")}</h2>
      </div>
      <footer class="landing-engine-status"><span class="status-dot" class:missing={(ffmpegStatus !== null && !ffmpegStatus.ready)||(downloaderStatus!==null&&!downloaderStatus.ready)}></span> {ffmpegStatus?.ready&&downloaderStatus?.ready ? `FFMPEG · FFPROBE · YT-DLP ${t("ready").toUpperCase()}` : (dependencyChecking||downloaderStatus===null ? (language==="tr"?"BİLEŞENLER KONTROL EDİLİYOR":"CHECKING COMPONENTS") : (language==="tr"?"MEDYA BİLEŞENLERİ GEREKLİ":"MEDIA COMPONENTS REQUIRED"))}</footer>
      <button class="output-clean-trigger al-icon-wrapper" onclick={()=>{outputCleanupMessage="";outputCleanupOpen=true}} title={language==="tr"?"CONTAINER Output klasörünü temizle":"Clean CONTAINER Output"} aria-label={language==="tr"?"CONTAINER Output klasörünü temizle":"Clean CONTAINER Output"}>
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="M10 11v6" class="trash-handle trash-delay-0" />
          <path d="M14 11v6" class="trash-fill trash-delay-1" />
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" class="trash-fill trash-delay-2" />
          <path d="M3 6h18" class="trash-fill trash-delay-3" />
          <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" class="trash-handle trash-delay-4" />
        </svg>
      </button>
      <button class="downloader-quick-trigger" onclick={()=>downloaderOpen=true} title={language==="tr"?"DWLNDR’ı aç":"Open DWLNDR"} aria-label={language==="tr"?"DWLNDR’ı aç":"Open DWLNDR"}>
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 3v11M8 10l4 4 4-4M5 18v2a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2" /></svg>
        <span>{language==="tr"?"Bağlantıdan indir":"Download from link"}</span>
      </button>
      {#if outputCleanupMessage}<div class="output-clean-toast" role="status">{outputCleanupMessage}</div>{/if}
    </section>
  {:else}
    {#if workspaceMode === "autocut" && media.kind === "video"}
      <SmartCutWorkspace bind:this={autoCutWorkspace} {media} {mediaUrl} {language} onaddstack={addSmartCutStack} stackHasCuts={processingStack.some(step=>step.tool.id==="smartcut")} stackCount={processingStack.length} oncontinue={continueEditingOutput} onhistorychange={(undo:boolean,redo:boolean)=>{autoCutCanUndo=undo;autoCutCanRedo=redo}} onsessionchange={(value:unknown)=>{autoCutSession=value}} onbusychange={(value:boolean)=>autoCutBusy=value} />
    {:else if workspaceMode === "batch"}
      <BatchWorkspace bind:this={batchWorkspace} initialPaths={batchInitialPaths.length?batchInitialPaths:[media.path]} initialKind={media.kind} {language} {availableEncoders} oncontinue={continueEditingOutput} onhistorychange={(undo:boolean,redo:boolean)=>{batchCanUndo=undo;batchCanRedo=redo}} onsessionchange={(value:unknown)=>{batchSession=value}} onbusychange={(value:boolean)=>batchBusy=value} onqueuechange={(count:number)=>batchQueueCount=count} />
    {:else}
    {@const panelSizes=toolboxPanelSizes()}
    <section class="workspace resizable" bind:this={toolboxWorkspace} style={`--toolbox-left:${panelSizes.left}px;--toolbox-right:${panelSizes.right}px`}>
      <aside class="tool-pane panel">
        <div class="pane-head">
          <div><h3>{t("tools")}</h3><p>{kindTools(activeKind).length} {t("available")}</p></div>
          <span class="media-pill">{activeKind}</span>
        </div>
        <div class="tabs">
          {#each (["video","audio","image"] as MediaKind[]) as kind}
            <button class:active={activeKind === kind} onclick={() => switchKind(kind)} disabled={kind !== media?.kind && !(media?.kind === "video" && kind === "audio")}>{t(kind)}</button>
          {/each}
        </div>
        <input class="search" bind:value={search} placeholder={t("search")} />
        <button class="favorites-filter" class:active={favoritesOnly} onclick={()=>favoritesOnly=!favoritesOnly}><span>★</span>{language==="tr"?"FAVORİLER":"FAVORITES"}<b>{visibleFavoriteCount}</b></button>
        <div class="tool-scroll">
          {#each categories as [category, entries]}
            <section class="tool-group" data-category={toolCategory(entries[0].id)}>
              <h4>{category}</h4>
              {#each entries as tool}
                <div class="tool-entry">
                  <button class="tool-row" class:active={selected?.id === tool.id} title={tool.description} onclick={() => chooseTool(tool)}>
                    <i class:blue={tool.accent === "blue"} class:green={tool.accent === "green"} class:purple={tool.accent === "purple"} class:red={tool.accent === "red"} class:yellow={tool.accent === "yellow"}></i>
                    <span><b>{tool.title}</b><small>{toolSummary(tool,language)}</small></span><em>›</em>
                  </button>
                  <button class="favorite-toggle" class:active={favoriteIds.includes(tool.id)} onclick={()=>toggleFavorite(tool.id)} title={language==="tr"?"Favoriye ekle/kaldır":"Add/remove favorite"} aria-label={language==="tr"?`${tool.title} favori`:`Favorite ${tool.title}`}>★</button>
                </div>
              {/each}
            </section>
          {/each}
          {#if categories.length===0}<div class="favorites-empty"><p>{search.trim()?(language==="tr"?"Aramana uygun araç bulunamadı.":"No matching tools found."):(language==="tr"?"Bu bölümde henüz favori araç yok.":"No favorite tools in this section yet.")}</p>{#if search.trim()}<button class="ghost" onclick={()=>search=""}>{language==="tr"?"Aramayı temizle":"Clear search"}</button>{/if}{#if favoritesOnly}<button class="ghost" onclick={()=>favoritesOnly=false}>{language==="tr"?"Tüm araçları göster":"Show all tools"}</button>{/if}</div>{/if}
        </div>
      </aside>

      <div class="workspace-resizer" role="slider" tabindex="0" aria-label={language==="tr"?"Araçlar panelinin genişliği":"Tools panel width"} aria-orientation="horizontal" aria-valuemin={panelSizes.minLeft} aria-valuemax={Math.min(560,panelSizes.available-panelSizes.right-panelSizes.minCenter)} aria-valuenow={Math.round(panelSizes.left)} onpointerdown={(event)=>startToolboxPanelResize(event,"left")} onkeydown={(event)=>toolboxPanelKey(event,"left")} ondblclick={resetToolboxPanelWidths} title={language==="tr"?"Genişliği sürükle · sıfırla: çift tık":"Drag to resize · double-click to reset"}></div>

      <section class="center-stack" class:timeline-active={timelineTool}>
        <div class="preview panel" class:stack-preview={media.kind==="video"&&processingStack.length>0}>
          <div class="preview-head"><span>{stackResultPreview&&outputMode==="stack"&&!outputStale?(language==="tr"?"STACK SONUCU":"STACK RESULT"):processingStack.length?(language==="tr"?"ARAÇ ÖNİZLEMESİ":"TOOL PREVIEW"):t("preview")}</span>{#if multiRegionClipper&&!(stackResultPreview&&outputMode==="stack"&&!outputStale)}<div class="clipper-preview-tabs" role="group" aria-label={language==="tr"?"Clipper önizleme görünümü":"Clipper preview view"}><button class:active={clipperPreviewMode==="source"} aria-pressed={clipperPreviewMode==="source"} onclick={()=>clipperPreviewMode="source"}>{language==="tr"?"KAYNAK BÖLGELERİ":"SOURCE REGIONS"}</button><button class:active={clipperPreviewMode==="output"} aria-pressed={clipperPreviewMode==="output"} onclick={()=>clipperPreviewMode="output"}>{language==="tr"?"ÇIKTI ÖNİZLEME":"OUTPUT PREVIEW"}</button></div>{:else}<span class="mono">{t(media.kind).toUpperCase()} · {media.codec.toUpperCase()}</span>{/if}{#if processingStack.length}<button class="stack-preview-toggle ghost" onclick={()=>{toolboxVideo?.pause();stackResultPreview=!stackResultPreview}} disabled={!output||outputMode!=="stack"||outputStale} title={language==="tr"?"Tüm adımların birleşmiş sonucu renderdan sonra görüntülenir.":"The combined result of all steps is available after rendering."}>{stackResultPreview?(language==="tr"?"Araca dön":"Back to tool"):(language==="tr"?"Stack sonucu":"Stack result")}</button>{/if}</div>
          <div class="media-stage" class:ac-player={media.kind === "video"} class:toolbox-player={media.kind === "video"} bind:this={toolboxStage}>
            {#if media.kind === "video"&&stackResultPreview&&output&&outputMode==="stack"&&!outputStale}
              <!-- svelte-ignore a11y_media_has_caption -->
              <div class="video-canvas"><video bind:this={toolboxVideo} class="stack-result-video" aria-label={language==="tr"?"Render edilmiş işlem listesi sonucu":"Rendered Stack result"} src={convertFileSrc(output)} preload="metadata" onloadedmetadata={()=>{stackResultDuration=Number.isFinite(toolboxVideo?.duration)?toolboxVideo!.duration:0;toolboxCurrent=0;toolboxPlaying=false;if(toolboxVideo)toolboxVideo.volume=toolboxVolume}} ontimeupdate={()=>{if(toolboxVideo)toolboxCurrent=toolboxVideo.currentTime}} onplay={()=>toolboxPlaying=true} onpause={()=>toolboxPlaying=false} onended={()=>toolboxPlaying=false}></video></div>
            {:else if media.kind === "video"}
              <div class="video-canvas" bind:this={toolboxCanvas}>
              <!-- svelte-ignore a11y_media_has_caption -->
              {#if selected?.id==="clipper"&&toolValue("vertical_layout")==="blur"}
                {@const blurBox=verticalOutputBox()}
                {#if blurBox}<BlurBackdropPreview video={toolboxVideo} box={blurBox} strength={toolNumber("blur_strength")}/>{/if}
              {/if}
              {#if selected?.id==="clipper"&&toolValue("vertical_layout")==="original"}<div style={originalCanvasStyle()}></div>{/if}
              <!-- svelte-ignore a11y_media_has_caption -->
              <div style={blurForegroundBoxStyle()}>
                <video bind:this={toolboxVideo} crossorigin="anonymous" style={previewVideoStyle()} src={mediaUrl} preload={clipperOutputVisible?"auto":"metadata"} onloadedmetadata={handleToolboxMetadata} ontimeupdate={() => { if (toolboxVideo) toolboxCurrent = toolboxVideo.currentTime;  }} onplay={() => {toolboxPlaying=true;}} onpause={() => {toolboxPlaying=false;}} onended={() => {toolboxPlaying=false;}}></video>
              </div>
              {#if selected?.id==="noise"}<NoisePreview video={toolboxVideo} amount={toolNumber("amount")} {language}/>{/if}
              {#if clipperOutputVisible}
                {@const outputBox=verticalOutputBox()}
                {#if outputBox}<ClipperLayoutPreview video={toolboxVideo} input={clipperLayoutInput()} box={outputBox} />{/if}
              {/if}
              {#if multiRegionClipper&&clipperPreviewMode==="source"}
                <div class="transform-source-box" bind:this={transformSourceBox} style={transformBoxStyle()}>
                  {#each [{id:"a" as const,label:"CAMERA REGION"},{id:"b" as const,label:"CONTENT REGION"}] as region}
                    <div class={`transform-crop transform-region region-${region.id}`} style:left={`${toolNumber(`region_${region.id}_x`)}%`} style:top={`${toolNumber(`region_${region.id}_y`)}%`} style:width={`${toolNumber(`region_${region.id}_w`)}%`} style:height={`${toolNumber(`region_${region.id}_h`)}%`} onpointerdown={(event)=>startTransformRegion(event,region.id,"move")} role="presentation">
                      <b>{region.label}</b>
                      {#each transformHandles as handle}<button class={`crop-handle ${handle}`} aria-label={`Resize ${region.label} ${handle}`} onpointerdown={(event)=>startTransformRegion(event,region.id,handle)}></button>{/each}
                    </div>
                  {/each}
                </div>
              {:else if selected?.id==="clipper"&&toolValue("vertical_layout")==="fill"}
                {@const fillBox=verticalOutputBox()}
                 {#if fillBox}<div class="fill-pan-layer" style:left={`${fillBox.left}px`} style:top={`${fillBox.top}px`} style:width={`${fillBox.width}px`} style:height={`${fillBox.height}px`} onpointerdown={startFillPan} onwheel={wheelClipperZoom} role="presentation"><span>{language==="tr"?"SÜRÜKLE · TEKERLEKLE YAKINLAŞ":"DRAG · WHEEL TO ZOOM"}</span></div>{/if}
              {:else if selected?.id==="clipper"&&["blur","original"].includes(toolValue("vertical_layout"))}
                 <div class="fill-pan-layer" style={blurPanLayerStyle()} onpointerdown={startBlurPan} onwheel={wheelClipperZoom} role="presentation"><span>{language==="tr"?"SÜRÜKLE · TEKERLEKLE YAKINLAŞ":"DRAG · WHEEL TO ZOOM"}</span></div>
              {:else if selected?.id==="transform" && toolValue("crop_mode") !== "off"}
                <div class="transform-source-box" bind:this={transformSourceBox} style={transformBoxStyle()}>
                  <div class="crop-shade top" style:height={`${toolNumber("crop_y")}%`}></div>
                  <div class="crop-shade left" style:left="0" style:top={`${toolNumber("crop_y")}%`} style:width={`${toolNumber("crop_x")}%`} style:height={`${toolNumber("crop_h")}%`}></div>
                  <div class="crop-shade right" style:left={`${toolNumber("crop_x")+toolNumber("crop_w")}%`} style:top={`${toolNumber("crop_y")}%`} style:right="0" style:height={`${toolNumber("crop_h")}%`}></div>
                  <div class="crop-shade bottom" style:top={`${toolNumber("crop_y")+toolNumber("crop_h")}%`}></div>
                  <div class="transform-crop" style:left={`${toolNumber("crop_x")}%`} style:top={`${toolNumber("crop_y")}%`} style:width={`${toolNumber("crop_w")}%`} style:height={`${toolNumber("crop_h")}%`} onpointerdown={(event)=>startTransformCrop(event,"move")} role="presentation">
                    <i class="crop-grid v one"></i><i class="crop-grid v two"></i><i class="crop-grid h one"></i><i class="crop-grid h two"></i>
                    {#each transformHandles as handle}<button class={`crop-handle ${handle}`} aria-label={`Resize crop ${handle}`} onpointerdown={(event)=>startTransformCrop(event,handle)}></button>{/each}
                  </div>
                </div>
               {/if}
               {#if selected?.id==="clipper"&&clipperGuidesActive&&["original","blur","fill"].includes(toolValue("vertical_layout"))}
                 {@const output=verticalOutputBox()}
                 {@const guides=clipperGuideAxes()}
                 {#if output&&(guides.x||guides.y)}
                   <div class="clipper-center-guides" style:left={`${output.left}px`} style:top={`${output.top}px`} style:width={`${output.width}px`} style:height={`${output.height}px`}>
                     {#if guides.x}<i class="vertical"></i>{/if}
                     {#if guides.y}<i class="horizontal"></i>{/if}
                     {#if guides.x&&guides.y}<span>{language==="tr"?"MERKEZ":"CENTER"}</span>{/if}
                   </div>
                 {/if}
               {/if}
               {#if selected?.id==="clipper"&&(!multiRegionClipper||clipperOutputVisible)&&toolValue("watermark_enabled")==="true"&&toolValue("watermark_text").trim()}
                {#if watermarkLayer&&watermarkFontReady===watermarkLayer.font_path}<WatermarkPreview layer={watermarkLayer} width={toolNumber("output_width")||1080} height={toolNumber("output_height")||1920} box={verticalOutputBox()} onchange={changeWatermark}/>{:else}
                <i class="clipper-watermark-background" style={clipperWatermarkPreviewStyle(true)}></i>
                <span class="clipper-watermark-preview" style={clipperWatermarkPreviewStyle()}>{toolValue("watermark_text")}</span>
                {/if}
              {/if}
              {#if selected?.id==="clipper"&&(!multiRegionClipper||clipperOutputVisible)&&toolValue("social_tag_enabled")==="true"&&toolValue("social_tag_username").trim()}
                {#if toolValue("social_tag_style")==="kick_banner"}
                  <div class="clipper-social-banner" style={socialBannerPreviewStyle()} aria-label="Kick.com banner preview">
                    <span class="clipper-social-banner-bg"></span>
                    <span class="clipper-social-banner-logo"><img class="clipper-social-banner-logo-art" src={kickBanner} alt="" /></span>
                    <span class="clipper-social-banner-prefix"><img class="clipper-social-banner-prefix-art" src={kickBanner} alt="" /></span>
                    <span class="clipper-social-banner-name">{toolValue("social_tag_username").trim().toUpperCase()}</span>
                  </div>
                {:else}
                  <div class="clipper-social-tag" class:boxed={toolValue("social_tag_style")==="boxed"} class:twitch={toolValue("social_tag_platform")==="twitch"} style={socialTagPreviewStyle()}>
                    <span class="clipper-social-icon"><img src={toolValue("social_tag_platform")==="twitch"?twitchMark:kickMark} alt="" /></span>
                    <span class="clipper-social-name">{toolValue("social_tag_username").trim()}</span>
                  </div>
                {/if}
              {/if}
              {#if selected?.id === "text"}
                <div class="text-preview-layer">
                  <canvas class="text-preview-canvas" bind:this={textPreviewCanvas} style={textPreviewCanvasStyle()}></canvas>
                  {#each textLayers as layer (layer.id)}
                    <div class="preview-text" class:active={activeTextId===layer.id} style={textLayerStyle(layer)} role="group" aria-label={`${language==="tr"?"Yazı katmanı":"Text layer"}: ${layer.text}`} onpointerdown={(event)=>startTextDrag(event,layer)}>
                      <i class="text-size-handle left" role="presentation" aria-label="Resize text from left" onpointerdown={(event)=>startTextDrag(event,layer,-1)}></i>
                      <span>{wrappedText(layer)}</span>
                      <i class="text-size-handle right" role="presentation" aria-label="Resize text from right" onpointerdown={(event)=>startTextDrag(event,layer,1)}></i>
                      <i class="text-size-handle nw" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,-1,-1)}></i><i class="text-size-handle ne" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,1,-1)}></i><i class="text-size-handle sw" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,-1,1)}></i><i class="text-size-handle se" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,1,1)}></i>
                    </div>
                  {/each}
                </div>
              {/if}
              {#if selected?.id === "image_overlay" && toolValue("image_path") && toolboxCurrent >= toolNumber("start") && toolboxCurrent <= toolNumber("end")}
                <button class="overlay-preview-box" style={overlayPreviewStyle()} onpointerdown={(event)=>startOverlayDrag(event)} aria-label="Move overlay">
                  <img bind:this={overlayPreviewImage} src={convertFileSrc(toolValue("image_path"))} alt="Overlay preview" draggable="false" style:opacity={toolNumber("opacity")/100} />
                  <i class="text-size-handle left" role="presentation" aria-label="Resize overlay from left" onpointerdown={(event)=>startOverlayDrag(event,-1)}></i>
                  <i class="text-size-handle right" role="presentation" aria-label="Resize overlay from right" onpointerdown={(event)=>startOverlayDrag(event,1)}></i>
                  <i class="text-size-handle nw" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,-1,-1)}></i><i class="text-size-handle ne" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,1,-1)}></i><i class="text-size-handle sw" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,-1,1)}></i><i class="text-size-handle se" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,1,1)}></i>
                </button>
              {/if}
              {#if selected?.id === "blur_pixelate"}
                <div class="transform-source-box" bind:this={transformSourceBox} style={mediaBoxStyle()}>
                  <div class="effect-region" class:pixelated={toolValue("effect")==="pixelate"} style:left={`${toolNumber("region_x")}%`} style:top={`${toolNumber("region_y")}%`} style:width={`${toolNumber("region_w")}%`} style:height={`${toolNumber("region_h")}%`} style={`--effect-strength:${Math.max(1,toolNumber("strength")/3)}px`} onpointerdown={(event)=>startEffectRegion(event,"move")} role="presentation">
                    {#each transformHandles as handle}<button class={`crop-handle ${handle}`} aria-label={`Resize effect ${handle}`} onpointerdown={(event)=>startEffectRegion(event,handle)}></button>{/each}
                  </div>
                </div>
              {/if}
              </div>
            {:else if media.kind === "audio"}
              <div class="audio-visual"><div class="disc">◉</div><h2>{media.name}</h2><p>{media.codec.toUpperCase()} · {formatDuration(media.duration)}</p><audio src={mediaUrl} controls></audio></div>
            {:else if selected && ["transform","text","image_overlay","blur_pixelate","color"].includes(selected.id)}
              <div class="transform-image-canvas" bind:this={toolboxCanvas}>
                <img style={selected.id==="transform"?transformPreviewStyle():`${neutralPreviewStyle()};${colorPreviewStyle()}`} src={mediaUrl} alt={media.name} draggable="false" />
                {#if selected.id === "transform" && toolValue("crop_mode") !== "off" && toolValue("fit_mode") !== "contain"}
                  <div class="transform-source-box" bind:this={transformSourceBox} style={transformBoxStyle()}>
                    <div class="crop-shade top" style:height={`${toolNumber("crop_y")}%`}></div>
                    <div class="crop-shade left" style:left="0" style:top={`${toolNumber("crop_y")}%`} style:width={`${toolNumber("crop_x")}%`} style:height={`${toolNumber("crop_h")}%`}></div>
                    <div class="crop-shade right" style:left={`${toolNumber("crop_x")+toolNumber("crop_w")}%`} style:top={`${toolNumber("crop_y")}%`} style:right="0" style:height={`${toolNumber("crop_h")}%`}></div>
                    <div class="crop-shade bottom" style:top={`${toolNumber("crop_y")+toolNumber("crop_h")}%`}></div>
                    <div class="transform-crop" style:left={`${toolNumber("crop_x")}%`} style:top={`${toolNumber("crop_y")}%`} style:width={`${toolNumber("crop_w")}%`} style:height={`${toolNumber("crop_h")}%`} onpointerdown={(event)=>startTransformCrop(event,"move")} role="presentation">
                      <i class="crop-grid v one"></i><i class="crop-grid v two"></i><i class="crop-grid h one"></i><i class="crop-grid h two"></i>
                      {#each transformHandles as handle}<button class={`crop-handle ${handle}`} aria-label={`Resize crop ${handle}`} onpointerdown={(event)=>startTransformCrop(event,handle)}></button>{/each}
                    </div>
                  </div>
                {/if}
                {#if selected.id === "text"}
                  <div class="text-preview-layer">
                    <canvas class="text-preview-canvas" bind:this={textPreviewCanvas} style={textPreviewCanvasStyle()}></canvas>
                    {#each textLayers as layer (layer.id)}
                      <div class="preview-text" class:active={activeTextId===layer.id} style={textLayerStyle(layer)} role="group" aria-label={`${language==="tr"?"Yazı katmanı":"Text layer"}: ${layer.text}`} onpointerdown={(event)=>startTextDrag(event,layer)}>
                      <i class="text-size-handle left" role="presentation" aria-label="Resize text from left" onpointerdown={(event)=>startTextDrag(event,layer,-1)}></i><span>{wrappedText(layer)}</span><i class="text-size-handle right" role="presentation" aria-label="Resize text from right" onpointerdown={(event)=>startTextDrag(event,layer,1)}></i><i class="text-size-handle nw" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,-1,-1)}></i><i class="text-size-handle ne" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,1,-1)}></i><i class="text-size-handle sw" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,-1,1)}></i><i class="text-size-handle se" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,1,1)}></i>
                      </div>
                    {/each}
                  </div>
                {/if}
                {#if selected.id === "image_overlay" && toolValue("image_path")}
                  <button class="overlay-preview-box" style={overlayPreviewStyle()} onpointerdown={(event)=>startOverlayDrag(event)} aria-label="Move overlay">
                    <img bind:this={overlayPreviewImage} src={convertFileSrc(toolValue("image_path"))} alt="Overlay preview" draggable="false" style:opacity={toolNumber("opacity")/100} />
                  <i class="text-size-handle left" role="presentation" aria-label="Resize overlay from left" onpointerdown={(event)=>startOverlayDrag(event,-1)}></i><i class="text-size-handle right" role="presentation" aria-label="Resize overlay from right" onpointerdown={(event)=>startOverlayDrag(event,1)}></i><i class="text-size-handle nw" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,-1,-1)}></i><i class="text-size-handle ne" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,1,-1)}></i><i class="text-size-handle sw" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,-1,1)}></i><i class="text-size-handle se" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,1,1)}></i>
                  </button>
                {/if}
                {#if selected.id === "blur_pixelate"}
                  <div class="transform-source-box" bind:this={transformSourceBox} style={mediaBoxStyle()}>
                    <div class="effect-region" class:pixelated={toolValue("effect")==="pixelate"} style:left={`${toolNumber("region_x")}%`} style:top={`${toolNumber("region_y")}%`} style:width={`${toolNumber("region_w")}%`} style:height={`${toolNumber("region_h")}%`} style={`--effect-strength:${Math.max(1,toolNumber("strength")/3)}px`} onpointerdown={(event)=>startEffectRegion(event,"move")} role="presentation">
                      {#each transformHandles as handle}<button class={`crop-handle ${handle}`} aria-label={`Resize effect ${handle}`} onpointerdown={(event)=>startEffectRegion(event,handle)}></button>{/each}
                    </div>
                  </div>
                {/if}
              </div>
            {:else}
              <div class="image-viewport" class:dragging={imageDragging} bind:this={imageViewport} onwheel={zoomImage} onpointerdown={startImagePan} ondblclick={resetImageView} role="presentation">
                {#if renderedImageUrl}
                  <div class="image-compare" onkeydown={imageCompareKey} role="slider" tabindex="0" aria-label="Original and rendered image comparison" aria-valuemin="0" aria-valuemax="100" aria-valuenow={imageCompare}>
                    <img class="compare-rendered zoomable-image" style:transform={imageTransform()} src={renderedImageUrl} alt={`Rendered ${media.name}`} draggable="false" />
                    <div class="compare-original-clip" style:clip-path={`inset(0 ${100-imageCompare}% 0 0)`}>
                      <img class="compare-original zoomable-image" style:transform={imageTransform()} src={mediaUrl} alt={`Original ${media.name}`} draggable="false" onload={initializeImageView} />
                    </div>
                    <span class="compare-label original">{t("original")}</span><span class="compare-label rendered">{t("rendered")}</span>
                    <button class="compare-handle" style:left={`${imageCompare}%`} onpointerdown={startImageCompare} title={language === "tr" ? "Karşılaştırma çizgisini sürükle" : "Drag the comparison line"}><b>↔</b></button>
                  </div>
                {:else}
                  <img class="zoomable-image" style:transform={imageTransform()} src={mediaUrl} alt={media.name} draggable="false" onload={initializeImageView} />
                {/if}
                <div class="image-view-controls">
                  <button onclick={() => setImageZoom(imageZoom / 1.2)} aria-label={language === "tr" ? "Uzaklaştır" : "Zoom out"}>−</button>
                  <span class="mono">{Math.round(imageZoom * 100)}%</span>
                  <button onclick={() => setImageZoom(imageZoom * 1.2)} aria-label={language === "tr" ? "Yakınlaştır" : "Zoom in"}>+</button>
                  <button class="fit" onclick={resetImageView}>{language === "tr" ? "SIĞDIR" : "FIT"}</button>
                </div>
                <span class="image-view-hint mono">{language === "tr" ? "TEKERLEK: YAKINLAŞTIR · SÜRÜKLE: TAŞI" : "SCROLL: ZOOM · DRAG: PAN"}</span>
              </div>
            {/if}
            {#if media.kind==="video"}
              <div class="ac-controls">
                {#if playerSeekHover}<span class="player-seek-tooltip mono" class:precision={playerSeekHover.precision} style:left={`${playerSeekHover.percent}%`}>{playerTime(playerSeekHover.time)}</span>{/if}
                <input class="player-seek" style={`--seek-pct:${playbackDuration?Math.min(100,toolboxCurrent/playbackDuration*100):0}%`} aria-label="Video position" type="range" min="0" max={playbackDuration} step="0.01" value={toolboxCurrent} onpointerdown={precisionPlayerSeek} onpointermove={hoverPlayerSeek} onpointerleave={()=>playerSeekHover=null} onwheel={wheelPlayerSeek} oninput={event=>seekToolbox(Number(event.currentTarget.value))}>
                <button onclick={()=>seekToolboxBy(-5)} title="5 seconds back">−5</button>
                <button class="play" onclick={toggleToolboxPlayer} title="Play / Pause">{toolboxPlaying?"Ⅱ":"▶"}</button>
                <button onclick={()=>seekToolboxBy(5)} title="5 seconds forward">+5</button>
                <span class="ac-time mono">{playerTime(toolboxCurrent)} <i>/</i> {playerTime(playbackDuration)}</span>
                <input class="volume" aria-label="Volume" type="range" min="0" max="1" step="0.05" bind:value={toolboxVolume} oninput={()=>{if(toolboxVideo)toolboxVideo.volume=toolboxVolume}}>
                <button onclick={fullscreenToolboxPlayer} title="Fullscreen">⛶</button>
              </div>
            {/if}
          </div>
        </div>

        {#if timelineTool && media.duration}
          <div class="tool-timeline panel">
            <header><div><h3>{language==="tr"?"ZAMAN ÇİZELGESİ":"TIMELINE"}</h3><p>{selected?.id === "screenshot" ? (language==="tr"?"kare zamanını seç":"choose frame time") : (language==="tr"?"çıktı aralığını seç":"choose export range")}</p></div>{#if selected?.id==="cut"}<span class="timeline-current mono"><i>▶</i> {language==="tr"?"KONUM":"PLAYHEAD"} {editableTime(toolboxCurrent)}</span>{/if}<b class="mono">{selected?.id === "screenshot" ? playerTime(timelineBounds().start) : `${timelineTime(timelineBounds().start)} — ${timelineTime(timelineBounds().end)}`}</b></header>
            {#if rangeTimelineTool()}<div class="cut-timecodes"><div class="cut-timecode"><span>{language==="tr"?"BAŞLANGIÇ":"START"} <i>H:M:S</i></span><input aria-label={language==="tr"?"Başlangıç zamanı":"Start time"} class="mono" bind:value={cutStartInput} onfocus={()=>cutTimeEditing="start"} onblur={()=>commitCutTime("start")} onkeydown={(event)=>handleCutTimeKey(event,"start")} placeholder="0:05:14"><button onclick={()=>markCutAtPlayhead("start")} title={language==="tr"?"Geçerli oynatma zamanını başlangıç yap (I)":"Set IN to current playhead time (I)"}><b>IN</b><kbd>I</kbd></button></div><div class="cut-timecode"><span>{language==="tr"?"BİTİŞ":"END"} <i>H:M:S</i></span><input aria-label={language==="tr"?"Bitiş zamanı":"End time"} class="mono" bind:value={cutEndInput} onfocus={()=>cutTimeEditing="end"} onblur={()=>commitCutTime("end")} onkeydown={(event)=>handleCutTimeKey(event,"end")} placeholder="0:05:46"><button onclick={()=>markCutAtPlayhead("end")} title={language==="tr"?"Geçerli oynatma zamanını bitiş yap (O)":"Set OUT to current playhead time (O)"}><b>OUT</b><kbd>O</kbd></button></div><small class="cut-seek-help mono">{language==="tr"?"PLAYER ÇUBUĞU: tekerlek ±1 sn · Ctrl+tekerlek ±5 sn · Shift+tık tam saniye":"PLAYER BAR: wheel ±1 sec · Ctrl+wheel ±5 sec · Shift+click whole second"}</small></div>{/if}
            {#if selected?.id==="screenshot"}<div class="screenshot-timecode"><label for="screenshot-time">{language==="tr"?"Kare zamanı":"Frame timestamp"}</label><input id="screenshot-time" class="mono" aria-label={language==="tr"?"Kare zamanı":"Frame timestamp"} value={editableTime(toolNumber("timestamp"))} placeholder="0:00:05.250" onblur={(event)=>{setScreenshotTime(event.currentTarget.value);event.currentTarget.value=editableTime(toolNumber("timestamp"))}} onkeydown={(event)=>{if(event.key==="Enter"){event.preventDefault();event.currentTarget.blur()}if(event.key==="Escape"){event.currentTarget.value=editableTime(toolNumber("timestamp"));event.currentTarget.blur()}}}/><small>{language==="tr"?"Saniye, M:S veya H:M:S · örnek: 5.250":"Seconds, M:S or H:M:S · example: 5.250"}</small></div>{/if}
            <div class="tool-wave" bind:this={toolboxTimeline} onclick={seekTimeline} onpointermove={hoverTimeline} onpointerleave={()=>timelineHover=null} role="presentation">
              {#if toolboxFilmstripUrl}<img class="filmstrip" src={toolboxFilmstripUrl} alt="Video filmstrip" draggable="false">{:else}<span class="wave-loading">{toolboxFilmstripLoading ? (language==="tr"?"video kareleri hazırlanıyor…":"building video frames…") : "—"}</span>{/if}
              {#if selected?.id === "screenshot"}
                <i class="timeline-point" role="slider" tabindex="0" aria-label="Timestamp" aria-valuemin="0" aria-valuemax={media.duration} aria-valuenow={timelineBounds().start} style:left={`${timelineBounds().start/media.duration*100}%`} onkeydown={(event)=>timelineHandleKey(event,"point")} onpointerdown={(event)=>startToolTimelineDrag(event,"point")}><b></b></i>
              {:else}
                <div class="timeline-selection" style:left={`${timelineBounds().start/media.duration*100}%`} style:width={`${Math.max(0,timelineBounds().end-timelineBounds().start)/media.duration*100}%`} onpointerdown={(event)=>startToolTimelineDrag(event,"range")} role="presentation">
                  <i class="timeline-edge left" role="slider" tabindex="0" aria-label="Start" aria-valuemin="0" aria-valuemax={timelineBounds().end} aria-valuenow={timelineBounds().start} onkeydown={(event)=>timelineHandleKey(event,"start")} onpointerdown={(event)=>startToolTimelineDrag(event,"start")}></i><i class="timeline-edge right" role="slider" tabindex="0" aria-label="End" aria-valuemin={timelineBounds().start} aria-valuemax={media.duration} aria-valuenow={timelineBounds().end} onkeydown={(event)=>timelineHandleKey(event,"end")} onpointerdown={(event)=>startToolTimelineDrag(event,"end")}></i>
                </div>
              {/if}
              {#if timelineHover!==null}<i class="timeline-hover" class:right={timelineHover>85} style:left={`${timelineHover}%`}><b>{playerTime(media.duration*timelineHover/100)}</b></i>{/if}
              <em class="timeline-playhead" style:left={`${toolboxCurrent/media.duration*100}%`}></em>
            </div>
            <div class="tool-ruler mono"><span>{timelineTime(0)}</span><span>{timelineTime(media.duration/4)}</span><span>{timelineTime(media.duration/2)}</span><span>{timelineTime(media.duration*3/4)}</span><span>{timelineTime(media.duration)}</span></div>
          </div>
        {/if}

        <div class="job panel">
          <div class="job-head">
            <div class="job-status"><h3>{t("process")}</h3><div class="job-status-line"><p class="mono">{outputStale?(language==="tr"?"ayarlar değişti · yeniden işle":"settings changed · render again"):jobStatus}</p></div></div>
            <div class="job-meta">
              <div class="job-stats mono"><span><b>{t("frame")}</b>{frame}</span><span><b>{t("speed")}</b>{speed}</span><span><b>{t("elapsed")}</b>{elapsed.toFixed(1)}s</span></div>
              {#if output||busy}<div class="job-actions">{#if output}
                <button class="ghost job-action play-render" disabled={operationBusy} onclick={playRenderedOutput} title={language==="tr"?"Son çıktıyı aç":"Open the last output"}>▶ {output.endsWith(".frames")?(language==="tr"?"kareler":"frames"):(language==="tr"?"oynat":"play")}</button>
                <button class="ghost job-action" onclick={() => {if(outputStale)showToast(language==="tr"?"Bu eski çıktı. Yeni ayarları görmek için yeniden işle.":"This is the old output. Render again to see the new settings.","info");revealItemInDir(output)}}>{outputStale?(language==="tr"?"eski çıktı · güncellenmedi":"old output · not updated"):t("showOutput")}</button><button class="ghost job-action" disabled={operationBusy||outputStale} title={outputStale?(language==="tr"?"Ayarlar değişti; önce yeniden işle.":"Settings changed; render again first."):undefined} onclick={()=>continueEditingOutput()}>{language==="tr"?"Kaynak olarak aç":"Open as source"}</button>{/if}{#if busy}<button class="danger job-action" onclick={cancelJob}>{t("cancelJob")}</button>{/if}</div>{/if}
            </div>
          </div>
          <div class="job-feedback"><PremiereSend path={output} {language} disabled={operationBusy||outputStale}/><RenderFeedback running={busy} {progress} {language} {speed} {elapsed} {output}/></div>
          <div class="progress-track"><div style:width={`${progress}%`}></div></div>
          {#if error}<div class="error-box"><ProblemDetails reason={error} {language}/></div>{/if}
        </div>
      </section>

      <div class="workspace-resizer" role="slider" tabindex="0" aria-label={language==="tr"?"Parametreler panelinin genişliği":"Parameters panel width"} aria-orientation="horizontal" aria-valuemin={panelSizes.minRight} aria-valuemax={Math.min(620,panelSizes.available-panelSizes.left-panelSizes.minCenter)} aria-valuenow={Math.round(panelSizes.right)} onpointerdown={(event)=>startToolboxPanelResize(event,"right")} onkeydown={(event)=>toolboxPanelKey(event,"right")} ondblclick={resetToolboxPanelWidths} title={language==="tr"?"Genişliği sürükle · sıfırla: çift tık":"Drag to resize · double-click to reset"}></div>

      <aside class="settings panel" class:merge-compact={selected?.id==="merge_videos"} class:compact-controls={panelSizes.right<300}>
        {#if selected}
          <div class="pane-head tool-settings-head"><div><h2>{selected.title}</h2><p>{selected.description}</p></div><button class="reset" onclick={resetSelectedTool}>{t("defaults")}</button></div>
          {#if media.kind==="video"}
            <section class="processing-stack" aria-label="Processing Stack">
              <header><b>{language==="tr"?"İŞLEM LİSTESİ":"PROCESSING STACK"}</b><small>{processingStack.length}</small></header>
              {#if processingStack.length}
                <div class="processing-stack-list">
                  {#each processingStack as step,index (step.id)}
                    <div class:current={editingStackStepId===step.id} class="processing-stack-step">
                      <input type="checkbox" aria-label={`${step.tool.title} ${language==="tr"?"etkin":"enabled"}`} checked={step.enabled} disabled={operationBusy} onchange={(event)=>{step.enabled=event.currentTarget.checked;flushEditorSnapshot();persistRecovery()}}>
                      <button class="processing-stack-name" onclick={()=>selectStackStep(step)} disabled={operationBusy}>{index+1}. {step.tool.title}</button>
                      <button aria-label="Move up" onclick={()=>moveStackStep(index,-1)} disabled={operationBusy||index===0||step.tool.id==="smartcut"||processingStack[index-1]?.tool.id==="smartcut"}>↑</button>
                      <button aria-label="Move down" onclick={()=>moveStackStep(index,1)} disabled={operationBusy||index===processingStack.length-1||step.tool.id==="smartcut"}>↓</button>
                      <button aria-label="Remove step" onclick={()=>removeStackStep(step.id)} disabled={operationBusy}>×</button>
                    </div>
                  {/each}
                </div>
              {/if}
              {#if stackToolIds.has(selected.id)}<button class="processing-stack-add" onclick={addOrUpdateStackStep} disabled={operationBusy}>{editingStackStepId!==null&&processingStack.some(step=>step.id===editingStackStepId)?(language==="tr"?"Seçili adımı güncelle":"Update selected step"):(language==="tr"?"+ Listeye ekle":"+ Add to stack")}</button>{/if}
              {#if stackDraftDirty()}<small class="processing-stack-draft">{language==="tr"?"Bu adımda kaydedilmemiş ayarlar var.":"This step has unapplied settings."}</small>{/if}
              {#if processingStack.length}<details class="stack-help"><summary>{language==="tr"?"Liste nasıl çalışır?":"How does the Stack work?"}</summary><p>{language==="tr"?"Araç önizlemesi yalnızca seçili aracı gösterir. Tüm adımların birleşmiş sonucunu renderdan sonra Stack sonucu ile izle. Cut zamanları orijinal kaynak videoya aittir; önceki kesimlerde kaldırılan bölümler dahil edilmez. Ara adımlar kayıpsız dosyalar kullanır ve ek disk alanı gerektirir.":"Tool preview shows only the selected tool. View the combined result with Stack result after rendering. Cut times refer to the original source; regions removed by earlier cuts are excluded. Intermediate steps use lossless files and require extra disk space."}</p></details>{/if}
            </section>
          {/if}
          {#if !["transform","clipper","cut","text","color","merge_videos"].includes(selected.id)}<div class="explain"><b>{t("what")}</b><p>{selected.id==="fix_timestamps"&&media.kind==="audio"?(language==="tr"?"Hızlı onarım, sesi kalite kaybı olmadan yeniden paketler. Derin onarım sesi FLAC olarak yeniden kodlar; yalnızca hızlı yöntem yetmezse kullan.":"Fast Repair remuxes audio without quality loss. Deep Repair re-encodes audio as FLAC; use it only when the fast method is not enough."):selected.detail}</p></div>{/if}
          {#if selected.id === "merge_videos"}
            <div class="merge-list">
              <button class="merge-add" onclick={addMergeVideos}>＋ {language==="tr"?"VİDEO EKLE":"ADD VIDEOS"}</button>
              {#each mergeInputs as path,index}
                <article><b class="mono">{index+1}</b><span title={path}>{basename(path)}</span><aside><button onclick={()=>moveMergeVideo(index,-1)} disabled={index===0} aria-label="Move up">↑</button><button onclick={()=>moveMergeVideo(index,1)} disabled={index===mergeInputs.length-1} aria-label="Move down">↓</button><button onclick={()=>removeMergeVideo(index)} aria-label="Remove">×</button></aside></article>
              {/each}
            </div>
          {/if}
          {#if selected.id === "subtitles" && toolValue("action")==="extract" && !subtitleTracks.length}
            <div class="codec-note"><b>{language==="tr"?"ALTYAZI YOK":"NO SUBTITLES"}</b><span>{language==="tr"?"Bu dosyada çıkarılabilir gömülü altyazı bulunamadı.":"No extractable embedded subtitle track was found in this file."}</span></div>
          {/if}
          {#if recommendation()}<div class="recommend"><b>{t("forVideo")}</b><span>{recommendation()}</span></div>{/if}
          <div class="field-list">
          {#if selected.id === "color"}
            <div class="color-workspace">
              <div class="color-top-actions"><button class="reset-filters" onclick={resetColorFilters}>{language==="tr"?"Sıfırla":"Reset"}</button><button class:active={!colorPreviewVisible} class="compare-color" onclick={()=>colorPreviewVisible=!colorPreviewVisible}>{colorPreviewVisible?(language==="tr"?"Öncesini göster":"Show before"):(language==="tr"?"Sonrasını göster":"Show after")}</button></div>
              <section class="color-presets"><h4>{language==="tr"?"Hızlı görünümler":"Quick looks"}</h4><div>{#each [["natural",language==="tr"?"Doğal":"Natural"],["cinematic",language==="tr"?"Sinematik":"Cinematic"],["warm",language==="tr"?"Sıcak":"Warm"],["cold",language==="tr"?"Soğuk":"Cold"],["bw",language==="tr"?"Siyah-beyaz":"B&W"]] as preset}<button onclick={()=>applyColorPreset(preset[0])}>{preset[1]}</button>{/each}</div></section>
              {#each colorGroups as group}
                <section class="color-group"><h4>{group.title}</h4>
                  {#each group.keys as key}
                    {@const field=toolField(key)}
                    {#if field}
                      <label class="color-control">
                        <span><b>{language==="tr"?(field.label):colorLabels[key]}</b><em>{colorValueLabel(key)}</em><button type="button" aria-label={language==="tr"?`${field.label} sıfırla`:`Reset ${colorLabels[key]}`} title={language==="tr"?"Sıfırla":"Reset"} onclick={(event)=>{event.preventDefault();resetColorKey(key)}}>↻</button></span>
                        <input aria-label={language==="tr"?field.label:colorLabels[key]} type="range" style={`--range-pct:${rangePercent(displayedColorValue(key),Number(field.min),Number(field.max))}%`} min={field.min} max={field.max} step={field.step} value={displayedColorValue(key)} oninput={(event)=>adjustColor(key,Number(event.currentTarget.value))}>
                      </label>
                    {/if}
                  {/each}
                  {#if group.title === "Cleanup"}
                    <div class="denoise-control">
                      <span>{language==="tr"?"Gürültü azaltma":"Denoise"}</span>
                      <div class="segmented">{#each [["off",language==="tr"?"Kapalı":"Off"],["low",language==="tr"?"Az":"Low"],["medium",language==="tr"?"Orta":"Medium"],["high",language==="tr"?"Yüksek":"High"]] as mode}<button class:active={toolValue("denoise")===mode[0]} onclick={()=>setToolValue("denoise",mode[0])}>{mode[1]}</button>{/each}</div>
                    </div>
                  {/if}
                  {#if group.title === "Style"}
                    <button class="ghost color-grayscale" aria-pressed={toolValue("grayscale")==="on"} onclick={()=>setToolValue("grayscale",toolValue("grayscale")==="on"?"off":"on")}>{language==="tr"?"Gri tonlama":"Grayscale"}</button>
                  {/if}
                </section>
              {/each}
              <section class="color-group"><h4>{language==="tr"?"Tarama":"Interlace"}</h4><div class="segmented">{#each ["off","auto","on"] as mode}<button class:active={toolValue("deinterlace")===mode} onclick={()=>setToolValue("deinterlace",mode)}>{mode}</button>{/each}</div></section>
            </div>
          {:else if selected.id === "text"}
            <div class="text-workspace">
              <section class="text-presets">
                <h4 title={language==="tr"?"Seçili yazıyı, konumunu ve tüm görünüm ayarlarını kaydet. Uygula yeni bir katman ekler.":"Save the selected text, position and all appearance settings. Add from preset creates a new layer."}>{language==="tr"?"Yazı presetleri":"Text presets"}</h4>
                {#if textPresets.length}
                  <div class="text-preset-row">
                    <select aria-label={language==="tr"?"Kayıtlı preset":"Saved preset"} bind:value={selectedTextPreset}><option value="">{language==="tr"?"Preset seç":"Choose a preset"}</option>{#each textPresets as preset (preset.id)}<option value={preset.id}>{preset.name}</option>{/each}</select>
                    <button class="ghost" disabled={!selectedTextPreset||textPresetBusy} onclick={applyTextPreset}>{language==="tr"?"Presetten ekle":"Add from preset"}</button>
                    <button class="ghost preset-delete" aria-label={language==="tr"?"Preseti sil":"Delete preset"} title={language==="tr"?"Preseti sil":"Delete preset"} disabled={!selectedTextPreset||textPresetBusy} onclick={deleteTextPreset}><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7"/></svg></button>
                  </div>
                {/if}
                <div class="text-preset-row">
                  <input aria-label={language==="tr"?"Preset adı":"Preset name"} maxlength="80" bind:value={textPresetName} placeholder={language==="tr"?"Yeni preset adı…":"New preset name…"}>
                  <button class="ghost" aria-label={language==="tr"?"Seçili yazıyı kaydet":"Save selected text"} title={language==="tr"?"Seçili yazıyı kaydet":"Save selected text"} disabled={!activeText()||!textPresetName.trim()} onclick={saveTextPreset}>{language==="tr"?"Kaydet":"Save"}</button>
                </div>
                {#if selectedTextPreset}<details class="preset-manage"><summary>{language==="tr"?"Seçili preseti düzenle":"Edit selected preset"}</summary><div class="text-preset-row"><button class="ghost" disabled={!activeText()||textPresetBusy} onclick={updateTextPreset}>{language==="tr"?"Seçili yazıyla güncelle":"Update from selected text"}</button><button class="ghost" disabled={!textPresetName.trim()||textPresetBusy} onclick={renameTextPreset}>{language==="tr"?"Adı değiştir":"Rename"}</button></div><small>{language==="tr"?"Adı değiştirmek için üstteki ad alanını kullan.":"Use the name field above to rename."}</small></details>{/if}
              </section>
              <button class="add-text" onclick={addTextLayer}>＋ {language==="tr"?"Yazı ekle":"Add text"}</button>
              {#if textLayers.length}
                <div class="text-tabs">{#each textLayers as layer,index (layer.id)}<div class="text-tab" class:active={activeTextId===layer.id}><button class="text-tab-select" onclick={()=>activeTextId=layer.id}>{index+1}. {layer.text||"—"}</button><button class="text-tab-remove" aria-label={`${language==="tr"?"Yazıyı kaldır":"Remove text"}: ${layer.text||index+1}`} title={language==="tr"?"Yazıyı kaldır":"Remove text"} onclick={()=>removeTextLayer(layer.id)}>×</button></div>{/each}</div>
                {@const layer=activeText()}
                {#if layer}
                  <label class="field"><span>{language==="tr"?"Yazı":"Text"}</span><textarea class="text-content-input" rows="3" value={layer.text} oninput={(event)=>updateTextLayer({text:event.currentTarget.value})}></textarea></label>
                  <label class="field"><span>{language==="tr"?"Font":"Font"}</span><select value={layer.font_path} onchange={(event)=>chooseTextFont(event.currentTarget.value)}>{#each systemFonts as font}<option value={font.path}>{font.name}</option>{/each}</select></label>
                  <div class="text-color-editor">
                    <span>{language==="tr"?"Renk":"Color"}</span>
                    <div class="text-color-row"><i style:background={layer.color}></i><input aria-label="Hex color" value={layer.color} maxlength="7" onchange={(event)=>setTextColor(event.currentTarget.value)}></div>
                    <div class="text-swatches">{#each textColors as color}<button class:active={layer.color===color} style:background={color} aria-label={`Use ${color}`} onclick={()=>setTextColor(color)}></button>{/each}</div>
                  </div>
                  <label class="field"><span>{language==="tr"?"Yazı boyutu":"Font size"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.size,8,600)}%`} min="8" max="600" step="1" value={layer.size} oninput={(event)=>resizeTextLayer(Number(event.currentTarget.value))}><small class="hint">{Math.round(layer.size)} px</small></label>
                  <label class="field"><span>{language==="tr"?"Opaklık":"Opacity"}<small>%</small></span><input type="range" style={`--range-pct:${rangePercent(layer.opacity,0,100)}%`} min="0" max="100" step="1" value={layer.opacity} oninput={(event)=>updateTextLayer({opacity:Number(event.currentTarget.value)})}><small class="hint">{Math.round(layer.opacity)}%</small></label>
                  <label class="field"><span>{language==="tr"?"Hizalama":"Alignment"}</span><select value={layer.align} onchange={(event)=>updateTextLayer({align:event.currentTarget.value as "left"|"center"|"right"})}><option value="left">{language==="tr"?"Sol":"Left"}</option><option value="center">{language==="tr"?"Orta":"Center"}</option><option value="right">{language==="tr"?"Sağ":"Right"}</option></select></label>
                  <div class="text-position-grid" aria-label={language==="tr"?"Yazı konumu":"Text position"}>{#each ["top-left","top-center","top-right","middle-left","middle-center","middle-right","bottom-left","bottom-center","bottom-right"] as position}<button title={position.replace("-"," ")} onclick={()=>positionText(position)}></button>{/each}</div>
                  <details class="text-style-options">
                    <summary>{language==="tr"?"Kontur, gölge ve arka plan":"Stroke, shadow & background"}</summary>
                    <label class="color-toggle"><input type="checkbox" checked={layer.outline>0} onchange={(event)=>updateTextLayer({outline:event.currentTarget.checked?Math.max(2,layer.outline):0})}><span>{language==="tr"?"Kontur açık":"Stroke enabled"}</span></label>
                    {#if layer.outline>0}<label class="field"><span>{language==="tr"?"Kontur genişliği":"Stroke width"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.outline,1,20)}%`} min="1" max="20" step="1" value={layer.outline} oninput={(event)=>updateTextLayer({outline:Number(event.currentTarget.value)})}><small class="hint">{layer.outline}px</small></label>
                    <label class="field"><span>{language==="tr"?"Kontur rengi":"Stroke color"}</span><input type="text" value={layer.outline_color} maxlength="7" onchange={(event)=>/^#[0-9a-f]{6}$/i.test(event.currentTarget.value)&&updateTextLayer({outline_color:event.currentTarget.value})}></label>{/if}
                    <label class="field"><span>{language==="tr"?"Gölge":"Shadow"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.shadow,0,30)}%`} min="0" max="30" step="1" value={layer.shadow} oninput={(event)=>updateTextLayer({shadow:Number(event.currentTarget.value)})}><small class="hint">{layer.shadow}px</small></label>
                    <label class="color-toggle"><input type="checkbox" checked={layer.background} onchange={(event)=>updateTextLayer({background:event.currentTarget.checked})}><span>{language==="tr"?"Arka plan kutusu":"Background box"}</span></label>
                    {#if layer.background}
                      <label class="field"><span>{language==="tr"?"Arka plan rengi":"Background color"}</span><input type="text" value={layer.background_color} maxlength="7" onchange={(event)=>/^#[0-9a-f]{6}$/i.test(event.currentTarget.value)&&updateTextLayer({background_color:event.currentTarget.value})}></label>
                      <label class="field"><span>{language==="tr"?"Arka plan opaklığı":"Background opacity"}<small>%</small></span><input type="range" style={`--range-pct:${rangePercent(layer.background_opacity,0,100)}%`} min="0" max="100" step="1" value={layer.background_opacity} oninput={(event)=>updateTextLayer({background_opacity:Number(event.currentTarget.value)})}><small class="hint">{layer.background_opacity}%</small></label>
                      <label class="field"><span>{language==="tr"?"İç boşluk":"Padding"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.background_padding,0,80)}%`} min="0" max="80" step="1" value={layer.background_padding} oninput={(event)=>updateTextLayer({background_padding:Number(event.currentTarget.value)})}><small class="hint">{layer.background_padding}px</small></label>
                    {/if}
                  </details>
                  <p class="text-help">{language==="tr"?"Yazıyı sürükle; Shift ile yatay/dikey eksene kilitle. Kenar veya köşe tutamaçlarından boyutlandır.":"Drag text; hold Shift to lock movement to one axis. Resize from side or corner handles."}</p>
                {/if}
              {:else}<p class="text-empty">{language==="tr"?"Önizlemeye ilk katmanı eklemek için Yazı ekle’ye bas.":"Choose Add text to place the first layer in the preview."}</p>{/if}
            </div>
          {/if}
          {#if selected.id === "discord_compressor"}
            {@const budget = discordBudget()}
            <details class="discord-help">
              <summary><b>?</b><span>{language === "tr" ? "SIKIŞTIRMA REHBERİ" : "COMPRESSION GUIDE"}</span></summary>
            <div class="discord-guide">
              <h4>{language === "tr" ? "KALİTE NASIL KORUNUYOR?" : "HOW QUALITY IS PRESERVED"}</h4>
              <ol>
                <li><b>1</b><span>{language === "tr" ? "Önce seçilen MB sınırından gerçek toplam bitrate hesaplanır." : "The real total bitrate is calculated from the selected MB limit."}</span></li>
                <li><b>2</b><span>{language === "tr" ? "Akıllı ses, dar bütçede 64; orta bütçede 96; rahat bütçede 128 kbps AAC seçer." : "Smart audio uses 64 kbps for tight, 96 kbps for medium, and 128 kbps AAC for roomy budgets."}</span></li>
                <li><b>3</b><span>{language === "tr" ? "Normal videoda çözünürlük ve FPS birlikte ayarlanır. Düşük hareketli ekran/yazı videosu algılanırsa okunabilirlik ve akıcılık için kaynak çözünürlük ile FPS korunur." : "Resolution and FPS are adjusted together for normal footage. For detected low-motion screen/text video, source resolution and FPS are preserved for readability and smoothness."}</span></li>
                <li><b>4</b><span>{language === "tr" ? "İki geçiş, sakin sahnelerden artırdığı alanı hareketli sahnelere verir; çıktı büyük kalırsa güvenli bitrate ile tekrar dener." : "Two-pass encoding gives bits saved on calm scenes to complex motion and retries safely if the result is oversized."}</span></li>
              </ol>
              <div class="codec-note"><b>H.264</b><span>{language === "tr" ? "Discord ve cihazlarla en güvenli uyumluluk." : "Safest compatibility across Discord and devices."}</span><b>H.265</b><span>{language === "tr" ? "Aynı boyutta daha iyi görüntü verebilir; eski cihazlarda siyah ekran veya yalnız ses riski vardır." : "Can look better at the same size; older clients may show a black screen or audio only."}</span></div>
              {#if budget && budget.videoKbps < 180}
                <p class="budget-warning">{language === "tr" ? `Bu video için sınır çok dar: görüntüye yalnızca yaklaşık ${budget.videoKbps} kbps kalıyor. Akıllı mod ${budget.screenLike ? "ekran/yazı içeriğini algıladığı için kaynak çözünürlük ve FPS" : "240p / 15 FPS"} kullanacak. Daha temiz görüntü için en etkili çözüm 50 MB seçmek veya videoyu kısaltmaktır.` : `This target is extremely tight: only about ${budget.videoKbps} kbps remains for video. Smart mode will use ${budget.screenLike ? "source resolution and FPS because screen/text content was detected" : "240p / 15 FPS"}. Choosing 50 MB or trimming the video is the most effective quality improvement.`}</p>
              {:else if budget && budget.videoKbps < 750}
                <p class="budget-warning mild">{language === "tr" ? `Bütçe sınırlı (~${budget.videoKbps} kbps). Akıllı çözünürlük ve FPS önerilir.` : `The budget is limited (~${budget.videoKbps} kbps). Smart resolution and FPS are recommended.`}</p>
              {/if}
              <small>{language === "tr" ? "Önerilen Discord başlangıçları: 20 MB → en fazla 480p, 50 MB → 720p, 100 MB → 1080p, 500 MB → kaynak. Uzun videolarda Akıllı mod bunlardan daha aşağı inebilir." : "Suggested Discord starting points: 20 MB → up to 480p, 50 MB → 720p, 100 MB → 1080p, 500 MB → source. Smart mode may go lower for long videos."}</small>
            </div>
            </details>
          {/if}
          {#if selected.id === "encode"}
            <div class="quality-guide">
              <h4>{language === "tr" ? "KAYNAK VE CODEC REHBERİ" : "SOURCE & CODEC GUIDE"}</h4>
              <p><b>{language === "tr" ? "Kaynak:" : "Source:"}</b> {media.pixel_format ?? "unknown"}{media.bits_per_raw_sample ? ` · ${media.bits_per_raw_sample}-bit` : ""}{media.color_transfer ? ` · ${media.color_transfer}` : ""}</p>
              <p><b>{encoderQualityMode()} {toolNumber("crf")}:</b> {language==="tr"?(encoderQualityMode()==="CRF"&&toolNumber("crf")===0?"CPU'da kayıpsız; dosya çok büyük.":"Düşük değer = daha temiz ve daha büyük dosya."):(encoderQualityMode()==="CRF"&&toolNumber("crf")===0?"Lossless on CPU; very large file.":"Lower value = cleaner picture and larger file.")}</p>
              <ul>
                <li><b>H.264</b><span>{language === "tr" ? "En uyumlu seçenek." : "Best compatibility."}</span></li>
                <li><b>HEVC</b><span>{language === "tr" ? "Daha küçük; eski cihaz desteği zayıf." : "Smaller; weaker legacy support."}</span></li>
                <li><b>VP9 / AV1</b><span>{language === "tr" ? "Verimli; CPU'da yavaş." : "Efficient; slow on CPU."}</span></li>
              </ul>
              <small>{language === "tr" ? "Yalnızca bu bilgisayarda testten geçen encoder’lar listelenir. Auto uyumlu bit derinliğini korur." : "Only encoders verified on this PC are listed. Auto preserves compatible bit depth."}</small>
            </div>
          {/if}
          {#if selected.id === "upscale" && media.width && media.height}
            {@const upscaleOutput=upscaleDimensions(toolNumber("target_edge"))}
            <div class="upscale-summary">
              <header><b>VIDEO</b><small>{language==="tr"?"Kaynak algılandı":"Source detected"}</small></header>
              <div><span>{language==="tr"?"Kaynak":"Source"}</span><strong>{media.width}×{media.height}{media.fps?` · ${media.fps.toFixed(2)} FPS`:""}</strong></div>
              <div><span>{language==="tr"?"Çıktı":"Output"}</span><strong>{upscaleOutput?`${upscaleOutput.width}×${upscaleOutput.height}`:"—"}{media.fps?` · ${media.fps.toFixed(2)} FPS`:""}</strong></div>
              <div><span>{language==="tr"?"Ölçekleme":"Scaling"}</span><strong>Lanczos · {language==="tr"?"yüksek kalite":"high quality"}</strong></div>
              <div><span>{language==="tr"?"Ses":"Audio"}</span><strong>{language==="tr"?"uyumluysa kopyala":"copy when compatible"}</strong></div>
              <div><span>{language==="tr"?"Kodlayıcı":"Encoder"}</span><strong>H.264 · CRF 14</strong></div>
            </div>
          {/if}
          {#if ["encode","cut","remux","extract_audio"].includes(selected.id) && media.audio_tracks.length}
            <div class="codec-note"><b>{language === "tr" ? "SES PARÇALARI" : "AUDIO TRACKS"}</b><span>{language === "tr" ? `${media.audio_tracks.length} parça bulundu. Ana varsayılandır; Tümü parçaları ayrı tutar; Birleştir hepsini tek dengeli ses parçasında toplar.` : `${media.audio_tracks.length} track(s) found. Main is the default; All keeps tracks separate; Merge combines them into one normalized track.`}</span></div>
          {/if}
          {#if ["transform","clipper"].includes(selected.id)}
            <div class="transform-controls">
              {#if selected.id==="clipper"}
                <section>
                  <header><b>{language==="tr"?"DİKEY YERLEŞİM":"VERTICAL LAYOUT"}</b><small>1080×1920</small></header>
                  <div class="transform-options three">
                    <button class:active={toolValue("vertical_layout")==="split"} onclick={()=>setVerticalLayout("split")}>{language==="tr"?"BÖL":"SPLIT"}</button>
                    <button class:active={toolValue("vertical_layout")==="squares"} onclick={()=>setVerticalLayout("squares")}>{language==="tr"?"KARELER":"SQUARES"}</button>
                    <button class:active={toolValue("vertical_layout")==="freecam"} onclick={()=>setVerticalLayout("freecam")}>FREECAM</button>
                    <button class:active={toolValue("vertical_layout")==="original"} onclick={()=>setVerticalLayout("original")}>{language==="tr"?"ORİJİNAL BOYUT":"ORIGINAL SIZE"}</button>
                    <button class:active={toolValue("vertical_layout")==="blur"} onclick={()=>setVerticalLayout("blur")}>{language==="tr"?"BULANIK":"BLUR"}</button>
                    <button class:active={toolValue("vertical_layout")==="fill"} onclick={()=>setVerticalLayout("fill")}>{language==="tr"?"DOLDUR":"FILL"}</button>
                  </div>
                  {#if ["split","squares","freecam"].includes(toolValue("vertical_layout"))}
                    <button class="auto-camera" class:working={cameraDetecting} onclick={autoDetectCamera} disabled={cameraDetecting||busy}><span>{cameraDetecting?"◌":"◇"}</span>{cameraDetecting?(language==="tr"?"KAMERA ARANIYOR…":"DETECTING CAMERA…"):(language==="tr"?"KAMERAYI OTOMATİK BUL":"AUTO-DETECT CAMERA")}<em title={language==="tr"?"Deneysel özellik":"Experimental feature"}>{language==="tr"?"DENEYSEL":"EXPERIMENTAL"}</em></button>
                    {#if cameraDetectionMessage}<p class="auto-camera-result">{cameraDetectionMessage}</p>{/if}
                  {/if}
                  {#if ["split","squares","freecam"].includes(toolValue("vertical_layout"))}
                    {@const contentTarget=contentTargetDimensions()}
                    <button class="center-content" onclick={centerContentRegion}>◎ {language==="tr"?"İÇERİĞİ ORTALA":"CENTER CONTENT"} · {contentTarget.width}×{contentTarget.height}</button>
                  {/if}
                  {#if toolValue("vertical_layout")==="original"}
                    <label class="field"><span>{language==="tr"?"Tuval arka planı":"Canvas background"}</span><select value={toolValue("canvas_background")} onchange={(event)=>setToolValue("canvas_background",event.currentTarget.value)}><option value="black">{language==="tr"?"Siyah":"Black"}</option><option value="white">{language==="tr"?"Beyaz":"White"}</option><option value="custom">{language==="tr"?"Özel renk":"Custom color"}</option></select></label>
                    {#if toolValue("canvas_background")==="custom"}<label class="field"><span>{language==="tr"?"Arka plan rengi":"Background color"}</span><input type="text" maxlength="7" value={toolValue("canvas_color")} oninput={(event)=>setToolValue("canvas_color",event.currentTarget.value)}></label>{/if}
                  {/if}
                  {#if toolValue("vertical_layout")==="fill"}<p>{language==="tr"?"Video dikey tuvali tamamen doldurur. Sonuç önizlemesini sürükleyerek yatay kadrajı ayarla.":"The video fills the vertical canvas completely. Drag the result preview to adjust the horizontal framing."}</p>{/if}
                  {#if toolValue("vertical_layout")==="blur"}
                    <label class="watermark-slider"><span>{language==="tr"?"Arka plan bulanıklığı":"Background blur"}<small>{toolNumber("blur_strength").toFixed(0)} px</small></span><input type="range" min="0" max="60" step="1" value={toolNumber("blur_strength")} oninput={(event)=>setToolNumber("blur_strength",Number(event.currentTarget.value))}></label>
                  {/if}
                  {#if ["original","blur","fill"].includes(toolValue("vertical_layout"))}
                    {#if toolValue("vertical_layout")==="fill"}<label class="watermark-slider"><span>{language==="tr"?"Video yakınlaştırma":"Video zoom"}<small>{toolNumber("clipper_zoom").toFixed(0)}%</small></span><input type="range" min="100" max="300" step="1" value={toolNumber("clipper_zoom")} oninput={(event)=>setToolNumber("clipper_zoom",Number(event.currentTarget.value))}></label>{/if}
                    <button class="center-content" onclick={()=>{setToolNumber("clipper_zoom",100);setToolNumber("clipper_x",50);setToolNumber("clipper_y",50)}}>{language==="tr"?"ZOOMU SIFIRLA":"RESET ZOOM"}</button>
                    {#if toolValue("vertical_layout")==="blur"}<p>{language==="tr"?"Net videoyu yakınlaştırıp sürükle; bulanık arka plan değişmez.":"Zoom and drag the sharp video; the blurred background stays unchanged."}</p>{/if}
                  {/if}
                  {#if toolValue("vertical_layout")==="squares"}<p>{language==="tr"?"Kamera ve içerik, dikey tuvali eşit iki tam genişlikte bölüme ayırır.":"Camera and content divide the vertical canvas into two equal full-width sections."}</p>{/if}
                  {#if toolValue("vertical_layout")==="freecam"}
                    {@const freecam=freecamPlacement()}
                    <div class="freecam-layout" bind:this={freecamLayoutBox} aria-label={language==="tr"?"Freecam çıktı yerleşimi":"Freecam output layout"}>
                      <span>{language==="tr"?"İÇERİK":"CONTENT"}</span>
                      <button class="freecam-camera" style:left={`${freecam.left}%`} style:top={`${freecam.top}%`} style:width={`${freecam.width}%`} style:height={`${freecam.height}%`} onpointerdown={(event)=>startFreecamPlacement(event,"move")}>
                        {language==="tr"?"KAMERA":"CAMERA"}<i role="presentation" onpointerdown={(event)=>startFreecamPlacement(event,"resize")}></i>
                      </button>
                    </div>
                    <p>{language==="tr"?"Kamerayı bu küçük yerleşimde sürükle; sağ alt köşeden boyutlandır. Kaynak kırpımını KAYNAK BÖLGELERİ görünümündeki Camera Region ile ayarla.":"Drag the camera in this layout; resize from the lower-right corner. Adjust its crop using Camera Region in SOURCE REGIONS."}</p>
                     <label><span class="slider-title">{language==="tr"?"Kamera konumu X":"Camera position X"}<button type="button" class="slider-reset" aria-label={language==="tr"?"Kamera X konumunu sıfırla":"Reset camera X"} onclick={(event)=>{event.preventDefault();setToolNumber("freecam_x",50)}}>↺</button></span><input aria-label={language==="tr"?"Kamera konumu X":"Camera position X"} type="range" min="0" max="100" step="1" value={toolNumber("freecam_x")} oninput={(event)=>setToolNumber("freecam_x",Number(event.currentTarget.value))}></label>
                     <label><span class="slider-title">{language==="tr"?"Kamera konumu Y":"Camera position Y"}<button type="button" class="slider-reset" aria-label={language==="tr"?"Kamera Y konumunu sıfırla":"Reset camera Y"} onclick={(event)=>{event.preventDefault();setToolNumber("freecam_y",2)}}>↺</button></span><input aria-label={language==="tr"?"Kamera konumu Y":"Camera position Y"} type="range" min="0" max="100" step="1" value={toolNumber("freecam_y")} oninput={(event)=>setToolNumber("freecam_y",Number(event.currentTarget.value))}></label>
                     <label><span class="slider-title">{language==="tr"?"Kamera boyutu":"Camera size"}<button type="button" class="slider-reset" aria-label={language==="tr"?"Kamera boyutunu sıfırla":"Reset camera size"} onclick={(event)=>{event.preventDefault();setToolNumber("freecam_size",77)}}>↺</button></span><input aria-label={language==="tr"?"Kamera boyutu":"Camera size"} type="range" min="15" max="90" step="1" value={toolNumber("freecam_size")} oninput={(event)=>setToolNumber("freecam_size",Number(event.currentTarget.value))}><small>{toolNumber("freecam_size").toFixed(0)}%</small></label>
                     <button class="center-content" onclick={()=>{setToolNumber("freecam_x",50);setToolNumber("freecam_y",2);setToolNumber("freecam_size",77)}}>{language==="tr"?"KAMERAYI SIFIRLA":"RESET CAMERA"}</button>
                  {/if}
                  <div class="clipper-watermark-controls">
                    <label class="watermark-toggle"><input type="checkbox" checked={toolValue("watermark_enabled")==="true"} onchange={(event)=>{setToolValue("watermark_enabled",event.currentTarget.checked?"true":"false");if(event.currentTarget.checked)clipperPreviewMode="output"}}><span>Watermark</span></label>
                    {#if toolValue("watermark_enabled")==="true"}
                      {#if watermarkLayer}<WatermarkControls layer={watermarkLayer} fonts={systemFonts} presets={textPresets} width={toolNumber("output_width")||1080} {language} onchange={changeWatermark} onfont={chooseWatermarkFont} onpreset={applyWatermarkPreset}/>{:else}<small>{language==="tr"?"Font hazırlanıyor…":"Preparing font…"}</small><button class="ghost" disabled={watermarkLoading} onclick={prepareWatermark}>{language==="tr"?"Tekrar dene":"Retry"}</button>{/if}
                    {/if}
                  </div>
                  <div class="clipper-watermark-controls">
                    <label class="watermark-toggle"><input type="checkbox" checked={toolValue("social_tag_enabled")==="true"} onchange={(event)=>setToolValue("social_tag_enabled",event.currentTarget.checked?"true":"false")}><span>Social Tag</span></label>
                    {#if toolValue("social_tag_enabled")==="true"}
                      <label class="watermark-text-field"><span>{language==="tr"?"Platform":"Platform"}</span><select value={toolValue("social_tag_platform")} onchange={(event)=>setToolValue("social_tag_platform",event.currentTarget.value)}><option value="kick">Kick</option><option value="twitch" disabled={toolValue("social_tag_style")==="kick_banner"}>Twitch</option></select></label>
                      <label class="watermark-text-field"><span>{language==="tr"?"Kullanıcı adı":"Username"}</span><input type="text" maxlength="32" placeholder="kanaladi" value={toolValue("social_tag_username")} oninput={(event)=>setToolValue("social_tag_username",event.currentTarget.value)}></label>
                      <label class="watermark-text-field"><span>{language==="tr"?"Görünüm":"Style"}</span><select value={toolValue("social_tag_style")} onchange={(event)=>{const value=event.currentTarget.value;setToolValue("social_tag_style",value);if(value==="kick_banner"){setToolValue("social_tag_platform","kick");setToolNumber("social_tag_size",54)}}}><option value="boxed">{language==="tr"?"Kutulu etiket":"Boxed badge"}</option><option value="plain">{language==="tr"?"Düz etiket":"Plain tag"}</option><option value="kick_banner">{language==="tr"?"Kick.com şeridi":"Kick.com banner"}</option></select></label>
                      {#if toolValue("social_tag_style")==="plain"}
                        <label class="watermark-text-field"><span>{language==="tr"?"Konum":"Position"}</span><select value={toolValue("social_tag_plain_position")} onchange={(event)=>setToolValue("social_tag_plain_position",event.currentTarget.value)}><option value="left">{language==="tr"?"Sol":"Left"}</option><option value="center">{language==="tr"?"Orta":"Center"}</option><option value="right">{language==="tr"?"Sağ":"Right"}</option></select></label>
                      {/if}
                       <label class="watermark-slider"><span>{language==="tr"?"Boyut":"Size"}<small>{toolNumber("social_tag_size").toFixed(0)} px</small></span><input type="range" min="20" max={toolValue("social_tag_style")==="kick_banner"?54:96} step="1" value={toolNumber("social_tag_size")} oninput={(event)=>setToolNumber("social_tag_size",Number(event.currentTarget.value))}></label>
                       <label class="watermark-slider"><span class="slider-title"><span>{toolValue("social_tag_style")==="kick_banner"?(language==="tr"?"Şeridi kameradan uzaklaştır":"Move banner away from camera"):(language==="tr"?"Etiketi kameradan uzaklaştır":"Move tag away from camera")}</span><span class="slider-end"><small>{toolNumber("social_tag_seam_offset").toFixed(0)}%</small><button type="button" class="slider-reset" aria-label={language==="tr"?"Etiket mesafesini sıfırla":"Reset tag distance"} title={language==="tr"?"Varsayılan: 0%":"Default: 0%"} onclick={()=>setToolNumber("social_tag_seam_offset",0)}>↺</button></span></span><input aria-label={toolValue("social_tag_style")==="kick_banner"?(language==="tr"?"Şeridi kameradan uzaklaştır":"Move banner away from camera"):(language==="tr"?"Etiketi kameradan uzaklaştır":"Move tag away from camera")} type="range" min="0" max="100" step="1" value={toolNumber("social_tag_seam_offset")} oninput={(event)=>setToolNumber("social_tag_seam_offset",Number(event.currentTarget.value))}></label>
                    {/if}
                  </div>
                  {#if toolValue("vertical_layout")==="split"}
                    <details class="text-style-options clipper-advanced">
                      <summary>{language==="tr"?"GELİŞMİŞ":"ADVANCED"}</summary>
                      <label><span>{language==="tr"?"Sıralama":"Order"}</span><select value={toolValue("region_order")} onchange={(event)=>setSplitOrder(event.currentTarget.value)}><option value="a_first">{language==="tr"?"Kamera üstte":"Camera above content"}</option><option value="b_first">{language==="tr"?"İçerik üstte":"Content above camera"}</option></select></label>
                      <label><span>{language==="tr"?"Üst bölüm yüksekliği":"Top region height"}</span><input type="range" min="20" max="80" step="1" value={toolNumber("region_a_height")} oninput={(event)=>setSplitHeight(Number(event.currentTarget.value))}><small>{toolNumber("region_a_height").toFixed(0)}%</small></label>
                      <p>{language==="tr"?"Kamera ve içerik kutularını kaynak önizleme üzerinde sürükleyip kenarlarından boyutlandır.":"Drag Camera and Content on the source preview and resize them from their edges."}</p>
                    </details>
                  {/if}
                </section>
              {/if}
              {#if selected.id==="transform"}
              <section>
                <header><b>{language==="tr"?"KIRP":"CROP"}</b><small>{toolValue("crop_mode")==="off" ? (language==="tr"?"kapalı":"off") : `${toolNumber("crop_w").toFixed(1)}% × ${toolNumber("crop_h").toFixed(1)}%`}</small></header>
                <div class="transform-options crop-options">
                  {#each transformPresets.filter(preset=>(media?.kind==="image"||!["5:4","3:4"].includes(preset))&&!(media?.kind==="image"&&toolValue("fit_mode")==="contain"&&preset==="free")) as preset}<button class:active={toolValue("crop_mode")===preset} onclick={()=>setCropPreset(preset)}>{preset==="off"?(media?.kind==="image"?(language==="tr"?"ORİJİNAL":"ORIGINAL"):(language==="tr"?"KAPALI":"OFF")):preset==="free"?(language==="tr"?"SERBEST":"FREE"):preset==="191:100"?"1.91:1":preset.toUpperCase()}</button>{/each}
                </div>
                {#if toolValue("crop_mode")!=="off" && toolValue("fit_mode")!=="contain"}<p>{language==="tr"?"Kadrajı önizlemede sürükle; kenar ve köşelerden serbestçe boyutlandır.":"Drag the frame in the preview; resize freely from its edges and corners."}</p>{/if}
                {#if media.kind === "image"}
                  <details class="text-style-options image-fit-options"><summary>{language==="tr"?"Sığdır / Tuval":"Fit / Canvas"}</summary>
                    <div class="transform-options two"><button class:active={toolValue("fit_mode")==="crop"} onclick={()=>setToolValue("fit_mode","crop")}>{language==="tr"?"Kırp / doldur":"Crop / fill"}</button><button class:active={toolValue("fit_mode")==="contain"} onclick={()=>{setToolValue("fit_mode","contain");if(toolValue("crop_mode")==="free")setCropPreset("off")}}>{language==="tr"?"Sığdır / tuval":"Fit / contain"}</button></div>
                    {#if toolValue("fit_mode")==="contain" && toolValue("crop_mode")!=="off"}<label class="field"><span>{language==="tr"?"Tuval arka planı":"Canvas background"}</span><select value={toolValue("canvas_background")} onchange={(event)=>setToolValue("canvas_background",event.currentTarget.value)}><option value="transparent">{language==="tr"?"Şeffaf":"Transparent"}</option><option value="black">{language==="tr"?"Siyah":"Black"}</option><option value="white">{language==="tr"?"Beyaz":"White"}</option><option value="custom">{language==="tr"?"Özel renk":"Custom color"}</option></select></label>{/if}
                    {#if toolValue("fit_mode")==="contain" && toolValue("canvas_background")==="custom"}<label class="field"><span>{language==="tr"?"Arka plan rengi":"Background color"}</span><input type="text" maxlength="7" value={toolValue("canvas_color")} oninput={(event)=>setToolValue("canvas_color",event.currentTarget.value)}></label>{/if}
                  </details>
                {/if}
              </section>
              <section>
                <header><b>{language==="tr"?"DÖNDÜR":"ROTATE"}</b><small>{toolValue("rotate")}°</small></header>
                <div class="transform-options four"><button class:active={toolValue("rotate")==="0"} onclick={()=>setTransformRotation(0)}>0°</button><button onclick={()=>rotateTransform(-90)}>↶ 90°</button><button onclick={()=>rotateTransform(90)}>↷ 90°</button><button onclick={()=>rotateTransform(180)}>180°</button></div>
              </section>
              <section>
                <header><b>{language==="tr"?"ÇEVİR":"FLIP"}</b></header>
                <div class="transform-options two"><button class:active={toolValue("flip_h")==="true"} onclick={()=>setToolValue("flip_h",toolValue("flip_h")==="true"?"false":"true")}>↔ {language==="tr"?"Yatay":"Horizontal"}</button><button class:active={toolValue("flip_v")==="true"} onclick={()=>setToolValue("flip_v",toolValue("flip_v")==="true"?"false":"true")}>↕ {language==="tr"?"Dikey":"Vertical"}</button></div>
              </section>
              <section>
                <header><b>{language==="tr"?"ÇIKTI BOYUTU":"OUTPUT SIZE"}</b></header>
                <div class="transform-options two"><button class:active={toolValue("size_mode")==="source"} onclick={()=>setToolValue("size_mode","source")}>{language==="tr"?"Kırpılan boyutu koru":"Keep crop size"}</button><button class:active={toolValue("size_mode")==="height"} onclick={()=>setToolValue("size_mode","height")}>{language==="tr"?"Yükseklik":"Height"}</button><button class:active={toolValue("size_mode")==="width"} onclick={()=>setToolValue("size_mode","width")}>{language==="tr"?"Genişlik":"Width"}</button><button class:active={toolValue("size_mode")==="exact"} onclick={()=>setToolValue("size_mode","exact")}>{language==="tr"?"Tam boyut":"Exact"}</button></div>
                {#if ["height","width"].includes(toolValue("size_mode"))}
                  <label><span>{toolValue("size_mode")==="height"?(language==="tr"?"Hedef yükseklik":"Target height"):(language==="tr"?"Hedef genişlik":"Target width")}</span><div class="size-entry"><select value={String(toolNumber("size"))} onchange={(event)=>setToolNumber("size",Number(event.currentTarget.value))}>{#each [480,720,1080,1440,2160,4320] as size}<option value={size}>{size}px</option>{/each}</select><input aria-label={language==="tr"?"Özel çıktı boyutu":"Custom output size"} type="number" min="2" max="7680" step="2" value={toolNumber("size")} oninput={(event)=>setToolNumber("size",Number(event.currentTarget.value))}></div></label>
                {:else if toolValue("size_mode")==="exact"}
                  <div class="exact-size"><label><span>{language==="tr"?"Genişlik":"Width"}</span><input type="number" min="2" max="7680" step="2" value={toolNumber("output_width")} oninput={(event)=>setToolNumber("output_width",Number(event.currentTarget.value))}></label><b>×</b><label><span>{language==="tr"?"Yükseklik":"Height"}</span><input type="number" min="2" max="7680" step="2" value={toolNumber("output_height")} oninput={(event)=>setToolNumber("output_height",Number(event.currentTarget.value))}></label></div>
                  <p>{media.kind==="image"?(language==="tr"?"Görsel esnetilmeden bu tuvale sığdırılır; boş alanlar şeffaf kalır.":"The image is fitted into this canvas without stretching; unused space remains transparent."):(language==="tr"?"Tam boyut, seçtiğin kadrajı bu ölçülere ölçekler; oranlar farklıysa görüntü esneyebilir.":"Exact size scales the crop to these dimensions; mismatched ratios may stretch the image.")}</p>
                {/if}
              </section>
              {#if media.kind === "image"}
                <section>
                  <header><b>{language==="tr"?"ÇIKTI FORMATI":"OUTPUT FORMAT"}</b></header>
                  <div class="transform-options three"><button class:active={toolValue("format")==="png"} onclick={()=>setToolValue("format","png")}>{language==="tr"?"PNG · KAYIPSIZ":"PNG · LOSSLESS"}</button><button class:active={toolValue("format")==="webp"} onclick={()=>setToolValue("format","webp")}>WEBP</button><button class:active={toolValue("format")==="jpg"} onclick={()=>setToolValue("format","jpg")}>JPEG</button><button class:active={toolValue("format")==="bmp"} onclick={()=>setToolValue("format","bmp")}>BMP</button><button class:active={toolValue("format")==="tiff"} onclick={()=>setToolValue("format","tiff")}>TIFF</button><button class:active={toolValue("format")==="avif"} onclick={()=>setToolValue("format","avif")}>AVIF</button></div>
                  {#if toolValue("format")==="jpg"}<label class="field"><span>{language==="tr"?"Şeffaf alan rengi":"Transparent area color"}</span><input type="text" maxlength="7" value={toolValue("jpeg_background")} oninput={(event)=>setToolValue("jpeg_background",event.currentTarget.value)}></label>{/if}
                </section>
              {/if}
              {/if}
            </div>
          {/if}
            {#if selected.id === "compression"}
              <div class="quality-mode-switch"><button class:active={!qualityAdvanced} onclick={()=>setQualityMode(false)}>{language==="tr"?"Basit":"Simple"}</button><button class:active={qualityAdvanced} onclick={()=>setQualityMode(true)}>{language==="tr"?"Gelişmiş":"Advanced"}</button></div>
              {#if !qualityAdvanced}<div class="quality-profiles">{#each [["high",language==="tr"?"Yüksek kalite":"High quality","CRF 16"],["balanced",language==="tr"?"Dengeli":"Balanced","CRF 20"],["small",language==="tr"?"Küçük dosya":"Small file","CRF 24"]] as profile}<button class:active={toolValue("goal")===profile[0]} onclick={()=>applyQualityProfile(profile[0] as "high"|"balanced"|"small")}><b>{profile[1]}</b><small>{profile[2]}</small></button>{/each}</div>{/if}
            {/if}
            {#if selected.id==="frame_extractor"}
              <div class="frame-extractor-simple">
                {#if toolValue("mode")==="burst"}
                  <p>{language==="tr"?"Çıkarma başlayacak:":"Extraction starts at:"} <b class="mono">{playerTime(toolboxCurrent)}</b><br/><small>{language==="tr"?"Başlangıcı oynatıcıdan değiştir.":"Move the playhead to choose the start."}</small></p>
                  <label class="field"><span>{language==="tr"?"Çıkarılacak kare sayısı":"Number of frames to extract"}</span><input type="number" min="1" max="1000" step="1" value={toolValue("count")} oninput={event=>setToolValue("count",event.currentTarget.value)}/></label>
                  <label class="field"><span>{language==="tr"?"Kareler arası süre":"Distance between frames"}<small>ms</small></span><input type="number" min="1" max="3600000" step="1" value={Math.round(toolNumber("interval")*1000)} oninput={event=>setToolValue("interval",String(Number(event.currentTarget.value)/1000))}/></label>
                  <p>{language==="tr"?`${toolValue("count")} kare, ${Math.round(toolNumber("interval")*1000)} ms aralıkla. Örnek: 10 kare × 100 ms, ilk kareden son kareye yaklaşık 0,9 saniyeyi kapsar.`:`${toolValue("count")} frames, ${Math.round(toolNumber("interval")*1000)} ms apart. Example: 10 frames × 100 ms covers about 0.9 seconds from first to last.`}</p>
                  <small>{language==="tr"?"PNG, videonun kendi çözünürlüğünde. Video biterse daha az kare çıkar. Aralıklar mevcut video karelerine yuvarlanır; aynı kare tekrarlanmaz.":"PNG at the video's original resolution. Fewer frames are extracted if the video ends. Intervals align to available video frames; frames are not duplicated."}</small>
                {/if}
                <button class="ghost" onclick={()=>{frameAdvanced=!frameAdvanced;if(!frameAdvanced)setToolValue("mode","burst")}}>{frameAdvanced?(language==="tr"?"Basit moda dön":"Back to simple mode"):(language==="tr"?"Gelişmiş seçenekler":"Advanced options")}</button>
              </div>
            {/if}
            {#each selected.fields as field}
              {#if !fieldLivesOnTimeline(field.key) && fieldVisible(field.key)}
              <label class="field">
                <span>{field.label}{#if field.unit}<small>{field.unit}</small>{/if}</span>
                {#if field.type === "select"}
                  <select bind:value={field.value}>{#each field.options ?? [] as option}<option value={option.value}>{option.label}</option>{/each}</select>
                {:else if field.type === "file"}
                  <button class="file-field" onclick={() => selectFieldFile(field)}>{field.value ? basename(String(field.value)) : t("choose")}</button>
                {:else}
                  {#if field.type === "number" && numericPresets(selected.id, field).length}
                    <div class="number-choice">
                      <select value={numberIsCustom(selected.id,field) ? "__custom__" : String(Number(field.value))} onchange={(event)=>chooseNumberPreset(selected!.id,field,event.currentTarget.value)}>
                        {#each numericPresets(selected.id,field) as preset}<option value={String(preset)}>{preset}{field.unit ? ` ${field.unit}` : ""}</option>{/each}
                        <option value="__custom__">{t("custom")}</option>
                      </select>
                      {#if numberIsCustom(selected.id,field)}<input aria-label={`Custom ${field.label}`} type="number" bind:value={field.value} min={field.min} max={field.max} step={field.step} />{/if}
                    </div>
                  {:else}
                    <input type={field.type === "text" ? "text" : "number"} bind:value={field.value} min={field.min} max={field.max} step={field.step} />
                  {/if}
                {/if}
                {#if field.hint}<small class="hint">{field.hint}</small>{/if}
              </label>
              {/if}
            {/each}
            {#if selected.id === "compression" && toolValue("mode") !== "bitrate"}
              <div class="quality-guide">
                <h4>{language === "tr" ? "CRF VE AKILLI ANALİZ" : "CRF & SMART ANALYSIS"}</h4>
                <p>{language === "tr" ? "CRF 0 gerçek kayıpsızdır fakat çok büyük dosya üretir. CRF 16 çok yüksek kalitedir; 17 de kayıpsız değildir. Değer yükseldikçe dosya küçülür ve kalite kademeli azalır. VMAF analizi kaynak videoya uygun değeri ölçer." : "CRF 0 is truly lossless but creates a very large file. CRF 16 is very high quality; 17 is not lossless either. Higher values reduce size and gradually reduce quality. VMAF can measure a suitable value for this source."}</p>
                <ul>
                  <li><b>95–100</b><span>{language === "tr" ? "Neredeyse kayıpsız görünür." : "Looks nearly transparent."}</span></li>
                  <li><b>90–94</b><span>{language === "tr" ? "Çoğu kullanım için çok iyi." : "Very good for most uses."}</span></li>
                  <li><b>85–89</b><span>{language === "tr" ? "İyi; hareket ve dokuda fark çıkabilir." : "Good; motion and textures may differ."}</span></li>
                  <li><b>&lt;85</b><span>{language === "tr" ? "Kalite kaybı belirginleşir." : "Quality loss becomes obvious."}</span></li>
                </ul>
                <small>{language === "tr" ? "Hız için örnekler en fazla 720p ölçülür. Boyut tahmini yalnızca video akışıdır; ses ve kapsayıcı birkaç MB ekleyebilir." : "For speed, samples are measured at up to 720p. The size estimate covers video only; audio and container overhead may add a few MB."}</small>
                <button class="analyze-video" onclick={analyzeCompression} disabled={qualityAnalyzing||busy}>{qualityAnalyzing?(language==="tr"?"Analiz ediliyor…":"Analyzing…"):(language==="tr"?"Videoyu analiz et":"Analyze Video")}</button>
              </div>
              {#if qualityAnalysis}
                <div class="quality-result">
                  <div class="quality-verdict"><span>{language === "tr" ? "ÖNERİLEN" : "RECOMMENDED"}</span><b>CRF {qualityAnalysis.recommended_crf}</b><small>{language === "tr" ? `Hedef VMAF ${qualityAnalysis.target_vmaf.toFixed(0)} · ${qualityAnalysis.sample_count} bölgeden ${qualityAnalysis.sampled_seconds.toFixed(1)} sn incelendi` : `Target VMAF ${qualityAnalysis.target_vmaf.toFixed(0)} · ${qualityAnalysis.sampled_seconds.toFixed(1)} sec across ${qualityAnalysis.sample_count} regions`}</small></div>
                  <div class="quality-table">
                    <div class="quality-table-head"><span>CRF</span><span>VMAF</span><span>{language === "tr" ? "TAHMİN" : "ESTIMATE"}</span></div>
                    {#each qualityAnalysis.candidates as candidate}
                      <article class:chosen={candidate.crf === qualityAnalysis.recommended_crf}>
                        <b>{candidate.crf}</b><strong>{candidate.vmaf.toFixed(1)}</strong><span>~{candidate.estimated_size_mb.toFixed(1)} MB</span>
                        <small>{qualityRating(candidate.rating)}</small>
                      </article>
                    {/each}
                  </div>
                  <p>{language === "tr" ? `CRF ${qualityAnalysis.recommended_crf}, seçtiğin kalite hedefi için önerilir. Bu bir tahmindir; kesin dosya boyutu sahnelere göre değişebilir.` : `CRF ${qualityAnalysis.recommended_crf} is recommended for the selected quality goal. This is an estimate; final size may vary by scene.`}</p>
                  <button class="apply-quality" onclick={applyQualityRecommendation}>{language==="tr"?`Öneriyi uygula · CRF ${qualityAnalysis.recommended_crf}`:`Apply Recommendation · CRF ${qualityAnalysis.recommended_crf}`}</button>
                </div>
              {/if}
            {/if}
            {#if selected.id === "file_hash" && hashResult}
              <div class="quality-result">
                <div class="quality-verdict"><span>SHA-256</span><code>{hashResult}</code></div>
                <button class="ghost" onclick={() => navigator.clipboard.writeText(hashResult)}>{language === "tr" ? "özeti kopyala" : "copy hash"}</button>
              </div>
            {/if}
            {#if selected.id === "image_compressor"}
              {@const outputFormat=imageOutputFormat(media.path,toolValue("format"))}
              <section class="image-size-summary" aria-live="polite">
                <div><span>{language==="tr"?"Kaynak boyutu":"Source size"}</span><b>{formatBytes(media.size)}</b></div>
                <div><span>{toolValue("mode")==="target"?(language==="tr"?"Hedef boyut":"Target size"):(language==="tr"?"Tahmini çıktı":"Estimated output")}</span><b>{toolValue("mode")==="target"?formatBytes(toolNumber("target_kb")*1024):(compressionEstimateLoading?(language==="tr"?"Hesaplanıyor…":"Calculating…"):(compressionEstimate!=null?formatBytes(compressionEstimate):compressionEstimateError?(language==="tr"?"Hesaplanamadı":"Unavailable"):"—"))}</b></div>
                {#if toolValue("mode")==="target"}
                  {#if !imageTargetSupported(outputFormat)}<p class="image-format-warning">{language==="tr"?"Hedef boyut için WebP veya JPEG seç. Mevcut biçimde bu işlem desteklenmiyor.":"Choose WebP or JPEG for a target size. The current format does not support this mode."}</p><div class="image-format-actions"><button class="ghost" onclick={()=>setToolValue("format","webp")}>WebP</button><button class="ghost" onclick={()=>setToolValue("format","jpg")}>JPEG</button></div>{:else}<p>{language==="tr"?"Bu bir boyut hedefidir; en uygun kalite render sırasında aranır. Ulaşılamayan hedeflerde çıktı daha büyük olabilir.":"This is a size target, not an estimate. Rendering searches for the best fitting quality; an unattainable target may produce a larger file."}</p>{/if}
                {:else if !imageQualityAdjustable(outputFormat)}<p>{outputFormat==="png"?(language==="tr"?"PNG yerel olarak optimize edilir; kalite yüzdesi uygulanmaz.":"PNG is optimized locally; quality percentages do not apply."):(language==="tr"?"Bu biçim kayıpsızdır; kalite yüzdesi uygulanmaz.":"This format is lossless; quality percentages do not apply.")}</p>
                {:else}<p>{language==="tr"?"Boyut, seçilen ayarlarla yapılan gerçek deneme kodlamasından hesaplanır.":"Size is calculated from a trial encode using the selected settings."}</p>{/if}
                {#if outputFormat==="png"}<p>{toolValue("png_mode")==="palette"?(language==="tr"?"Renk paleti azaltılabilir; bu mod tamamen kayıpsız değildir. Ölçüler ve şeffaf alanlar korunur; yarı saydam renkler sınırlı değişebilir. 16-bit, animasyonlu veya 16 MP üzeri PNG kayıpsız optimize edilir.":"The palette may be reduced; this mode is not fully lossless. Dimensions and transparent areas are retained; translucent colours may change within the error limit. 16-bit, animated or over-16MP PNG stay lossless."):(language==="tr"?"Pikseller, şeffaflık ve renk profili aynen korunur. Hedefe ulaşılamazsa en küçük kayıpsız sonuç kaydedilir; daha fazla küçültmek için palet modunu seçebilirsin.":"Pixels, transparency and colour profile stay unchanged. If the target cannot be reached, the smallest lossless result is saved; palette mode can reduce it further.")}</p>{/if}
                {#if compressionEstimateError&&toolValue("mode")==="quality"}<button class="ghost" onclick={()=>compressionEstimateRetry++}>{language==="tr"?"Tekrar hesapla":"Retry estimate"}</button>{/if}
                {#if renderedImageSize}<div class="image-size-actual"><span>{language==="tr"?"Son çıktı":"Last output"}</span><b>{formatBytes(renderedImageSize)}</b></div>{/if}
                {#if outputFormat==="png"&&toolValue("mode")==="target"&&renderedImageSize&&renderedImageSize>toolNumber("target_kb")*1024}<p class="image-format-warning">{language==="tr"?"Hedef boyuta ulaşılamadı; kalite sınırları korunarak bulunan en küçük PNG kaydedildi.":"The target was not reached; the smallest PNG within the quality limits was saved."}</p>{/if}
              </section>
            {/if}
          </div>
          <div class="run-box" class:stack-run-box={media.kind==="video"&&processingStack.length>0}>
            {#if media.kind==="video"&&processingStack.length}
              <div class="stack-output-quality" role="group" aria-label={language==="tr"?"Son çıktı kalitesi":"Final output quality"}>
                <span>{language==="tr"?"Çıktı kalitesi":"Output quality"}</span>
                <div class="stack-quality-options">
                  {#each [{value:"high",label:language==="tr"?"Yüksek":"High"},{value:"medium",label:language==="tr"?"Orta":"Medium"},{value:"small",label:language==="tr"?"Küçük":"Small"},{value:"lossless",label:language==="tr"?"Kayıpsız":"Lossless"}] as quality}
                    <button class:active={stackQuality===quality.value} aria-pressed={stackQuality===quality.value} disabled={operationBusy} onclick={()=>stackQuality=quality.value}>{quality.label}</button>
                  {/each}
                </div>
              </div>
              <button class="run stack-render" aria-label={language==="tr"?"İşlem listesini renderla":"Render processing stack"} onclick={runProcessingStack} disabled={busy||qualityAnalyzing||!processingStack.some(step=>step.enabled)||stackDraftDirty()}><span>▶ {language==="tr"?"Listeyi renderla":"Render stack"}</span><small>{processingStack.filter(step=>step.enabled).length} {language==="tr"?"adım":"steps"}</small></button>
              <button class="stack-render-single" onclick={runTool} disabled={busy||qualityAnalyzing}>{language==="tr"?"Yalnızca açık aracı renderla":"Render current tool only"}</button>
            {:else}
              <button class="run" onclick={runTool} disabled={busy||qualityAnalyzing||(selected.id==="image_compressor"&&toolValue("mode")==="target"&&!imageTargetSupported(imageOutputFormat(media.path,toolValue("format"))))}>▶ {selected.id === "file_hash" ? (language === "tr" ? "SHA-256 hesapla" : "calculate SHA-256") : `${t("render")} ${selected.title.toLocaleLowerCase(language)}`}</button>
            {/if}
          </div>
        {:else}
          <div class="empty-settings"><span>←</span><p>{t("selectTool")}</p></div>
        {/if}
      </aside>
    </section>
    {/if}
  {/if}
  {#if dragActive}<div class="drop-overlay"><span>{t("dropOpen")}</span></div>{/if}
  {#if toastMessage}<aside class:error={toastKind==="error"} class:success={toastKind==="success"} class="app-toast" role="alert">{#if toastKind==="success"}<i class="toast-success-icon" aria-hidden="true">✓</i>{/if}<span>{toastMessage}</span><button onclick={()=>{toastMessage="";window.clearTimeout(toastTimer)}} aria-label={language==="tr"?"Bildirimi kapat":"Close notification"}>×</button></aside>{/if}
</main>
