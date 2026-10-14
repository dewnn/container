<script lang="ts">
  import "@fontsource-variable/geist";
  import "@fontsource-variable/geist-mono";
  import { onMount, tick, untrack } from "svelte";
  import { invoke, convertFileSrc, isTauri } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Image as TauriImage } from "@tauri-apps/api/image";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { confirm, open, save } from "@tauri-apps/plugin-dialog";
  import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { armCompletionSound, playCompletionSound } from "./lib/completionSound";
  import { AppUpdater } from "./lib/appUpdater.svelte";
  import { ImageView } from "./lib/imageView.svelte";
  import { TextPresetStore } from "./lib/textPresets.svelte";
  import { RecentFiles } from "./lib/recentFiles.svelte";
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
  import { moveTimelineBoundary, type TimelineBoundary } from "./lib/timelineRange";
  import { reportProblem, type ToastDetail } from "./lib/toast";
  import { friendlyProblem } from "./lib/problemMessages";
  import { sanitizeSessionParts } from "./lib/sessionValidation";
  import ProblemDetails from "./lib/ProblemDetails.svelte";
  import RenderFeedback from "./lib/RenderFeedback.svelte";
  import GeneralSettings from "./lib/GeneralSettings.svelte";
  import LandingScreen from "./lib/LandingScreen.svelte";
  import UpdateDialog from "./lib/dialogs/UpdateDialog.svelte";
  import ProjectFilesDialog from "./lib/dialogs/ProjectFilesDialog.svelte";
  import DependencyDialog from "./lib/dialogs/DependencyDialog.svelte";
  import OutputCleanupDialog from "./lib/dialogs/OutputCleanupDialog.svelte";
  import PanelResetDialog from "./lib/dialogs/PanelResetDialog.svelte";
  import ShortcutsDialog from "./lib/dialogs/ShortcutsDialog.svelte";
  import { isFormFieldTarget, keyOwnedByControl, playerAction } from "./lib/playerShortcuts";
  import {imageOutputFormat,imageQualityAdjustable,imageTargetSupported} from "./lib/imageCompressionUi";
  import {completionAlert} from "./lib/completionAlert";
  import type { MediaInfo, ProgressEvent, JobResult, QualityCandidate, QualityAnalysis, CameraDetectionResult, FfmpegStatus, FfmpegCapabilities, DownloaderStatus, SubtitleTrack, OutputCleanupResult, FontOption, TextLayer, StackStep, EditorSnapshot, RecoverySession } from "./lib/editorTypes";
  import WatermarkControls from "./lib/WatermarkControls.svelte";
  import ColorControls from "./lib/toolPanels/ColorControls.svelte";
  import SafeAreaOverlay from "./lib/toolPanels/SafeAreaOverlay.svelte";
  import PlayerControls from "./lib/toolPanels/PlayerControls.svelte";
  import ImageViewer from "./lib/toolPanels/ImageViewer.svelte";
  import SocialTagOverlay from "./lib/toolPanels/SocialTagOverlay.svelte";
  import TextLayerOverlay from "./lib/toolPanels/TextLayerOverlay.svelte";
  import ImageOverlayBox from "./lib/toolPanels/ImageOverlayBox.svelte";
  import BlurRegionBox from "./lib/toolPanels/BlurRegionBox.svelte";
  import ToolList from "./lib/toolPanels/ToolList.svelte";
  import ToolTimeline from "./lib/toolPanels/ToolTimeline.svelte";
  import JobPanel from "./lib/toolPanels/JobPanel.svelte";
  import ProcessingStackPanel from "./lib/toolPanels/ProcessingStackPanel.svelte";
  import MergeInputList from "./lib/toolPanels/MergeInputList.svelte";
  import FrameExtractorControls from "./lib/toolPanels/FrameExtractorControls.svelte";
  import QualityAnalysisPanel from "./lib/toolPanels/QualityAnalysisPanel.svelte";
  import ImageCompressorEstimate from "./lib/toolPanels/ImageCompressorEstimate.svelte";
  import RunBox from "./lib/toolPanels/RunBox.svelte";
  import TextLayersPanel from "./lib/toolPanels/TextLayersPanel.svelte";
  import ToolNotes from "./lib/toolPanels/ToolNotes.svelte";
  import ClipperControls from "./lib/toolPanels/ClipperControls.svelte";
  import TransformControls from "./lib/toolPanels/TransformControls.svelte";
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
  let shortcutsOpen = $state(false);
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
  const recentFiles = new RecentFiles();
  const presetStore = new TextPresetStore(() => language, message => showToast(message, "info"));
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
      if(!font)throw new Error(language==="tr"?"Watermark yazı tipi kullanılamıyor. Yüklü bir yazı tipi seç.":"Watermark font is unavailable. Choose an installed font.");
      const fontName=await loadPreviewFont(font);if(selected!==sourceTool||version!==watermarkSelectionVersion)return;
      const layer:TextAppearance=existing?{...existing,fontName}:{text:toolValue("watermark_text"),x:50,y:50,size:toolNumber("watermark_size")||32,color:"#ffffff",opacity:toolNumber("watermark_opacity")||85,align:"center",fontName,font_path:font.path,outline:0,outline_color:"#000000",shadow:0,shadow_color:"#000000",background:toolValue("watermark_background")==="true",background_color:"#000000",background_opacity:65,background_padding:12};
      setToolValue("watermark_layer",JSON.stringify(layer));watermarkFontReady=font.path;
    }catch(reason){reportProblem(reason)}finally{
      watermarkLoading=false;
      if(selected!==sourceTool&&selected?.id==="clipper"&&toolValue("watermark_enabled")==="true"&&(!watermarkLayer||watermarkFontReady!==watermarkLayer.font_path))void prepareWatermark();
    }
  }
  async function chooseWatermarkFont(path:string){const sourceTool=selected,version=++watermarkSelectionVersion;try{const font=(await ensureSystemFonts()).find(font=>font.path===path);if(!font)return;const fontName=await loadPreviewFont(font);if(selected===sourceTool&&version===watermarkSelectionVersion){changeWatermark({font_path:path,fontName});watermarkFontReady=path}}catch(reason){if(version===watermarkSelectionVersion)reportProblem(reason)}}
  async function applyWatermarkPreset(id:string){const preset=presetStore.find(id);if(!preset)return;const sourceTool=selected,version=++watermarkSelectionVersion;try{const font=(await ensureSystemFonts()).find(font=>font.path.toLowerCase()===preset.layer.font_path.toLowerCase());if(!font)throw new Error("Preset font is unavailable.");const fontName=await loadPreviewFont(font);if(selected!==sourceTool||version!==watermarkSelectionVersion)return;const scale=preset.sourceWidth?(toolNumber("output_width")||1080)/preset.sourceWidth:1;const layer={...preset.layer,font_path:font.path,fontName,size:preset.layer.size*scale,outline:preset.layer.outline*scale,shadow:preset.layer.shadow*scale,background_padding:preset.layer.background_padding*scale};setToolValue("watermark_layer",JSON.stringify(layer));setToolValue("watermark_text",layer.text);watermarkFontReady=font.path;clipperPreviewMode="output"}catch(reason){if(version===watermarkSelectionVersion)reportProblem(reason)}}
  $effect(()=>{if(selected?.id==="clipper"&&toolValue("watermark_enabled")==="true"&&(!watermarkLayer||watermarkFontReady!==watermarkLayer.font_path))untrack(()=>void prepareWatermark())});
  let activeTextId: number | null = $state(null);
  let systemFonts: FontOption[] = $state([]);
  let systemFontsLoad: Promise<FontOption[]> | null = null;
  const previewFontLoads = new Map<string,Promise<string>>();
  const fontSelectionVersions = new Map<number,number>();
  let textMeasureCanvas: HTMLCanvasElement | null = null;
  let qualityAdvanced = $state(localStorage.getItem("container-quality-mode")==="advanced");
  let nextTextId = 1;
  const imageView = new ImageView(() => media);
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
  const updater=new AppUpdater(()=>language);
  let outputCleanupOpen = $state(false);
  let outputCleaning = $state(false);
  let outputCleanupMessage = $state("");
  let outputCleanupMessageTimer:number|undefined;
  let toastMessage=$state("");
  let toastKind:"error"|"info"|"success"=$state("error");
  let toastTimer:number|undefined;
  // Snapshots are replaced, never mutated, so the list does not need deep
  // reactivity. Signatures are kept beside them to avoid re-serializing.
  let editHistory: EditorSnapshot[] = $state.raw([]);
  let editSignatures: string[] = [];
  let editHistoryIndex = $state(-1);
  let historyApplying = false;
  const messages:Record<"tr"|"en",Record<string,string>>={
    tr:{tagline:"FFMPEG MEDYA ARAÇLARI",close:"DOSYAYI KAPAT",drop:"dosyanı buraya bırak",browse:"ya da seçmek için tıkla",landingTitle:"tek yerde. tüm araçlar.",landingCopy:"Videonu, sesini veya görselini aç; ihtiyacın olan araçlar ve tüm ayarlar burada.",local:"işlemler cihazında yapılır",untouched:"kaynak dosyaların değişmez",tools:"ARAÇLAR",available:"araç",video:"video",audio:"ses",image:"görsel",search:"araç ara...",preview:"ÖNİZLEME",original:"ORİJİNAL",rendered:"İŞLENMİŞ",process:"İŞLEM",frame:"kare",speed:"hız",elapsed:"geçen süre",showOutput:"Klasörde göster",cancelJob:"İşlemi iptal et",parameters:"AYARLAR",defaults:"Varsayılanlar",what:"NE İŞE YARAR?",forVideo:"BU VİDEODA",choose:"dosya seç...",custom:"Özel…",render:"İŞLE",outputNote:"Çıktın İndirilenler/CONTAINER Output klasörüne kaydedilir. Kaynak dosyan değişmez.",selectTool:"Bir araç seç",dropOpen:"açmak için bırak",ready:"hazır",toolbox:"ARAÇ KUTUSU"},
    en:{tagline:"FFMPEG MEDIA TOOLBOX",close:"CLOSE FILE",drop:"drop media here",browse:"or click to browse files",landingTitle:"one place. every tool.",landingCopy:"All CONTAINER FFmpeg operations in one workspace with detailed controls and live progress.",local:"local processing only",untouched:"original files stay untouched",tools:"TOOLS",available:"available",video:"video",audio:"audio",image:"image",search:"search tools...",preview:"PREVIEW",original:"ORIGINAL",rendered:"RENDERED",process:"PROCESS",frame:"frame",speed:"speed",elapsed:"elapsed",showOutput:"Show in folder",cancelJob:"Cancel job",parameters:"PARAMETERS",defaults:"Defaults",what:"WHAT DOES IT DO?",forVideo:"FOR THIS VIDEO",choose:"choose file...",custom:"Custom…",render:"RENDER",outputNote:"Output is written to Downloads/CONTAINER Output. The source file is not changed.",selectTool:"Select a tool",dropOpen:"drop to open",ready:"ready",toolbox:"TOOLBOX"}
  };
  const t=(key:string)=>messages[language][key]??key;
  function showToast(reason:unknown,kind:"error"|"info"|"success"="error"){
    toastMessage=kind==="error"?friendlyProblem(reason,language):String(reason);
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
    return candidate.version===1&&typeof candidate.savedAt==="number"&&typeof candidate.mediaPath==="string"&&candidate.mediaPath.length>0&&["toolbox","autocut","batch"].includes(candidate.workspaceMode??"")&&sanitizeSessionParts(value as Record<string,unknown>);
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
  async function loadProjectPath(path:string):Promise<boolean>{
    const loadId=++mediaLoadId;
    const contents=await invoke<string>("read_project",{path});
    if(loadId!==mediaLoadId)return false;
    const saved=JSON.parse(contents);
    if(!validRecovery(saved))throw new Error(language==="tr"?"Geçersiz CONTAINER proje dosyası.":"Invalid CONTAINER project file.");
    recoveryCandidate=saved;return await restorePreviousSession()===true;
  }
  async function openIncomingPath(path:string){
    if(operationBusy)return;
    try{
      if(path.toLowerCase().endsWith(".cproj")){if(await loadProjectPath(path))recentFiles.add(path)}
      else if(path.toLowerCase().endsWith(".containerproject"))throw new Error(language==="tr"?"Bu eski proje biçimi artık desteklenmiyor. Yalnızca .cproj dosyaları açılabilir.":"This old project format is no longer supported. Only .cproj files can be opened.");
      else if(await loadMedia(path))recentFiles.add(path);
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

  function captureEditorState():{snapshot:EditorSnapshot;signature:string}|null{
    if(!media)return null;
    const signature=JSON.stringify({media,mediaUrl,selected,activeKind,output,outputSettingsKey,outputMode,renderedImageUrl,colorEnabled,colorPreviewVisible,textLayers,activeTextId,qualityAnalysis,customNumberFields,mergeInputs,processingStack,stackQuality,editingStackStepId});
    return {snapshot:JSON.parse(signature) as EditorSnapshot,signature};
  }
  function captureEditorSnapshot():EditorSnapshot|null{return captureEditorState()?.snapshot??null}
  function resetEditorHistory(){const state=captureEditorState();editHistory=state?[state.snapshot]:[];editSignatures=state?[state.signature]:[];editHistoryIndex=state?0:-1}
  function commitEditorSnapshot(snapshot:EditorSnapshot,signature=JSON.stringify(snapshot)){
    if(historyApplying)return;
    if(editHistoryIndex>=0&&editSignatures[editHistoryIndex]===signature)return;
    editHistory=[...editHistory.slice(0,editHistoryIndex+1),snapshot].slice(-80);
    editSignatures=[...editSignatures.slice(0,editHistoryIndex+1),signature].slice(-80);
    editHistoryIndex=editHistory.length-1;
  }
  function flushEditorSnapshot(){const state=captureEditorState();if(state)commitEditorSnapshot(state.snapshot,state.signature)}
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

  // One serialization per change: it tracks every editor value, feeds undo
  // history once the edit settles, and schedules the recovery save. Any
  // further change re-runs the effect and cancels both timers.
  $effect(()=>{
    const state=captureEditorState();
    if(!state||historyApplying)return;
    const saveRecovery=!restoringSession;
    workspaceMode;autoCutSession;batchSession;
    const historyTimer=window.setTimeout(()=>{if(!historyApplying)commitEditorSnapshot(state.snapshot,state.signature)},280);
    const recoveryTimer=saveRecovery?window.setTimeout(persistRecovery,500):0;
    return()=>{window.clearTimeout(historyTimer);window.clearTimeout(recoveryTimer)};
  });

  const categories = $derived.by(() => {
    const list = kindTools(activeKind).filter((tool) => (!favoritesOnly||favoriteIds.includes(tool.id))&&`${tool.title} ${tool.description}`.toLocaleLowerCase(language).includes(search.toLocaleLowerCase(language)));
    const map = new Map<string, Tool[]>();
    for (const tool of list) map.set(tool.category, [...(map.get(tool.category) ?? []), tool]);
    const entries=[...map.entries()];
    if(activeKind==="image")entries.sort(([left],[right])=>left==="Utilities"?1:right==="Utilities"?-1:0);
    return entries;
  });
  const otherKindMatch = $derived.by(() => {
    const query=search.trim().toLocaleLowerCase(language);
    if(!query||!media)return null;
    for(const kind of (["video","audio","image"] as MediaKind[])){
      if(kind===activeKind)continue;
      const tool=kindTools(kind).find(item=>`${item.title} ${item.description}`.toLocaleLowerCase(language).includes(query));
      if(tool)return {kind,tool,allowed:kind===media.kind||(media.kind==="video"&&kind==="audio")};
    }
    return null;
  });
  function openOtherKindMatch(){
    const match=otherKindMatch;
    if(!match?.allowed)return;
    switchKind(match.kind);
    const tool=kindTools(match.kind).find(item=>item.id===match.tool.id);
    if(tool)chooseTool(tool);
  }
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
    renderedImageUrl = "";
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
  const presetSourceWidth=()=>previewSourceDimensions().width||media?.width||undefined;
  function saveTextPreset(){presetStore.save(activeText(),presetSourceWidth())}
  function deleteTextPreset(){presetStore.remove()}
  function updateTextPreset(){presetStore.update(activeText(),presetSourceWidth())}
  function renameTextPreset(){presetStore.rename()}
  async function applyTextPreset(){
    const preset=presetStore.find(presetStore.selected);if(!preset||presetStore.busy)return;
    const sourceMedia=media;const sourceTool=selected;presetStore.busy=true;
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
    }catch(reason){reportProblem(reason)}finally{presetStore.busy=false}
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
    {title:"Color Adjustments",trTitle:"Renk ayarları",keys:["brightness","contrast","saturation","gamma"]},
    {title:"Tone",trTitle:"Ton",keys:["hue","temperature"]},
    {title:"Detail",trTitle:"Ayrıntı",keys:["sharpen","blur"]},
    {title:"Cleanup",trTitle:"Temizleme",keys:["deband"]},
    {title:"Style",trTitle:"Stil",keys:["vignette"]},
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
      imageView.clear();
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
    imageView.clear();
    toolboxFilmstripUrl="";toolboxFilmstripLoading=false;
    editHistory = [];
    editSignatures = [];
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
    if(!selected)throw new Error(language==="tr"?"Önce bir araç seç.":"Select a tool first.");
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
          if(!font)throw new Error(language==="tr"?"Watermark yazı tipi kullanılamıyor.":"Watermark font is unavailable.");
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
      if (fps <= media.fps || fps > 2400 || fps % 60 !== 0) return language==="tr"?`İnterpolasyon FPS değeri ${media.fps.toFixed(2)} değerinden yüksek, 60'ın katı ve en fazla 2400 olmalı.`:`Interpolation FPS must be above ${media.fps.toFixed(2)}, a multiple of 60 and at most 2400.`;
    }
    if (tool.id === "frame_blend" && media?.fps && Number(params.fps) >= media.fps) return language==="tr"?`Kare harmanlama hedefi ${media.fps.toFixed(2)} FPS değerinden düşük olmalı.`:`The Frame Blending target must be below ${media.fps.toFixed(2)} FPS.`;
    if (tool.id === "upscale" && media?.width && media?.height && Number(params.target_edge) <= Math.min(previewSourceDimensions().width,previewSourceDimensions().height)) return language==="tr"?"Bu kaynak için daha yüksek bir standart çözünürlük hedefi yok.":"There is no higher standard resolution target for this source.";
    if (["cut", "gif"].includes(tool.id) && Number(params.start) >= Number(params.end ?? Number(params.start) + Number(params.duration))) {
      if (tool.id === "cut") return language==="tr"?"Bitiş zamanı başlangıçtan büyük olmalı.":"End time must be after the start time.";
    }
    if (tool.id === "replace_audio" && !params.audio_path) return language==="tr"?"Önce yeni ses dosyasını seç.":"Choose the replacement audio file first.";
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
        if(watermarkLoading||watermarkFontReady!==watermarkLayer.font_path)throw new Error(language==="tr"?"Watermark yazı tipi hâlâ yükleniyor. Hazır olunca tekrar dene.":"Watermark font is still loading. Try again when it is ready.");
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
        imageView.compare = 50;
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

  const jobStatusTr:Record<string,string>={ready:"hazır",complete:"tamamlandı",failed:"başarısız",cancelled:"iptal edildi",cancelling:"iptal ediliyor",error:"hata","source missing":"kaynak bulunamadı","ffmpeg missing":"FFmpeg bulunamadı","probing media":"medya okunuyor"};
  function jobStatusLabel(value:string){
    if(language!=="tr")return value;
    if(value.startsWith("running · "))return `çalışıyor · ${value.slice("running · ".length)}`;
    return jobStatusTr[value]??value;
  }
  function recommendation(): string {
    if (!selected || !media) return "";
    if (selected.id === "noise") return media.height && media.height <= 720 ? (language==="tr"?"Güvenli 3 · Önerilen 6":"Safe 3 · Recommended 6") : (language==="tr"?"Güvenli 4 · Önerilen 8":"Safe 4 · Recommended 8");
    if (selected.id === "distortion") return language==="tr"?"Güvenli 1 · Önerilen 3":"Safe 1 · Recommended 3";
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

  onMount(() => {
    let disposed=false;
    const stopPremiere=watchPremiere();
    void document.fonts.load('32px "Gotham XNarrow Black"').then(()=>{if(!disposed)bannerFontReady=true}).catch(()=>{});
    presetStore.load();
    recentFiles.load();
    const saved=localStorage.getItem("container-language");
    language=saved==="tr"||saved==="en"?saved:navigator.language.toLowerCase().startsWith("tr")?"tr":"en";
    document.documentElement.lang=language;
    if(isTauri())void invoke("set_tray_language",{language}).catch(reportProblem);
    let unlistenTrayUpdate:UnlistenFn|undefined;
    if(isTauri())void listen("tray-check-updates",()=>{void updater.check(true)}).then(fn=>{if(disposed)fn();else unlistenTrayUpdate=fn});
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
    void updater.loadVersion();
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
      await updater.check(false);
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
      if (event.key === "?" && !editingText && !event.ctrlKey && !event.altKey) { event.preventDefault(); shortcutsOpen = true; return; }
      if (!media||restoringSession||stageNavigating) return;
      if (event.ctrlKey && !event.altKey && key === "z" && target instanceof HTMLInputElement && target.type === "range") {
        event.preventDefault();
        if (event.shiftKey) redoEditor(); else undoEditor();
        return;
      }
      if(isFormFieldTarget(target))return;
      if (event.ctrlKey && !event.altKey && event.key.toLowerCase() === "z") {
        event.preventDefault();
        if (event.shiftKey) redoEditor(); else undoEditor();
        return;
      }
      if (workspaceMode !== "toolbox" || media.kind !== "video" || keyOwnedByControl(event)) return;
      const action = playerAction(event, { marks: !stackResultPreview && rangeTimelineTool() });
      if (!action) return;
      event.preventDefault();
      if (action.kind === "toggle") toggleToolboxPlayer();
      else if (action.kind === "seek") seekToolboxBy(action.seconds);
      else if (action.kind === "frame") { toolboxVideo?.pause(); seekToolboxBy(action.direction / (media.fps || 30)); }
      else if (action.kind === "start") seekToolbox(0);
      else if (action.kind === "end") seekToolbox(playbackDuration);
      else markCutAtPlayhead(action.edge);
    };
    const blockBrowserMenu = (event: MouseEvent) => event.preventDefault();
    const toastEvent=(event:Event)=>{const detail=(event as CustomEvent<ToastDetail>).detail;if(detail?.message)showToast(detail.message,detail.kind??"error")};
    const browserError=(event:ErrorEvent)=>reportProblem(event.error??event.message);
    const rejected=(event:PromiseRejectionEvent)=>reportProblem(event.reason);
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
    {#if !media && updater.devVersion}<span class="dev-version mono" aria-label={`Development build version ${updater.devVersion}`}><span>DEV BUILD</span><b>v{updater.devVersion}</b></span>{/if}
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
      <nav class="mode-tabs" aria-label={language==="tr"?"Çalışma alanı":"Workspace"}>
        <button class:active={!downloaderOpen&&workspaceMode === "toolbox"} onclick={() => setWorkspaceMode("toolbox")} disabled={operationBusy&&workspaceMode!=="toolbox"}>{t("toolbox")}</button>
        {#if media.kind === "video"}<button class:active={!downloaderOpen&&workspaceMode === "autocut"} onclick={() => setWorkspaceMode("autocut")} disabled={operationBusy&&workspaceMode!=="autocut"}>SMARTCUT</button>{/if}
        <button class:active={!downloaderOpen&&workspaceMode === "batch"} onclick={() => setWorkspaceMode("batch")} disabled={operationBusy&&workspaceMode!=="batch"}>{language === "tr" ? "TOPLU" : "BATCH"}</button>
      </nav>
      <div class="project-actions"><button onclick={saveProject} disabled={operationBusy}>{language==="tr"?"PROJEYİ KAYDET":"SAVE PROJECT"}</button></div>
      <button class="ghost top-cancel" onclick={closeMedia} disabled={operationBusy}>{t("close")}</button>
    {:else}
      <div class="landing-header-actions">
        <div class="project-actions landing-project-actions"><button onclick={openProject} disabled={operationBusy}>{language==="tr"?"PROJE AÇ":"OPEN PROJECT"}</button></div>
        {#if updater.enabled}<button class="update-trigger" class:available={!!updater.available} class:checking={updater.checking} onclick={() => updater.check(true)} title={language === "tr" ? "Güncellemeleri denetle" : "Check for updates"}><b>↻</b><span>{updater.available ? `v${updater.available.version}` : (language === "tr" ? "GÜNCELLE" : "UPDATE")}</span>{#if updater.available}<i></i>{/if}</button>{/if}
      </div>
    {/if}
    <GeneralSettings {language} {theme} onlanguage={setLanguage} ontheme={setTheme} onresetlayout={media&&!downloaderOpen?()=>panelResetDialogOpen=true:undefined} oncleanoutput={!media&&!downloaderOpen?()=>{outputCleanupMessage="";outputCleanupOpen=true}:undefined} layoutBusy={operationBusy} onshortcuts={()=>shortcutsOpen=true}/>
  </header>

  {#if shortcutsOpen}
    <ShortcutsDialog {language} onclose={()=>shortcutsOpen=false}/>
  {/if}

  {#if panelResetDialogOpen}
    <PanelResetDialog {language} {workspaceMode} onclose={()=>panelResetDialogOpen=false} onconfirm={confirmResetPanelWidths}/>
  {/if}

  {#if updater.panelOpen}
    <UpdateDialog {language} appVersion={updater.version} availableUpdate={updater.available} status={updater.status} checking={updater.checking} installing={updater.installing} downloaded={updater.downloaded} total={updater.total} onclose={()=>updater.panelOpen=false} oncheck={()=>updater.check(true)} oninstall={()=>updater.install()}/>
  {/if}

  {#if projectFilesOpen}
    <ProjectFilesDialog {language} checks={projectFileChecks} onclose={()=>projectFilesOpen=false}/>
  {/if}

  {#if dependencyPanel && !updater.panelOpen}
    <DependencyDialog {language} migrationError={runtimeMigrationError} checking={dependencyChecking} onclose={()=>dependencyPanel=false} onrepair={repairFfmpegRuntime} oncheck={()=>refreshFfmpegStatus(true)}/>
  {/if}

  {#if !media && outputCleanupOpen}
    <OutputCleanupDialog {language} message={outputCleanupMessage} cleaning={outputCleaning} onclose={()=>outputCleanupOpen=false} onclean={cleanOutputFolder}/>
  {/if}

  {#if downloaderOpen}
    <DownloaderWorkspace {language} hasMedia={!!media} onback={()=>downloaderOpen=false} onbusychange={(value:boolean)=>downloaderBusy=value} onopenmedia={openDownloadedMedia} />
  {:else if !media}
    <LandingScreen {language} {t} recent={recentFiles} onopenrecent={openIncomingPath} {autoEncoderTuning} recovery={recoveryCandidate} restoring={restoringSession} {dragActive} {ffmpegStatus} {downloaderStatus} {dependencyChecking} cleanupMessage={outputCleanupMessage} ondiscard={discardRecovery} onrestore={restorePreviousSession} onselect={selectMedia} oncheck={()=>refreshFfmpegStatus(true)} ondownloader={()=>downloaderOpen=true}/>
  {:else}
    {#if workspaceMode === "autocut" && media.kind === "video"}
      <SmartCutWorkspace bind:this={autoCutWorkspace} {media} {mediaUrl} {language} onaddstack={addSmartCutStack} stackHasCuts={processingStack.some(step=>step.tool.id==="smartcut")} stackCount={processingStack.length} oncontinue={continueEditingOutput} onhistorychange={(undo:boolean,redo:boolean)=>{autoCutCanUndo=undo;autoCutCanRedo=redo}} onsessionchange={(value:unknown)=>{autoCutSession=value}} onbusychange={(value:boolean)=>autoCutBusy=value} />
    {:else if workspaceMode === "batch"}
      <BatchWorkspace bind:this={batchWorkspace} initialPaths={batchInitialPaths.length?batchInitialPaths:[media.path]} initialKind={media.kind} {language} {availableEncoders} oncontinue={continueEditingOutput} onhistorychange={(undo:boolean,redo:boolean)=>{batchCanUndo=undo;batchCanRedo=redo}} onsessionchange={(value:unknown)=>{batchSession=value}} onbusychange={(value:boolean)=>batchBusy=value} onqueuechange={(count:number)=>batchQueueCount=count} />
    {:else}
    {@const panelSizes=toolboxPanelSizes()}
    <section class="workspace resizable" bind:this={toolboxWorkspace} style={`--toolbox-left:${panelSizes.left}px;--toolbox-right:${panelSizes.right}px`}>
      <ToolList {language} {t} {activeKind} {categories} {chooseTool} {favoriteIds} bind:favoritesOnly {kindTools} {media} {openOtherKindMatch} {otherKindMatch} bind:search {selected} {switchKind} {toggleFavorite} {visibleFavoriteCount}/>

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
                {#if outputBox}<ClipperLayoutPreview video={toolboxVideo} input={clipperLayoutInput()} box={outputBox} {language} />{/if}
              {/if}
              {#if multiRegionClipper&&clipperPreviewMode==="source"}
                <div class="transform-source-box" bind:this={transformSourceBox} style={transformBoxStyle()}>
                  {#each [{id:"a" as const,label:language==="tr"?"KAMERA BÖLGESİ":"CAMERA REGION"},{id:"b" as const,label:language==="tr"?"İÇERİK BÖLGESİ":"CONTENT REGION"}] as region}
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
                {#if watermarkLayer&&watermarkFontReady===watermarkLayer.font_path}<WatermarkPreview layer={watermarkLayer} width={toolNumber("output_width")||1080} height={toolNumber("output_height")||1920} box={verticalOutputBox()} onchange={changeWatermark} {language}/>{:else}
                <i class="clipper-watermark-background" style={clipperWatermarkPreviewStyle(true)}></i>
                <span class="clipper-watermark-preview" style={clipperWatermarkPreviewStyle()}>{toolValue("watermark_text")}</span>
                {/if}
              {/if}
              {#if selected?.id==="clipper"&&(!multiRegionClipper||clipperOutputVisible)}<SafeAreaOverlay box={verticalOutputBox()} {language}/>{/if}
              <SocialTagOverlay {language} {clipperOutputVisible} {multiRegionClipper} {selected} {socialBannerPreviewStyle} {socialTagPreviewStyle} {toolValue}/>
              <TextLayerOverlay {language} {activeTextId} {selected} {startTextDrag} {textLayerStyle} {textLayers} bind:textPreviewCanvas {textPreviewCanvasStyle} {wrappedText}/>
              <ImageOverlayBox {language} bind:overlayPreviewImage {overlayPreviewStyle} {selected} {startOverlayDrag} {toolNumber} {toolValue} {toolboxCurrent}/>
              <BlurRegionBox {mediaBoxStyle} {selected} {startEffectRegion} {toolNumber} {toolValue} {transformHandles} bind:transformSourceBox/>
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
                      <i class="text-size-handle left" role="presentation" aria-label={language==="tr"?"Yazıyı soldan boyutlandır":"Resize text from left"} onpointerdown={(event)=>startTextDrag(event,layer,-1)}></i><span>{wrappedText(layer)}</span><i class="text-size-handle right" role="presentation" aria-label={language==="tr"?"Yazıyı sağdan boyutlandır":"Resize text from right"} onpointerdown={(event)=>startTextDrag(event,layer,1)}></i><i class="text-size-handle nw" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,-1,-1)}></i><i class="text-size-handle ne" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,1,-1)}></i><i class="text-size-handle sw" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,-1,1)}></i><i class="text-size-handle se" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,1,1)}></i>
                      </div>
                    {/each}
                  </div>
                {/if}
                {#if selected.id === "image_overlay" && toolValue("image_path")}
                  <button class="overlay-preview-box" style={overlayPreviewStyle()} onpointerdown={(event)=>startOverlayDrag(event)} aria-label={language==="tr"?"Kaplamayı taşı":"Move overlay"}>
                    <img bind:this={overlayPreviewImage} src={convertFileSrc(toolValue("image_path"))} alt="Overlay preview" draggable="false" style:opacity={toolNumber("opacity")/100} />
                  <i class="text-size-handle left" role="presentation" aria-label={language==="tr"?"Kaplamayı soldan boyutlandır":"Resize overlay from left"} onpointerdown={(event)=>startOverlayDrag(event,-1)}></i><i class="text-size-handle right" role="presentation" aria-label={language==="tr"?"Kaplamayı sağdan boyutlandır":"Resize overlay from right"} onpointerdown={(event)=>startOverlayDrag(event,1)}></i><i class="text-size-handle nw" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,-1,-1)}></i><i class="text-size-handle ne" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,1,-1)}></i><i class="text-size-handle sw" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,-1,1)}></i><i class="text-size-handle se" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,1,1)}></i>
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
              <ImageViewer {language} {t} view={imageView} {media} {mediaUrl} {renderedImageUrl} {outputStale}/>
            {/if}
            {#if media.kind==="video"}
              <PlayerControls {language} {fullscreenToolboxPlayer} {hoverPlayerSeek} {playbackDuration} bind:playerSeekHover {playerTime} {precisionPlayerSeek} {seekToolbox} {seekToolboxBy} {toggleToolboxPlayer} {toolboxCurrent} {toolboxPlaying} {toolboxVideo} bind:toolboxVolume {wheelPlayerSeek}/>
            {/if}
          </div>
        </div>

        {#if timelineTool && media.duration}
          <ToolTimeline {language} {commitCutTime} bind:cutEndInput bind:cutStartInput bind:cutTimeEditing {editableTime} {handleCutTimeKey} {hoverTimeline} {markCutAtPlayhead} duration={media.duration} {playerTime} {rangeTimelineTool} {seekTimeline} {selected} {setScreenshotTime} {startToolTimelineDrag} {timelineBounds} {timelineHandleKey} bind:timelineHover {timelineTime} {toolNumber} {toolboxCurrent} {toolboxFilmstripLoading} {toolboxFilmstripUrl} bind:toolboxTimeline/>
        {/if}

        <JobPanel {language} {t} {busy} {cancelJob} {continueEditingOutput} {elapsed} {error} {frame} {jobStatus} {jobStatusLabel} {operationBusy} {output} {outputStale} {playRenderedOutput} {progress} {showToast} {speed}/>
      </section>

      <div class="workspace-resizer" role="slider" tabindex="0" aria-label={language==="tr"?"Parametreler panelinin genişliği":"Parameters panel width"} aria-orientation="horizontal" aria-valuemin={panelSizes.minRight} aria-valuemax={Math.min(620,panelSizes.available-panelSizes.left-panelSizes.minCenter)} aria-valuenow={Math.round(panelSizes.right)} onpointerdown={(event)=>startToolboxPanelResize(event,"right")} onkeydown={(event)=>toolboxPanelKey(event,"right")} ondblclick={resetToolboxPanelWidths} title={language==="tr"?"Genişliği sürükle · sıfırla: çift tık":"Drag to resize · double-click to reset"}></div>

      <aside class="settings panel" class:merge-compact={selected?.id==="merge_videos"} class:compact-controls={panelSizes.right<300}>
        {#if selected}
          <div class="pane-head tool-settings-head"><div><h2>{selected.title}</h2><p>{selected.description}</p></div><button class="reset" onclick={resetSelectedTool}>{t("defaults")}</button></div>
          {#if media.kind==="video"}
            <ProcessingStackPanel {language} {addOrUpdateStackStep} {editingStackStepId} {flushEditorSnapshot} {moveStackStep} {operationBusy} {persistRecovery} {processingStack} {removeStackStep} {selectStackStep} {selected} {stackDraftDirty} {stackToolIds}/>
          {/if}
          {#if !["transform","clipper","cut","text","color","merge_videos"].includes(selected.id)}<div class="explain"><b>{t("what")}</b><p>{selected.id==="fix_timestamps"&&media.kind==="audio"?(language==="tr"?"Hızlı onarım, sesi kalite kaybı olmadan yeniden paketler. Derin onarım sesi FLAC olarak yeniden kodlar; yalnızca hızlı yöntem yetmezse kullan.":"Fast Repair remuxes audio without quality loss. Deep Repair re-encodes audio as FLAC; use it only when the fast method is not enough."):selected.detail}</p></div>{/if}
          {#if selected.id === "merge_videos"}
            <MergeInputList {language} {addMergeVideos} {basename} {mergeInputs} {moveMergeVideo} {removeMergeVideo}/>
          {/if}
          {#if selected.id === "subtitles" && toolValue("action")==="extract" && !subtitleTracks.length}
            <div class="codec-note"><b>{language==="tr"?"ALTYAZI YOK":"NO SUBTITLES"}</b><span>{language==="tr"?"Bu dosyada çıkarılabilir gömülü altyazı bulunamadı.":"No extractable embedded subtitle track was found in this file."}</span></div>
          {/if}
          {#if recommendation()}<div class="recommend"><b>{t("forVideo")}</b><span>{recommendation()}</span></div>{/if}
          <div class="field-list">
          {#if selected.id === "color"}
            <ColorControls {language} {adjustColor} {applyColorPreset} {colorGroups} {colorLabels} bind:colorPreviewVisible {colorValueLabel} {displayedColorValue} {rangePercent} {resetColorFilters} {resetColorKey} {setToolValue} {toolField} {toolValue}/>
          {:else if selected.id === "text"}
            <TextLayersPanel {language} {activeText} bind:activeTextId {addTextLayer} {applyTextPreset} {chooseTextFont} {deleteTextPreset} {positionText} {rangePercent} {removeTextLayer} {renameTextPreset} {resizeTextLayer} {saveTextPreset} bind:selectedTextPreset={presetStore.selected} {setTextColor} {systemFonts} {textColors} {textLayers} textPresetBusy={presetStore.busy} bind:textPresetName={presetStore.name} textPresets={presetStore.list} {updateTextLayer} {updateTextPreset}/>
          {/if}
          <ToolNotes {language} {media} {selected} {discordBudget} {encoderQualityMode} {toolNumber} {upscaleDimensions}/>
          {#if ["transform","clipper"].includes(selected.id)}
            <div class="transform-controls">
              {#if selected.id==="clipper"}
                <ClipperControls {language} {applyWatermarkPreset} {autoDetectCamera} {busy} {cameraDetecting} {cameraDetectionMessage} {centerContentRegion} {changeWatermark} {chooseWatermarkFont} bind:clipperPreviewMode {contentTargetDimensions} bind:freecamLayoutBox {freecamPlacement} {prepareWatermark} {setSplitHeight} {setSplitOrder} {setToolNumber} {setToolValue} {setVerticalLayout} {startFreecamPlacement} {systemFonts} textPresets={presetStore.list} {toolNumber} {toolValue} {watermarkLayer} {watermarkLoading}/>
              {/if}
              {#if selected.id==="transform"}
              <TransformControls {language} {media} {rotateTransform} {setCropPreset} {setToolNumber} {setToolValue} {setTransformRotation} {toolNumber} {toolValue} {transformPresets}/>
              {/if}
            </div>
          {/if}
            {#if selected.id === "compression"}
              <div class="quality-mode-switch"><button class:active={!qualityAdvanced} onclick={()=>setQualityMode(false)}>{language==="tr"?"Basit":"Simple"}</button><button class:active={qualityAdvanced} onclick={()=>setQualityMode(true)}>{language==="tr"?"Gelişmiş":"Advanced"}</button></div>
              {#if !qualityAdvanced}<div class="quality-profiles">{#each [["high",language==="tr"?"Yüksek kalite":"High quality","CRF 16"],["balanced",language==="tr"?"Dengeli":"Balanced","CRF 20"],["small",language==="tr"?"Küçük dosya":"Small file","CRF 24"]] as profile}<button class:active={toolValue("goal")===profile[0]} onclick={()=>applyQualityProfile(profile[0] as "high"|"balanced"|"small")}><b>{profile[1]}</b><small>{profile[2]}</small></button>{/each}</div>{/if}
            {/if}
            {#if selected.id==="frame_extractor"}
              <FrameExtractorControls {language} bind:frameAdvanced {playerTime} {setToolValue} {toolNumber} {toolValue} {toolboxCurrent}/>
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
              <QualityAnalysisPanel {language} {analyzeCompression} {applyQualityRecommendation} {busy} {qualityAnalysis} {qualityAnalyzing} {qualityRating} {selected}/>
            {/if}
            {#if selected.id === "file_hash" && hashResult}
              <div class="quality-result">
                <div class="quality-verdict"><span>SHA-256</span><code>{hashResult}</code></div>
                <button class="ghost" onclick={() => navigator.clipboard.writeText(hashResult)}>{language === "tr" ? "özeti kopyala" : "copy hash"}</button>
              </div>
            {/if}
            {#if selected.id === "image_compressor"}
              <ImageCompressorEstimate {language} {compressionEstimate} {compressionEstimateError} {compressionEstimateLoading} bind:compressionEstimateRetry {formatBytes} {media} {renderedImageSize} {setToolValue} {toolNumber} {toolValue}/>
            {/if}
          </div>
          <RunBox {language} {t} {busy} {media} {operationBusy} {processingStack} {qualityAnalyzing} {runProcessingStack} {runTool} {selected} {stackDraftDirty} bind:stackQuality {toolValue}/>
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
