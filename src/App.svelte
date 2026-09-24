<script lang="ts">
  import "@fontsource-variable/geist";
  import "@fontsource-variable/geist-mono";
  import { onMount, tick } from "svelte";
  import { invoke, convertFileSrc, isTauri } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Image as TauriImage } from "@tauri-apps/api/image";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { confirm, open, save } from "@tauri-apps/plugin-dialog";
  import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { armCompletionSound, playCompletionSound } from "./lib/completionSound";
  import { check, Update } from "@tauri-apps/plugin-updater";
  import { localizedForSection, localizedTool, preserveToolValues, type Field, type MediaKind, type Tool } from "./lib/tools";
  import { isTextEditingTarget } from "./lib/editorInput";
  import { startupAction } from "./lib/startupRecovery";
  import { projectResources, replaceProjectResource, type ProjectResource } from "./lib/projectResources";
  import { socialTagGeometry } from "./lib/socialTagGeometry";
  import AutoCutWorkspace from "./lib/AutoCutWorkspace.svelte";
  import BatchWorkspace from "./lib/BatchWorkspace.svelte";
  import StageHistoryControl from "./lib/StageHistory.svelte";
  import { toolSummary, toolCategory } from "./lib/toolSummaries";
  import { startStages, checkpointStage, continueStage, validStages, type StageHistory } from "./lib/stageHistory";
  import DownloaderWorkspace from "./lib/DownloaderWorkspace.svelte";
  import { recoveredMediaUrl } from "./lib/recovery";
  import { updatesAllowedForVersion } from "./lib/releaseChannel";
  import { moveTimelineBoundary, type TimelineBoundary } from "./lib/timelineRange";
  import { reportProblem, type ToastDetail } from "./lib/toast";
  import kickMark from "./assets/kick-mark.svg";
  import twitchMark from "./assets/twitch-mark.svg";

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
  interface ProgressEvent { percent: number; time: number; speed: string; frame: string; status: string }
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
  interface EditorSnapshot { media:MediaInfo; mediaUrl:string; selected:Tool|null; activeKind:MediaKind; output:string; outputSettingsKey?:string; renderedImageUrl:string; colorEnabled:Record<string,boolean>; colorPreviewVisible:boolean; textLayers:TextLayer[]; activeTextId:number|null; qualityAnalysis:QualityAnalysis|null; customNumberFields:Record<string,boolean>; mergeInputs?:string[] }
  interface RecoverySession { version:1; savedAt:number; mediaPath:string; workspaceMode:"toolbox"|"autocut"|"batch"; toolbox:EditorSnapshot|null; autocut:unknown; batch:unknown; resources?:ProjectResource[]; stageHistory?:StageHistory<RecoverySession> }
  let stageHistory:StageHistory<RecoverySession>|null=$state(null);
  let stageNavigating=$state(false);

  const mediaDialogFilters=[{name:"Media",extensions:["mp4","mov","mkv","avi","webm","m4v","mp3","wav","m4a","aac","flac","opus","jpg","jpeg","png","webp","bmp","tif","tiff","avif","heic","heif"]}];

  let media: MediaInfo | null = $state(null);
  let mediaUrl = $state("");
  let selected: Tool | null = $state(null);
  let activeKind: MediaKind = $state("video");
  let busy = $state(false);
  let dragActive = $state(false);
  let error = $state("");
  let output = $state("");
  let outputSettingsKey=$state("");
  const outputStale=$derived(!!output&&outputSettingsKey!==renderSettingsKey());
  function renderSettingsKey(){return media&&selected?JSON.stringify({source:media.path,operation:selected.id,params:paramsFrom(selected)}):""}
  let progress = $state(0);
  let jobStatus = $state("ready");
  let speed = $state("—");
  let frame = $state("—");
  let elapsed = $state(0);
  let search = $state("");
  let favoriteIds:string[]=$state([]);
  let favoritesOnly=$state(false);
  let workspaceMode: "toolbox" | "autocut" | "batch" = $state("toolbox");
  let downloaderOpen = $state(false);
  let downloaderBusy = $state(false);
  let autoCutBusy=$state(false),batchBusy=$state(false);
  let autoCutWorkspace:{undo:()=>void;redo:()=>void;exportSession:()=>unknown;restoreSession:(value:any)=>void;resetPanelWidths:()=>void}|null=$state(null);
  let batchWorkspace:{undo:()=>void;redo:()=>void;exportSession:()=>unknown;restoreSession:(value:any)=>void;resetPanelWidths:()=>void}|null=$state(null);
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
  let transformBackdropVideo: HTMLVideoElement | null = $state(null);
  let toolboxStage: HTMLElement | null = $state(null);
  let toolboxCanvas: HTMLElement | null = $state(null);
  let textPreviewCanvas: HTMLCanvasElement | null = $state(null);
  let transformCanvasWidth = $state(0);
  let transformCanvasHeight = $state(0);
  let toolboxMetadataVersion=$state(0);
  let transformSourceBox: HTMLElement | null = $state(null);
  let freecamLayoutBox: HTMLElement | null = $state(null);
  let toolboxCurrent = $state(0);
  let toolboxPlaying = $state(false);
  let toolboxVolume = $state(1);
  let renderedImageUrl = $state("");
  let renderedImageSize = $state(0);
  let temporaryImagePreviewPath = "";
  let compressionEstimate = $state<number|null>(null);
  let compressionEstimateLoading = $state(false);
  let compressionEstimateId = 0;
  let qualityAnalysis: QualityAnalysis | null = $state(null);
  let qualityAnalyzing = $state(false);
  let cameraDetecting = $state(false);
  let cameraDetectionMessage = $state("");
  let hashResult = $state("");
  let colorEnabled: Record<string,boolean> = $state({});
  let colorPreviewVisible = $state(true);
  let textLayers: TextLayer[] = $state([]);
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
  let availableUpdate: Update | null = $state(null);
  let updatePanel = $state(false);
  let outputCleanupOpen = $state(false);
  let outputCleaning = $state(false);
  let outputCleanupMessage = $state("");
  let outputCleanupMessageTimer:number|undefined;
  let toastMessage=$state("");
  let toastKind:"error"|"info"=$state("error");
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
    tr:{tagline:"FFMPEG MEDYA ARAÇ KUTUSU",close:"kapat",drop:"medyayı buraya bırak",browse:"veya dosya seçmek için tıkla",landingTitle:"tek dosya. bütün araçlar.",landingCopy:"CONTAINER’ın bütün FFmpeg işlemleri, ayrıntılı ayarlar ve canlı ilerleme bilgisiyle tek çalışma alanında.",local:"yalnızca yerel işlem",untouched:"orijinal dosyalar değişmez",tools:"ARAÇLAR",available:"mevcut",video:"video",audio:"ses",image:"görsel",search:"araçlarda ara...",preview:"ÖNİZLEME",original:"ORİJİNAL",rendered:"İŞLENMİŞ",process:"İŞLEM",frame:"kare",speed:"hız",elapsed:"geçen",showOutput:"çıktıyı göster",cancelJob:"işlemi iptal et",parameters:"PARAMETRELER",defaults:"varsayılanlar",what:"NE YAPAR?",forVideo:"BU VİDEO İÇİN",choose:"dosya seç...",custom:"Özel…",render:"işle",outputNote:"Çıktı Downloads/CONTAINER Output klasörüne yazılır. Kaynak dosya değiştirilmez.",selectTool:"Bir araç seç",dropOpen:"açmak için bırak",ready:"hazır",toolbox:"ARAÇ KUTUSU"},
    en:{tagline:"FFMPEG MEDIA TOOLBOX",close:"close",drop:"drop media here",browse:"or click to browse files",landingTitle:"one file. every tool.",landingCopy:"All CONTAINER FFmpeg operations in one workspace with detailed controls and live progress.",local:"local processing only",untouched:"original files stay untouched",tools:"TOOLS",available:"available",video:"video",audio:"audio",image:"image",search:"search tools...",preview:"PREVIEW",original:"ORIGINAL",rendered:"RENDERED",process:"PROCESS",frame:"frame",speed:"speed",elapsed:"elapsed",showOutput:"show output",cancelJob:"cancel job",parameters:"PARAMETERS",defaults:"defaults",what:"WHAT DOES IT DO?",forVideo:"FOR THIS VIDEO",choose:"choose file...",custom:"Custom…",render:"render",outputNote:"Output is written to Downloads/CONTAINER Output. The source file is not changed.",selectTool:"Select a tool",dropOpen:"drop to open",ready:"ready",toolbox:"TOOLBOX"}
  };
  const t=(key:string)=>messages[language][key]??key;
  function friendlyProblem(reason:unknown){
    const raw=String(reason??"").trim();
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
  function showToast(reason:unknown,kind:"error"|"info"="error"){
    toastMessage=kind==="error"?friendlyProblem(reason):String(reason);
    toastKind=kind;
    window.clearTimeout(toastTimer);
    toastTimer=window.setTimeout(()=>toastMessage="",3000);
  }
  const kindTools=(kind:MediaKind)=>localizedForSection(kind,media?.kind??kind,language).filter(tool=>{
    if(!ffmpegCapabilities)return true;
    if(tool.id==="stabilizer")return ffmpegCapabilities.vidstab;
    if(tool.id==="image_overlay")return ffmpegCapabilities.overlay;
    if(tool.id==="blur_pixelate")return ffmpegCapabilities.blur&&ffmpegCapabilities.overlay;
    if(tool.id==="merge_videos")return ffmpegCapabilities.concat;
    return true;
  });
  const timelineTool = $derived.by(()=>media?.kind==="video"&&selected ? ["cut","screenshot","gif","image_overlay"].includes(selected.id) : false);
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
    const path=await save({defaultPath:`${media?.name.replace(/\.[^.]+$/,"")||"project"}.containerproject`,filters:[{name:"CONTAINER Project",extensions:["containerproject"]}]});
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
    const saved=JSON.parse(await invoke<string>("read_project",{path}));
    if(!validRecovery(saved))throw new Error(language==="tr"?"Geçersiz CONTAINER proje dosyası.":"Invalid CONTAINER project file.");
    recoveryCandidate=saved;await restorePreviousSession();
  }
  async function openIncomingPath(path:string){
    if(operationBusy)return;
    try{
      if(path.toLowerCase().endsWith(".containerproject"))await loadProjectPath(path);
      else await loadMedia(path);
    }catch(reason){reportProblem(reason)}
  }
  async function openProject(){
    if(operationBusy)return;
    const path=await open({multiple:false,filters:[{name:"CONTAINER Project",extensions:["containerproject"]}]});if(typeof path!=="string")return;
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
          transformBackdropVideo?.load();
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
    return cloneEditorValue({media,mediaUrl,selected,activeKind,output,outputSettingsKey,renderedImageUrl,colorEnabled,colorPreviewVisible,textLayers,activeTextId,qualityAnalysis,customNumberFields,mergeInputs});
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
    historyApplying=true;
    toolboxVideo?.pause();
    if(!preserveLoadedMedia){media=cloneEditorValue(snapshot.media);mediaUrl=snapshot.mediaUrl}
    activeKind=snapshot.activeKind;
    selected=restoreToolSnapshot(snapshot.selected);
    output=snapshot.output;outputSettingsKey=snapshot.outputSettingsKey??"";renderedImageUrl=snapshot.renderedImageUrl;colorEnabled=cloneEditorValue(snapshot.colorEnabled);colorPreviewVisible=snapshot.colorPreviewVisible;textLayers=cloneEditorValue(snapshot.textLayers);activeTextId=snapshot.activeTextId;qualityAnalysis=cloneEditorValue(snapshot.qualityAnalysis);customNumberFields=cloneEditorValue(snapshot.customNumberFields);mergeInputs=cloneEditorValue(snapshot.mergeInputs??(media?[media.path]:[]));toolboxPlaying=false;toolboxCurrent=0;error="";jobStatus=language==="tr"?(direction==="undo"?"geri alındı":"ileri alındı"):(direction==="undo"?"undone":"redone");
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
    if(seconds===null||seconds<0||seconds>duration){error=language==="tr"?"Geçerli bir zaman gir (S, M:S veya H:M:S).":"Enter a valid time (S, M:S or H:M:S).";return}
    setTimelineBoundary(key,seconds);seekToolbox(seconds);error="";
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
      if(endField)endField.value=Math.round(end*1000)/1000;
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
    const changed = selected?.id !== tool.id;
    if(changed){
      colorEnabled={};
      colorPreviewVisible=true;
      textLayers=[];
      activeTextId=null;
    }
    selected = localizedTool(tool,language);
    if(selected.id==="clipper"){setCropPreset("9:16",true);centerContentRegion()}
    if(selected.id==="merge_videos"&&media)mergeInputs=[media.path];
    if(selected.id==="subtitles"&&media)void loadSubtitleTracks();
    if(selected.id==="text")void ensureSystemFonts();
    configureUpscale(selected);
    configureTimelineFields(selected);
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
    if(source){selected=localizedTool(source,language);configureTimelineFields(selected);if(selected.id==="clipper"){setCropPreset("9:16",true);centerContentRegion()}}
    colorEnabled={};colorPreviewVisible=true;textLayers=[];activeTextId=null;qualityAnalysis=null;error="";
  }
  function toolField(key:string){return selected?.fields.find(field=>field.key===key)}
  function fieldLivesOnTimeline(key:string){return ["cut","gif","image_overlay"].includes(selected?.id??"")?["start","end"].includes(key):selected?.id==="screenshot"?key==="timestamp":false}
  function fieldVisible(key:string){
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
      if(key==="quality")return toolValue("mode")==="quality";
      if(key==="target_kb")return toolValue("mode")==="target";
      if(key==="jpeg_background")return toolValue("format")==="jpg";
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
  function setToolNumber(key:string,value:number){const field=toolField(key);if(field)field.value=Math.round(value*1000)/1000}
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
  function toggleColor(key:string){colorEnabled={...colorEnabled,[key]:!colorEnabled[key]}}
  function resetColorKey(key:string){
    const source=kindTools(activeKind).find(tool=>tool.id==="color")?.fields.find(field=>field.key===key);
    if(source)setToolNumber(key,Number(source.value));
  }
  function colorValueLabel(key:string){const value=toolNumber(key);return key==="temperature"?`${value} K`:key==="hue"?`${value}°`:`${value}%`}
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
    if(selected?.id==="clipper"&&["original","blur","fill"].includes(toolValue("vertical_layout"))){
      const box=verticalOutputBox();if(!box)return "";
      const background=toolValue("canvas_background"),color=background==="white"?"#fff":background==="custom"?toolValue("canvas_color"):"#000";
      if(toolValue("vertical_layout")==="fill"){
        const maxX=Math.max(0,100-toolNumber("crop_w")),maxY=Math.max(0,100-toolNumber("crop_h"));
        const positionX=maxX?toolNumber("crop_x")/maxX*100:50,positionY=maxY?toolNumber("crop_y")/maxY*100:50;
        return `position:absolute;z-index:2;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;object-fit:cover;object-position:${positionX}% ${positionY}%;background:#000`;
      }
      const foregroundBackground=toolValue("vertical_layout")==="blur"?"transparent":color;
      return `position:absolute;z-index:2;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;object-fit:contain;background:${foregroundBackground}`;
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
  function verticalBackdropStyle(){
    const box=verticalOutputBox();if(!box)return "display:none";
    return `position:absolute;z-index:1;pointer-events:none;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;object-fit:cover;filter:blur(${Math.max(10,box.width*.045)}px);clip-path:inset(0)`;
  }
  function syncTransformBackdrop(force=false){
    if(!toolboxVideo||!transformBackdropVideo)return;
    if(force||Math.abs(transformBackdropVideo.currentTime-toolboxVideo.currentTime)>.12)transformBackdropVideo.currentTime=toolboxVideo.currentTime;
  }

  function activeText(){return textLayers.find(layer=>layer.id===activeTextId)??null}
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
  function textLayerAvailableWidth(layer:TextLayer){
    const sourceWidth=previewSourceDimensions().width||media?.width||1,x=Math.max(0,Math.min(1,layer.x/100));
    const anchorWidth=layer.align==="left"?1-x:layer.align==="right"?x:2*Math.min(x,1-x);
    const safeWidth=sourceWidth*Math.min(.9,Math.max(.05,anchorWidth));
    const effects=(layer.background?layer.background_padding*2:0)+layer.outline+Math.max(0,layer.shadow);
    return layer.wrap_width?Math.max(layer.size,layer.wrap_width*layer.size):Math.max(layer.size,safeWidth-effects);
  }
  function wrappedText(layer:TextLayer){
    textMeasureCanvas??=document.createElement("canvas");
    const context=textMeasureCanvas.getContext("2d");
    if(!context)return layer.text;
    context.font=`400 ${layer.size}px ${JSON.stringify(layer.fontName)}, "Segoe UI Emoji", sans-serif`;
    const maxWidth=textLayerAvailableWidth(layer),lines:string[]=[];
    const fits=(value:string)=>context.measureText(value).width<=maxWidth;
    const splitLongWord=(word:string)=>{
      let part="";
      for(const character of Array.from(word)){
        const candidate=part+character;
        if(part&&!fits(candidate)){lines.push(part);part=character}else part=candidate;
      }
      return part;
    };
    for(const paragraph of layer.text.replace(/\r\n?/g,"\n").split("\n")){
      if(!paragraph){lines.push("");continue}
      let line="";
      for(const word of paragraph.trim().split(/\s+/)){
        const candidate=line?`${line} ${word}`:word;
        if(fits(candidate)){line=candidate;continue}
        if(line){lines.push(line);line=""}
        line=fits(word)?word:splitLongWord(word);
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
    if(canvas.width!==width)canvas.width=width;if(canvas.height!==height)canvas.height=height;
    const context=canvas.getContext("2d");if(!context)return;
    context.clearRect(0,0,width,height);
    for(const layer of textLayers){
      const lines=wrappedText(layer).split("\n"),lineHeight=layer.size*1.05,totalHeight=lineHeight*lines.length;
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
    if(layers.every((layer,index)=>textLayers[index]?.id===layer.id)){
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
  function trackEditorPointer(event:PointerEvent,move:(event:PointerEvent)=>void){
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
    cameraDetectionMessage="";
    setToolValue("vertical_layout",layout);
    setCropPreset("9:16",true);
    if(layout!=="fill"){
      setToolNumber("crop_x",0);setToolNumber("crop_y",0);setToolNumber("crop_w",100);setToolNumber("crop_h",100);
      if(toolValue("canvas_background")==="transparent")setToolValue("canvas_background","black");
    }
    if(["split","squares","freecam"].includes(layout))centerContentRegion();
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
    const startX=event.clientX,startY=event.clientY,initialX=toolNumber("crop_x"),initialY=toolNumber("crop_y"),width=toolNumber("crop_w"),height=toolNumber("crop_h");
    const move=(moveEvent:PointerEvent)=>{
      const x=Math.max(0,Math.min(100-width,initialX-(moveEvent.clientX-startX)/box.width*width));
      const y=Math.max(0,Math.min(100-height,initialY-(moveEvent.clientY-startY)/box.height*height));
      setToolNumber("crop_x",x);setToolNumber("crop_y",y);
    };
    trackEditorPointer(event,move);
  }
  function freecamPlacement(){
    const source=previewSourceDimensions(),regionWidth=Math.max(1,source.width*toolNumber("region_a_w")/100),regionHeight=Math.max(1,source.height*toolNumber("region_a_h")/100);
    const width=Math.max(15,Math.min(90,toolNumber("freecam_size"))),height=Math.min(90,width*(9/16)/(regionWidth/regionHeight));
    return {width,height,left:(100-width)*toolNumber("freecam_x")/100,top:(100-height)*toolNumber("freecam_y")/100};
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
  function socialTagPreviewStyle(){
    const box=verticalOutputBox();if(!box)return "display:none";
    const width=Math.max(2,toolNumber("output_width")||1080),height=Math.max(2,toolNumber("output_height")||1920);
    const boxed=toolValue("social_tag_style")==="boxed";
    const position=toolValue(boxed?"social_tag_boxed_position":"social_tag_plain_position") as "left"|"center"|"right";
    const username=toolValue("social_tag_username").trim();
    const geometry=socialTagGeometry({width,height,sourceWidth:media?.width??1920,sourceHeight:media?.height??1080,layout:toolValue("vertical_layout"),style:boxed?"boxed":"plain",position,username,size:toolNumber("social_tag_size"),textUnits:socialTagTextUnits(username),regionAHeight:toolNumber("region_a_height"),regionOrder:toolValue("region_order"),regionAWidth:toolNumber("region_a_w"),regionARegionHeight:toolNumber("region_a_h"),freecamSize:toolNumber("freecam_size"),freecamX:toolNumber("freecam_x"),freecamY:toolNumber("freecam_y")});
    const scaleX=box.width/width,scaleY=box.height/height;
    let left=box.left+geometry.anchorX*scaleX;
    let top=box.top+geometry.centerY*scaleY;
    let boundsLeft=box.left,boundsRight=box.left+box.width;
    // The source preview shows the camera crop at its original position, not
    // inside the virtual 9:16 output box. Project both tag styles onto it.
    const layout=toolValue("vertical_layout");
    if(["split","squares","freecam"].includes(layout)){
      const source=transformDisplayBox();
      if(source){
        const cameraLeft=(source.stageWidth-source.width)/2+source.width*toolNumber("region_a_x")/100;
        const cameraTop=(source.stageHeight-source.height)/2+source.height*toolNumber("region_a_y")/100;
        const cameraWidth=source.width*toolNumber("region_a_w")/100;
        const cameraHeight=source.height*toolNumber("region_a_h")/100;
        const even=(value:number)=>Math.floor(Math.round(value)/2)*2;
        const outputCameraWidth=layout==="freecam"?even(width*toolNumber("freecam_size")/100):width;
        const outputCameraLeft=layout==="freecam"?(width-outputCameraWidth)*toolNumber("freecam_x")/100:0;
        const relativeX=(geometry.anchorX-outputCameraLeft)/Math.max(1,outputCameraWidth);
        left=cameraLeft+relativeX*cameraWidth;
        const seamAtCameraTop=layout==="split"&&toolValue("region_order")==="b_first";
        top=boxed?cameraTop+cameraHeight-geometry.side*scaleX/2:cameraTop+(seamAtCameraTop?0:cameraHeight);
        boundsLeft=boxed?cameraLeft:0;
        boundsRight=boxed?cameraLeft+cameraWidth:source.stageWidth;
      }
    }
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
    if(mode===workspaceMode||operationBusy)return;
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
    const path = await open({
      multiple: false,
      filters: mediaDialogFilters,
    });
    if (typeof path === "string") await loadMedia(path);
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
      mergeInputs=[];
      activeKind = media.kind;
      mediaUrl = nextMediaUrl;
      output = "";
      if(previousPreview&&previousPreview!==preparedPreview)releaseTemporaryImagePreview(previousPreview);
      workspaceMode = "toolbox";
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
    stageHistory=null;
  }

  function stageLabel(){return workspaceMode==="autocut"?"SmartCut":workspaceMode==="batch"?"Batch":selected?.title??media?.name??"Media"}
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
    const duration = toolboxVideo.duration || media?.duration || 0;
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
    const time=(media?.duration??0)*valuePercent/100;
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
    syncTransformBackdrop(true);
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
      const units=socialTagTextUnits(params.social_tag_username.trim());
      if(units!==undefined)params.social_tag_text_units=units.toFixed(5);
    }
    if(tool.id==="merge_videos")params.inputs=JSON.stringify(mergeInputs);
    if(tool.id==="color")for(const key of ["brightness","contrast","saturation","gamma","hue","temperature","sharpen","blur","deband","vignette"])params[`${key}_enabled`]=String(colorOn(key));
    if(tool.id==="text")params.layers=JSON.stringify(textLayers.map(layer=>({...layer,text:wrappedText(layer)})));
    return params;
  }
  function hasEmojiText(){return textLayers.some(layer=>/[\p{Extended_Pictographic}\p{Regional_Indicator}\p{Emoji_Modifier}\u20e3\ufe0f]/u.test(layer.text))}

  $effect(()=>{
    const path=media?.path,id=selected?.id,mode=toolValue("mode"),format=toolValue("format"),quality=toolNumber("quality"),background=toolValue("jpeg_background");
    const requestId=++compressionEstimateId;
    compressionEstimate=null;
    compressionEstimateLoading=false;
    if(!path||id!=="image_compressor"||mode!=="quality"||busy)return;
    compressionEstimateLoading=true;
    const timer=window.setTimeout(async()=>{
      try{
        const size=await invoke<number>("estimate_image_compression",{request:{input:temporaryImagePreviewPath||path,operation:"image_compressor",params:{mode,format,quality:String(quality),target_kb:"1",jpeg_background:background||"#ffffff"}}});
        if(requestId===compressionEstimateId)compressionEstimate=size;
      }catch{
        if(requestId===compressionEstimateId)compressionEstimate=null;
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
    try{
      const result=await invoke<QualityAnalysis>("analyze_quality",{request:{input:analyzedPath,goal:toolValue("goal")||"balanced",sample_duration:toolNumber("sample_duration")||2}});
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
    const validation = validate(selected);
    if (validation) { error = validation; return; }
    armCompletionSound();
    busy = true;
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
      if(selected.id==="text"&&hasEmojiText()){
        renderTextPreview();
        if(!textPreviewCanvas)throw new Error(language==="tr"?"Emoji çıktısı için yazı önizlemesi hazır değil.":"Text preview is not ready for emoji export.");
        operationParams.text_raster_png=textPreviewCanvas.toDataURL("image/png");
      }
      const operationInput=temporaryImagePreviewPath||media.path;
      if(temporaryImagePreviewPath)operationParams.__source_path=media.path;
      const result = await invoke<JobResult>("run_operation", {
        request: { input: operationInput, operation: selected.id, params: operationParams },
      });
      output = result.output;
      outputSettingsKey=renderedSettingsKey;
      if (media.kind === "image") {
        renderedImageUrl = `${convertFileSrc(result.output)}?render=${Date.now()}`;
        renderedImageSize = (await invoke<MediaInfo>("probe_media",{path:result.output})).size;
        imageCompare = 50;
      }
      elapsed = result.elapsed;
      progress = 100;
      jobStatus = "complete";
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
    await invoke("cancel_job");
    jobStatus = "cancelling";
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
    const saved=localStorage.getItem("container-language");
    language=saved==="tr"||saved==="en"?saved:navigator.language.toLowerCase().startsWith("tr")?"tr":"en";
    document.documentElement.lang=language;
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
      if(editingText)return;
      if (event.ctrlKey && !event.altKey && event.key.toLowerCase() === "z") {
        event.preventDefault();
        if (event.shiftKey) redoEditor(); else undoEditor();
        return;
      }
      if (workspaceMode !== "toolbox") return;
      if (media.kind !== "video" || event.ctrlKey || event.altKey || event.metaKey) return;
      if (event.code === "Space") { event.preventDefault(); toggleToolboxPlayer(); }
      else if (rangeTimelineTool() && key === "i") { event.preventDefault(); markCutAtPlayhead("start"); }
      else if (rangeTimelineTool() && key === "o") { event.preventDefault(); markCutAtPlayhead("end"); }
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
    let disposed=false;
    listen<ProgressEvent>("container-progress", (event) => {
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
        const path = event.payload.paths[0];
        if (path) void openIncomingPath(path);
      }
    }).then((fn) => {if(disposed)fn();else unlistenDrop=fn});

    return () => { disposed=true;unlistenProgress?.(); unlistenDrop?.(); window.clearTimeout(outputCleanupMessageTimer);window.clearTimeout(toastTimer); window.removeEventListener("keydown", playerKeys); window.removeEventListener("contextmenu", blockBrowserMenu); window.removeEventListener("beforeunload", persistRecovery);window.removeEventListener("container-toast",toastEvent);window.removeEventListener("error",browserError);window.removeEventListener("unhandledrejection",rejected); };
  });

  function setLanguage(next:"tr"|"en"){
    if(next===language)return;
    const previous=selected;
    language=next; localStorage.setItem("container-language",next); document.documentElement.lang=next;
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
    <span class="brand"><span class="brand-logo-stack" aria-hidden="true"><img class="brand-logo brand-logo-dark" src="/logo-dark.png" alt="" decoding="sync"><img class="brand-logo brand-logo-light" src="/logo-light.png" alt="" decoding="sync"></span>CONTAINER</span>
    {#if media}
      {@render historyControl()}
      <div class="file-summary">
      <span class="filename mono" title={`${media.name}${media.kind!=="image"?`\n${language==="tr"?"Süre":"Duration"}: ${formatDuration(media.duration)}`:""}${media.width?`\n${language==="tr"?"Çözünürlük":"Resolution"}: ${media.width}×${media.height}`:""}${media.kind==="video"&&media.fps?`\nFPS: ${media.fps.toFixed(3)}`:""}\nCodec: ${media.codec}\n${language==="tr"?"Boyut":"Size"}: ${formatBytes(media.size)}`}>{media.name}</span>
      <div class="chips mono">
        {#if media.kind!=="image"}<span><b>dur</b>{formatDuration(media.duration)}</span>{/if}
        {#if media.width}<span><b>res</b>{media.width}×{media.height}</span>{/if}
        {#if media.kind==="video"&&media.fps}<span class="media-extra"><b>fps</b>{media.fps.toFixed(3)}</span>{/if}
        <span class="media-extra"><b>codec</b>{media.codec}</span><span class="media-extra"><b>size</b>{formatBytes(media.size)}</span>
      </div>
      </div>
      <nav class="mode-tabs" aria-label="Workspace">
        <button class:active={workspaceMode === "toolbox"} onclick={() => setWorkspaceMode("toolbox")} disabled={operationBusy&&workspaceMode!=="toolbox"}>{t("toolbox")}</button>
        {#if media.kind === "video"}<button class:active={workspaceMode === "autocut"} onclick={() => setWorkspaceMode("autocut")} disabled={operationBusy&&workspaceMode!=="autocut"}>SMARTCUT</button>{/if}
        <button class:active={workspaceMode === "batch"} onclick={() => setWorkspaceMode("batch")} disabled={operationBusy&&workspaceMode!=="batch"}>{language === "tr" ? "TOPLU" : "BATCH"}</button>
      </nav>
      <div class="project-actions"><button onclick={saveProject} disabled={operationBusy}>{language==="tr"?"PROJEYİ KAYDET":"SAVE PROJECT"}</button></div>
      <button class="panel-reset-trigger" onclick={()=>panelResetDialogOpen=true} disabled={operationBusy} aria-label={language==="tr"?"Panel genişliklerini sıfırla":"Reset panel widths"} title={language==="tr"?"Panel genişliklerini sıfırla":"Reset panel widths"}><svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2.5" y="3.5" width="15" height="13" rx="1.5"/><path d="M7 3.5v13M13 3.5v13"/></svg></button>
      <button class="ghost top-cancel" onclick={closeMedia} disabled={operationBusy}>{t("close")}</button>
    {:else}
      <div class="landing-header-actions">
        <div class="language-switch landing-language"><button class:active={language==="tr"} onclick={()=>setLanguage("tr")}>TR</button><button class:active={language==="en"} onclick={()=>setLanguage("en")}>EN</button><i></i><button class="theme-button" class:active={theme==="dark"} title={language==="tr"?"Koyu tema":"Dark theme"} aria-label={language==="tr"?"Koyu tema":"Dark theme"} onclick={()=>setTheme("dark")}>☾</button><button class="theme-button" class:active={theme==="light"} title={language==="tr"?"Açık tema":"Light theme"} aria-label={language==="tr"?"Açık tema":"Light theme"} onclick={()=>setTheme("light")}>☀</button></div>
        {#if downloaderOpen}<button class="downloader-back" onclick={()=>downloaderOpen=false} disabled={downloaderBusy} title={downloaderBusy?(language==="tr"?"İndirme tamamlanana veya iptal edilene kadar bekle":"Wait until the download finishes or is cancelled"):(language==="tr"?"Ana menüye dön":"Back to main menu")}>← {language==="tr"?"GERİ":"BACK"}</button>{/if}
        <div class="project-actions landing-project-actions"><button onclick={openProject} disabled={operationBusy}>{language==="tr"?"PROJE AÇ":"OPEN PROJECT"}</button></div>
        {#if updaterEnabled}<button class="update-trigger" class:available={!!availableUpdate} class:checking={updateChecking} onclick={() => checkForUpdates(true)} title={language === "tr" ? "Güncellemeleri denetle" : "Check for updates"}><b>↻</b><span>{availableUpdate ? `v${availableUpdate.version}` : (language === "tr" ? "GÜNCELLE" : "UPDATE")}</span>{#if availableUpdate}<i></i>{/if}</button>{/if}
      </div>
    {/if}
  </header>

  {#if panelResetDialogOpen}
    <dialog class="panel-reset-dialog" use:mountPanelResetDialog oncancel={()=>panelResetDialogOpen=false} aria-labelledby="panel-reset-title" aria-describedby="panel-reset-description">
      <header><span class="panel-reset-dialog-icon" aria-hidden="true">↺</span><h2 id="panel-reset-title">{language==="tr"?"Panel düzenini sıfırla":"Reset panel layout"}</h2></header>
      <p id="panel-reset-description">{workspaceMode==="autocut"?(language==="tr"?"SmartCut yan panelleri varsayılan genişliklerine dönecek.":"SmartCut side panels will return to their default widths."):workspaceMode==="batch"?(language==="tr"?"Batch kontrol paneli varsayılan genişliğine dönecek.":"The Batch controls panel will return to its default width."):(language==="tr"?"Araçlar ve Parametreler panelleri varsayılan genişliklerine dönecek.":"Tools and Parameters will return to their default widths.")} {language==="tr"?"Proje ve düzenleme geçmişin değişmeyecek.":"Your project and editing history will stay unchanged."}</p>
      <footer><button class="panel-reset-cancel" onclick={()=>panelResetDialogOpen=false}>{language==="tr"?"İPTAL":"CANCEL"}</button><button class="panel-reset-confirm" onclick={confirmResetPanelWidths}>{language==="tr"?"SIFIRLA":"RESET"}</button></footer>
    </dialog>
  {/if}

  {#if updatePanel}
    <div class="update-layer">
      <button class="update-backdrop" aria-label={language === "tr" ? "Güncelleme penceresini kapat" : "Close update dialog"} onclick={() => { if (!updateInstalling) updatePanel = false; }}></button>
      <dialog class="update-dialog panel" open aria-labelledby="update-title">
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
      <dialog class="update-dialog project-files-dialog panel" open aria-labelledby="project-files-title">
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
      <dialog class="update-dialog dependency-dialog panel" open aria-labelledby="dependency-title">
        <header><div><span class="status-dot missing"></span><h2 id="dependency-title">{runtimeMigrationError ? (language === "tr" ? "FFMPEG GÜNCELLEMESİ TAMAMLANMADI" : "FFMPEG UPDATE DID NOT FINISH") : (language === "tr" ? "FFMPEG GEREKLİ" : "FFMPEG REQUIRED")}</h2></div><button onclick={() => dependencyPanel = false} aria-label={language === "tr" ? "Kapat" : "Close"}>×</button></header>
        <div class="dependency-message"><span>!</span><div><h3>{runtimeMigrationError ? (language === "tr" ? "SONRAKİ GÜNCELLEME İÇİN BİR ADIM GEREKİYOR" : "ONE STEP IS NEEDED FOR THE NEXT UPDATE") : (language === "tr" ? "MEDYA ARAÇLARI HENÜZ KULLANILAMAZ" : "MEDIA TOOLS ARE NOT READY YET")}</h3><p>{runtimeMigrationError ? (language === "tr" ? "CONTAINER, FFmpeg bileşenlerini güvenli güncelleme alanına hazırlayamadı. Bu sürüm çalışmaya devam eder; ancak sonraki küçük güncellemelerden önce aşağıdaki işlemi tekrar dene. Sorun sürerse bu sürümü yeniden kur." : "CONTAINER could not prepare its FFmpeg components for future lightweight updates. This version can still run; retry below before the next update. If it continues, reinstall this version.") : (language === "tr" ? "Kurulumla gelen FFmpeg bileşenleri bulunamadı veya çalıştırılamadı. CONTAINER’ı yeniden kurup tekrar dene." : "The FFmpeg components included with CONTAINER are missing or could not start. Reinstall CONTAINER and try again.")}</p>{#if runtimeMigrationError}<small class="runtime-migration-detail mono">{runtimeMigrationError}</small>{/if}</div></div>
        <footer><button class="ghost" onclick={() => dependencyPanel = false}>{language === "tr" ? "ŞİMDİ DEĞİL" : "NOT NOW"}</button><button class="dependency-check" onclick={runtimeMigrationError ? repairFfmpegRuntime : () => refreshFfmpegStatus(true)} disabled={dependencyChecking}>{dependencyChecking ? "…" : (language === "tr" ? (runtimeMigrationError ? "YENİDEN HAZIRLA" : "TEKRAR KONTROL ET") : (runtimeMigrationError ? "REPAIR RUNTIME" : "CHECK AGAIN"))}</button></footer>
      </dialog>
    </div>
  {/if}

  {#if !media && outputCleanupOpen}
    <div class="update-layer output-clean-layer">
      <button class="update-backdrop" aria-label={language==="tr"?"Çıktı temizleme penceresini kapat":"Close output cleanup dialog"} onclick={()=>{if(!outputCleaning)outputCleanupOpen=false}}></button>
      <dialog class="update-dialog output-clean-dialog panel" open aria-labelledby="output-clean-title">
        <header><div><span class="output-clean-dialog-icon">⌫</span><h2 id="output-clean-title">{language==="tr"?"CONTAINER OUTPUT TEMİZLE":"CLEAN CONTAINER OUTPUT"}</h2></div><button onclick={()=>outputCleanupOpen=false} disabled={outputCleaning} aria-label={language==="tr"?"Kapat":"Close"}>×</button></header>
        <p>{language==="tr"?"Downloads/CONTAINER Output içindeki tüm çıktılar Geri Dönüşüm Kutusu’na taşınacak; klasör yerinde kalacak.":"Everything inside Downloads/CONTAINER Output will be moved to the Recycle Bin; the folder itself will remain."}</p>
        {#if outputCleanupMessage}<small class="output-clean-error">{outputCleanupMessage}</small>{/if}
        <footer><button class="ghost" onclick={()=>outputCleanupOpen=false} disabled={outputCleaning}>{language==="tr"?"İPTAL":"CANCEL"}</button><button class="clean-confirm" onclick={cleanOutputFolder} disabled={outputCleaning}>{outputCleaning?"…":(language==="tr"?"ÇIKTILARI TEMİZLE":"CLEAN OUTPUT")}</button></footer>
      </dialog>
    </div>
  {/if}

  {#if downloaderOpen}
    <DownloaderWorkspace {language} onbusychange={(value:boolean)=>downloaderBusy=value} />
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
      <footer><span class="status-dot" class:missing={(ffmpegStatus !== null && !ffmpegStatus.ready)||(downloaderStatus!==null&&!downloaderStatus.ready)}></span> {ffmpegStatus?.ready&&downloaderStatus?.ready ? `FFMPEG · FFPROBE · YT-DLP ${t("ready").toUpperCase()}` : (dependencyChecking||downloaderStatus===null ? "CHECKING COMPONENTS" : "MEDIA COMPONENTS REQUIRED")}</footer>
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
      </button>
      {#if outputCleanupMessage}<div class="output-clean-toast" role="status">{outputCleanupMessage}</div>{/if}
    </section>
  {:else}
    {#if workspaceMode === "autocut" && media.kind === "video"}
      <AutoCutWorkspace bind:this={autoCutWorkspace} {media} {mediaUrl} {language} oncontinue={continueEditingOutput} onhistorychange={(undo:boolean,redo:boolean)=>{autoCutCanUndo=undo;autoCutCanRedo=redo}} onsessionchange={(value:unknown)=>{autoCutSession=value}} onbusychange={(value:boolean)=>autoCutBusy=value} />
    {:else if workspaceMode === "batch"}
      <BatchWorkspace bind:this={batchWorkspace} initialPath={media.path} {language} {availableEncoders} oncontinue={continueEditingOutput} onhistorychange={(undo:boolean,redo:boolean)=>{batchCanUndo=undo;batchCanRedo=redo}} onsessionchange={(value:unknown)=>{batchSession=value}} onbusychange={(value:boolean)=>batchBusy=value} />
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
          {#if favoritesOnly&&categories.length===0}<p class="favorites-empty">{language==="tr"?"Bu bölümde henüz favori araç yok.":"No favorite tools in this section yet."}</p>{/if}
        </div>
      </aside>

      <div class="workspace-resizer" role="slider" tabindex="0" aria-label={language==="tr"?"Araçlar panelinin genişliği":"Tools panel width"} aria-orientation="horizontal" aria-valuemin={panelSizes.minLeft} aria-valuemax={Math.min(560,panelSizes.available-panelSizes.right-panelSizes.minCenter)} aria-valuenow={Math.round(panelSizes.left)} onpointerdown={(event)=>startToolboxPanelResize(event,"left")} onkeydown={(event)=>toolboxPanelKey(event,"left")} ondblclick={resetToolboxPanelWidths} title={language==="tr"?"Genişliği sürükle · sıfırla: çift tık":"Drag to resize · double-click to reset"}></div>

      <section class="center-stack" class:timeline-active={timelineTool}>
        <div class="preview panel">
          <div class="preview-head"><span>{t("preview")}</span><span class="mono">{t(media.kind).toUpperCase()} · {media.codec.toUpperCase()}</span></div>
          <div class="media-stage" class:ac-player={media.kind === "video"} class:toolbox-player={media.kind === "video"} bind:this={toolboxStage}>
            {#if media.kind === "video"}
              <div class="video-canvas" bind:this={toolboxCanvas}>
              <!-- svelte-ignore a11y_media_has_caption -->
              {#if selected?.id==="clipper"&&toolValue("vertical_layout")==="blur"}
                <!-- svelte-ignore a11y_media_has_caption -->
                <video bind:this={transformBackdropVideo} class="transform-video-backdrop" style={verticalBackdropStyle()} src={mediaUrl} preload="metadata" muted tabindex="-1"></video>
              {/if}
              <!-- svelte-ignore a11y_media_has_caption -->
              <video bind:this={toolboxVideo} style={previewVideoStyle()} src={mediaUrl} preload="metadata" onloadedmetadata={handleToolboxMetadata} ontimeupdate={() => { if (toolboxVideo) toolboxCurrent = toolboxVideo.currentTime; syncTransformBackdrop(); }} onplay={() => {toolboxPlaying=true;syncTransformBackdrop(true);void transformBackdropVideo?.play().catch(()=>{})}} onpause={() => {toolboxPlaying=false;transformBackdropVideo?.pause()}} onended={() => {toolboxPlaying=false;transformBackdropVideo?.pause()}}></video>
              {#if selected?.id==="clipper"&&["split","squares","freecam"].includes(toolValue("vertical_layout"))}
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
                {#if fillBox}<div class="fill-pan-layer" style:left={`${fillBox.left}px`} style:top={`${fillBox.top}px`} style:width={`${fillBox.width}px`} style:height={`${fillBox.height}px`} onpointerdown={startFillPan} role="presentation"><span>{language==="tr"?"KADRAJI SÜRÜKLE":"DRAG TO REFRAME"}</span></div>{/if}
              {:else if ["transform","clipper"].includes(selected?.id??"") && toolValue("crop_mode") !== "off" && (selected?.id==="transform" || !["original","blur"].includes(toolValue("vertical_layout")))}
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
              {#if selected?.id==="clipper"&&toolValue("watermark_enabled")==="true"&&toolValue("watermark_text").trim()}
                <i class="clipper-watermark-background" style={clipperWatermarkPreviewStyle(true)}></i>
                <span class="clipper-watermark-preview" style={clipperWatermarkPreviewStyle()}>{toolValue("watermark_text")}</span>
              {/if}
              {#if selected?.id==="clipper"&&toolValue("social_tag_enabled")==="true"&&toolValue("social_tag_username").trim()}
                <div class="clipper-social-tag" class:boxed={toolValue("social_tag_style")==="boxed"} class:twitch={toolValue("social_tag_platform")==="twitch"} style={socialTagPreviewStyle()}>
                  <span class="clipper-social-icon"><img src={toolValue("social_tag_platform")==="twitch"?twitchMark:kickMark} alt="" /></span>
                  <span class="clipper-social-name">{toolValue("social_tag_username").trim()}</span>
                </div>
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
              <div class="ac-controls">
                {#if playerSeekHover}<span class="player-seek-tooltip mono" class:precision={playerSeekHover.precision} style:left={`${playerSeekHover.percent}%`}>{playerTime(playerSeekHover.time)}</span>{/if}
                <input class="player-seek" style={`--seek-pct:${media.duration ? Math.min(100, toolboxCurrent / media.duration * 100) : 0}%`} aria-label="Video position" type="range" min="0" max={media.duration ?? 0} step="0.01" value={toolboxCurrent} onpointerdown={precisionPlayerSeek} onpointermove={hoverPlayerSeek} onpointerleave={()=>playerSeekHover=null} onwheel={wheelPlayerSeek} oninput={(event) => seekToolbox(Number(event.currentTarget.value))}>
                <button onclick={() => seekToolboxBy(-5)} title="5 seconds back">−5</button>
                <button class="play" onclick={toggleToolboxPlayer} title="Play / Pause">{toolboxPlaying ? "Ⅱ" : "▶"}</button>
                <button onclick={() => seekToolboxBy(5)} title="5 seconds forward">+5</button>
                <span class="ac-time mono">{playerTime(toolboxCurrent)} <i>/</i> {playerTime(media.duration ?? 0)}</span>
                <input class="volume" aria-label="Volume" type="range" min="0" max="1" step="0.05" bind:value={toolboxVolume} oninput={() => { if (toolboxVideo) toolboxVideo.volume = toolboxVolume; }}>
                <button onclick={fullscreenToolboxPlayer} title="Fullscreen">⛶</button>
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
          </div>
        </div>

        {#if timelineTool && media.duration}
          <div class="tool-timeline panel">
            <header><div><h3>TIMELINE</h3><p>{selected?.id === "screenshot" ? (language==="tr"?"kare zamanını seç":"choose frame time") : (language==="tr"?"çıktı aralığını seç":"choose export range")}</p></div>{#if selected?.id==="cut"}<span class="timeline-current mono"><i>▶</i> {language==="tr"?"KONUM":"PLAYHEAD"} {editableTime(toolboxCurrent)}</span>{/if}<b class="mono">{selected?.id === "screenshot" ? playerTime(timelineBounds().start) : `${timelineTime(timelineBounds().start)} — ${timelineTime(timelineBounds().end)}`}</b></header>
            {#if rangeTimelineTool()}<div class="cut-timecodes"><div class="cut-timecode"><span>START <i>H:M:S</i></span><input aria-label={language==="tr"?"Başlangıç zamanı":"Start time"} class="mono" bind:value={cutStartInput} onfocus={()=>cutTimeEditing="start"} onblur={()=>commitCutTime("start")} onkeydown={(event)=>handleCutTimeKey(event,"start")} placeholder="0:05:14"><button onclick={()=>markCutAtPlayhead("start")} title={language==="tr"?"Geçerli oynatma zamanını başlangıç yap (I)":"Set IN to current playhead time (I)"}><b>IN</b><kbd>I</kbd></button></div><div class="cut-timecode"><span>END <i>H:M:S</i></span><input aria-label={language==="tr"?"Bitiş zamanı":"End time"} class="mono" bind:value={cutEndInput} onfocus={()=>cutTimeEditing="end"} onblur={()=>commitCutTime("end")} onkeydown={(event)=>handleCutTimeKey(event,"end")} placeholder="0:05:46"><button onclick={()=>markCutAtPlayhead("end")} title={language==="tr"?"Geçerli oynatma zamanını bitiş yap (O)":"Set OUT to current playhead time (O)"}><b>OUT</b><kbd>O</kbd></button></div><small class="cut-seek-help mono">{language==="tr"?"PLAYER ÇUBUĞU: tekerlek ±1 sn · Ctrl+tekerlek ±5 sn · Shift+tık tam saniye":"PLAYER BAR: wheel ±1 sec · Ctrl+wheel ±5 sec · Shift+click whole second"}</small></div>{/if}
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
            <div><h3>{t("process")}</h3><p class="mono">{outputStale?(language==="tr"?"ayarlar değişti · yeniden işle":"settings changed · render again"):jobStatus}</p></div>
            <div class="job-meta">
              <div class="job-stats mono"><span><b>{t("frame")}</b>{frame}</span><span><b>{t("speed")}</b>{speed}</span><span><b>{t("elapsed")}</b>{elapsed.toFixed(1)}s</span></div>
              {#if output}<button class="ghost job-action" disabled={operationBusy||outputStale} title={outputStale?(language==="tr"?"Ayarlar değişti; önce yeniden işle.":"Settings changed; render again first."):undefined} onclick={()=>continueEditingOutput()}>{language==="tr"?"çıktıyı düzenle":"continue editing"}</button><button class="ghost job-action" onclick={() => revealItemInDir(output)}>{outputStale?(language==="tr"?"önceki çıktı":"previous output"):t("showOutput")}</button>{/if}
              {#if busy}<button class="danger job-action" onclick={cancelJob}>{t("cancelJob")}</button>{/if}
            </div>
          </div>
          <div class="progress-track"><div style:width={`${progress}%`}></div></div>
          {#if error}<div class="error-box">{error}</div>{/if}
        </div>
      </section>

      <div class="workspace-resizer" role="slider" tabindex="0" aria-label={language==="tr"?"Parametreler panelinin genişliği":"Parameters panel width"} aria-orientation="horizontal" aria-valuemin={panelSizes.minRight} aria-valuemax={Math.min(620,panelSizes.available-panelSizes.left-panelSizes.minCenter)} aria-valuenow={Math.round(panelSizes.right)} onpointerdown={(event)=>startToolboxPanelResize(event,"right")} onkeydown={(event)=>toolboxPanelKey(event,"right")} ondblclick={resetToolboxPanelWidths} title={language==="tr"?"Genişliği sürükle · sıfırla: çift tık":"Drag to resize · double-click to reset"}></div>

      <aside class="settings panel" class:merge-compact={selected?.id==="merge_videos"} class:compact-controls={panelSizes.right<300}>
        {#if selected}
          <div class="pane-head"><div><h3>{t("parameters")}</h3><p>{selected.category}</p></div><button class="reset" onclick={resetSelectedTool}>{t("defaults")}</button></div>
          <div class="selected-title"><span class="index mono">{String(kindTools(activeKind).findIndex((tool) => tool.id === selected?.id) + 1).padStart(2,"0")}</span><div><h2>{selected.title}</h2><p>{selected.description}</p></div></div>
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
                        <span><input type="checkbox" checked={colorOn(key)} onchange={()=>toggleColor(key)}><b>{language==="tr"?(field.label):colorLabels[key]}</b><em>{colorValueLabel(key)}</em><button type="button" title="Reset" onclick={(event)=>{event.preventDefault();resetColorKey(key)}}>↻</button></span>
                        <input type="range" style={`--range-pct:${rangePercent(Number(field.value),Number(field.min),Number(field.max))}%`} min={field.min} max={field.max} step={field.step} value={field.value} disabled={!colorOn(key)} oninput={(event)=>setToolNumber(key,Number(event.currentTarget.value))}>
                      </label>
                    {/if}
                  {/each}
                  {#if group.title === "Cleanup"}
                    <div class="denoise-control">
                      <label class="color-toggle"><input type="checkbox" checked={toolValue("denoise")!=="off"} onchange={(event)=>setToolValue("denoise",event.currentTarget.checked?"medium":"off")}><span>{language==="tr"?"Gürültü azaltma":"Denoise"}</span></label>
                      <div class="segmented">{#each ["low","medium","high"] as mode}<button class:active={toolValue("denoise")===mode} disabled={toolValue("denoise")==="off"} onclick={()=>setToolValue("denoise",mode)}>{mode}</button>{/each}</div>
                    </div>
                  {/if}
                  {#if group.title === "Style"}
                    <label class="color-toggle"><input type="checkbox" checked={toolValue("grayscale")==="on"} onchange={(event)=>setToolValue("grayscale",event.currentTarget.checked?"on":"off")}><span>{language==="tr"?"Gri tonlama":"Grayscale"}</span></label>
                  {/if}
                </section>
              {/each}
              <section class="color-group"><h4>{language==="tr"?"Tarama":"Interlace"}</h4><div class="segmented">{#each ["off","auto","on"] as mode}<button class:active={toolValue("deinterlace")===mode} onclick={()=>setToolValue("deinterlace",mode)}>{mode}</button>{/each}</div></section>
            </div>
          {:else if selected.id === "text"}
            <div class="text-workspace">
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
                    <button class:active={toolValue("vertical_layout")==="original"} onclick={()=>setVerticalLayout("original")}>ORIGINAL SIZE</button>
                    <button class:active={toolValue("vertical_layout")==="blur"} onclick={()=>setVerticalLayout("blur")}>BLUR</button>
                    <button class:active={toolValue("vertical_layout")==="fill"} onclick={()=>setVerticalLayout("fill")}>FILL</button>
                    <button class:active={toolValue("vertical_layout")==="split"} onclick={()=>setVerticalLayout("split")}>SPLIT</button>
                    <button class:active={toolValue("vertical_layout")==="squares"} onclick={()=>setVerticalLayout("squares")}>SQUARES</button>
                    <button class:active={toolValue("vertical_layout")==="freecam"} onclick={()=>setVerticalLayout("freecam")}>FREECAM</button>
                  </div>
                  {#if ["split","squares","freecam"].includes(toolValue("vertical_layout"))}
                    <button class="auto-camera" class:working={cameraDetecting} onclick={autoDetectCamera} disabled={cameraDetecting||busy}><span>{cameraDetecting?"◌":"◇"}</span>{cameraDetecting?(language==="tr"?"KAMERA ARANIYOR…":"DETECTING CAMERA…"):(language==="tr"?"KAMERAYI OTOMATİK BUL":"AUTO-DETECT CAMERA")}<em title="Experimental feature">EXPERIMENTAL</em></button>
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
                  {#if toolValue("vertical_layout")==="split"}
                    <details class="text-style-options clipper-advanced">
                      <summary>{language==="tr"?"GELİŞMİŞ":"ADVANCED"}</summary>
                      <label><span>{language==="tr"?"Sıralama":"Order"}</span><select value={toolValue("region_order")} onchange={(event)=>setSplitOrder(event.currentTarget.value)}><option value="a_first">{language==="tr"?"Kamera üstte":"Camera above content"}</option><option value="b_first">{language==="tr"?"İçerik üstte":"Content above camera"}</option></select></label>
                      <label><span>{language==="tr"?"Üst bölüm yüksekliği":"Top region height"}</span><input type="range" min="20" max="80" step="1" value={toolNumber("region_a_height")} oninput={(event)=>setSplitHeight(Number(event.currentTarget.value))}><small>{toolNumber("region_a_height").toFixed(0)}%</small></label>
                      <p>{language==="tr"?"Kamera ve içerik kutularını kaynak önizleme üzerinde sürükleyip kenarlarından boyutlandır.":"Drag Camera and Content on the source preview and resize them from their edges."}</p>
                    </details>
                  {/if}
                  {#if toolValue("vertical_layout")==="fill"}<p>{language==="tr"?"Video dikey tuvali tamamen doldurur. Sonuç önizlemesini sürükleyerek yatay kadrajı ayarla.":"The video fills the vertical canvas completely. Drag the result preview to adjust the horizontal framing."}</p>{/if}
                  {#if toolValue("vertical_layout")==="squares"}<p>{language==="tr"?"Kamera ve içerik, dikey tuvali eşit iki tam genişlikte bölüme ayırır.":"Camera and content divide the vertical canvas into two equal full-width sections."}</p>{/if}
                  {#if toolValue("vertical_layout")==="freecam"}
                    {@const freecam=freecamPlacement()}
                    <div class="freecam-layout" bind:this={freecamLayoutBox} aria-label={language==="tr"?"Freecam çıktı yerleşimi":"Freecam output layout"}>
                      <span>{language==="tr"?"İÇERİK":"CONTENT"}</span>
                      <button class="freecam-camera" style:left={`${freecam.left}%`} style:top={`${freecam.top}%`} style:width={`${freecam.width}%`} style:height={`${freecam.height}%`} onpointerdown={(event)=>startFreecamPlacement(event,"move")}>
                        {language==="tr"?"KAMERA":"CAMERA"}<i role="presentation" onpointerdown={(event)=>startFreecamPlacement(event,"resize")}></i>
                      </button>
                    </div>
                    <p>{language==="tr"?"Kamerayı çıktı yerleşiminde sürükle; sağ alt köşeden boyutlandır. Kaynak kırpımını ana önizlemedeki Camera Region ile ayarla.":"Drag the camera in the output layout; resize it from the lower-right corner. Adjust its source crop with Camera Region in the main preview."}</p>
                    <label><span>{language==="tr"?"Kamera konumu X":"Camera position X"}</span><input type="range" min="0" max="100" step="1" value={toolNumber("freecam_x")} oninput={(event)=>setToolNumber("freecam_x",Number(event.currentTarget.value))}></label>
                    <label><span>{language==="tr"?"Kamera konumu Y":"Camera position Y"}</span><input type="range" min="0" max="100" step="1" value={toolNumber("freecam_y")} oninput={(event)=>setToolNumber("freecam_y",Number(event.currentTarget.value))}></label>
                    <label><span>{language==="tr"?"Kamera boyutu":"Camera size"}</span><input type="range" min="15" max="90" step="1" value={toolNumber("freecam_size")} oninput={(event)=>setToolNumber("freecam_size",Number(event.currentTarget.value))}><small>{toolNumber("freecam_size").toFixed(0)}%</small></label>
                  {/if}
                  <div class="clipper-watermark-controls">
                    <label class="watermark-toggle"><input type="checkbox" checked={toolValue("watermark_enabled")==="true"} onchange={(event)=>setToolValue("watermark_enabled",event.currentTarget.checked?"true":"false")}><span>Watermark</span></label>
                    {#if toolValue("watermark_enabled")==="true"}
                      {#if ["split","squares","freecam"].includes(toolValue("vertical_layout"))}<label class="watermark-toggle"><input type="checkbox" checked={toolValue("watermark_background")==="true"} onchange={(event)=>setToolValue("watermark_background",event.currentTarget.checked?"true":"false")}><span>{language==="tr"?"Siyah arka plan şeridi":"Black background strip"}</span></label>{/if}
                      <label class="watermark-text-field"><span>{language==="tr"?"Yazı":"Text"}</span><input type="text" maxlength="64" placeholder="@kanaladi" value={toolValue("watermark_text")} oninput={(event)=>setToolValue("watermark_text",event.currentTarget.value)}></label>
                      <label class="watermark-slider"><span>{language==="tr"?"Boyut":"Size"}<small>{toolNumber("watermark_size").toFixed(0)} px</small></span><input type="range" min="18" max="160" step="1" value={toolNumber("watermark_size")} oninput={(event)=>setToolNumber("watermark_size",Number(event.currentTarget.value))}></label>
                      <label class="watermark-slider"><span>{language==="tr"?"Saydamlık":"Opacity"}<small>{toolNumber("watermark_opacity").toFixed(0)}%</small></span><input type="range" min="10" max="100" step="5" value={toolNumber("watermark_opacity")} oninput={(event)=>setToolNumber("watermark_opacity",Number(event.currentTarget.value))}></label>
                      <p>{language==="tr"?"Split/Squares'ta iki panelin birleşim çizgisinin tam ortasında; diğer düzenlerde TikTok ve Shorts arayüzlerinden uzak ortak güvenli alanda görünür.":"Centered exactly on the Split/Squares panel seam; other layouts use a shared TikTok/Shorts safe area."}</p>
                    {/if}
                  </div>
                  <div class="clipper-watermark-controls">
                    <label class="watermark-toggle"><input type="checkbox" checked={toolValue("social_tag_enabled")==="true"} onchange={(event)=>setToolValue("social_tag_enabled",event.currentTarget.checked?"true":"false")}><span>Social Tag</span></label>
                    {#if toolValue("social_tag_enabled")==="true"}
                      <label class="watermark-text-field"><span>{language==="tr"?"Platform":"Platform"}</span><select value={toolValue("social_tag_platform")} onchange={(event)=>setToolValue("social_tag_platform",event.currentTarget.value)}><option value="kick">Kick</option><option value="twitch">Twitch</option></select></label>
                      <label class="watermark-text-field"><span>{language==="tr"?"Kullanıcı adı":"Username"}</span><input type="text" maxlength="32" placeholder="kanaladi" value={toolValue("social_tag_username")} oninput={(event)=>setToolValue("social_tag_username",event.currentTarget.value)}></label>
                      <label class="watermark-text-field"><span>{language==="tr"?"Görünüm":"Style"}</span><select value={toolValue("social_tag_style")} onchange={(event)=>setToolValue("social_tag_style",event.currentTarget.value)}><option value="boxed">{language==="tr"?"Kutulu etiket":"Boxed badge"}</option><option value="plain">{language==="tr"?"Düz etiket":"Plain tag"}</option></select></label>
                      <label class="watermark-text-field"><span>{language==="tr"?"Konum":"Position"}</span><select value={toolValue(toolValue("social_tag_style")==="boxed"?"social_tag_boxed_position":"social_tag_plain_position")} onchange={(event)=>setToolValue(toolValue("social_tag_style")==="boxed"?"social_tag_boxed_position":"social_tag_plain_position",event.currentTarget.value)}><option value="left">{language==="tr"?"Sol":"Left"}</option><option value="center">{language==="tr"?"Orta":"Center"}</option><option value="right">{language==="tr"?"Sağ":"Right"}</option></select></label>
                      <label class="watermark-slider"><span>{language==="tr"?"Boyut":"Size"}<small>{toolNumber("social_tag_size").toFixed(0)} px</small></span><input type="range" min="20" max="96" step="1" value={toolNumber("social_tag_size")} oninput={(event)=>setToolNumber("social_tag_size",Number(event.currentTarget.value))}></label>
                    {/if}
                  </div>
                </section>
              {/if}
              {#if selected.id==="transform"}
              <section>
                <header><b>CROP</b><small>{toolValue("crop_mode")==="off" ? (language==="tr"?"kapalı":"off") : `${toolNumber("crop_w").toFixed(1)}% × ${toolNumber("crop_h").toFixed(1)}%`}</small></header>
                <div class="transform-options crop-options">
                  {#each transformPresets.filter(preset=>(media?.kind==="image"||!["5:4","3:4"].includes(preset))&&!(media?.kind==="image"&&toolValue("fit_mode")==="contain"&&preset==="free")) as preset}<button class:active={toolValue("crop_mode")===preset} onclick={()=>setCropPreset(preset)}>{preset==="off"?(media?.kind==="image"?"ORIGINAL":"OFF"):preset==="191:100"?"1.91:1":preset.toUpperCase()}</button>{/each}
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
                <header><b>ROTATE</b><small>{toolValue("rotate")}°</small></header>
                <div class="transform-options four"><button class:active={toolValue("rotate")==="0"} onclick={()=>setTransformRotation(0)}>0°</button><button onclick={()=>rotateTransform(-90)}>↶ 90°</button><button onclick={()=>rotateTransform(90)}>↷ 90°</button><button onclick={()=>rotateTransform(180)}>180°</button></div>
              </section>
              <section>
                <header><b>FLIP</b></header>
                <div class="transform-options two"><button class:active={toolValue("flip_h")==="true"} onclick={()=>setToolValue("flip_h",toolValue("flip_h")==="true"?"false":"true")}>↔ {language==="tr"?"Yatay":"Horizontal"}</button><button class:active={toolValue("flip_v")==="true"} onclick={()=>setToolValue("flip_v",toolValue("flip_v")==="true"?"false":"true")}>↕ {language==="tr"?"Dikey":"Vertical"}</button></div>
              </section>
              <section>
                <header><b>{language==="tr"?"ÇIKTI BOYUTU":"OUTPUT SIZE"}</b></header>
                <div class="transform-options two"><button class:active={toolValue("size_mode")==="source"} onclick={()=>setToolValue("size_mode","source")}>{language==="tr"?"Kırpılan boyutu koru":"Keep crop size"}</button><button class:active={toolValue("size_mode")==="height"} onclick={()=>setToolValue("size_mode","height")}>{language==="tr"?"Yükseklik":"Height"}</button><button class:active={toolValue("size_mode")==="width"} onclick={()=>setToolValue("size_mode","width")}>{language==="tr"?"Genişlik":"Width"}</button><button class:active={toolValue("size_mode")==="exact"} onclick={()=>setToolValue("size_mode","exact")}>{language==="tr"?"Tam boyut":"Exact"}</button></div>
                {#if ["height","width"].includes(toolValue("size_mode"))}
                  <label><span>{toolValue("size_mode")==="height"?(language==="tr"?"Hedef yükseklik":"Target height"):(language==="tr"?"Hedef genişlik":"Target width")}</span><div class="size-entry"><select value={String(toolNumber("size"))} onchange={(event)=>setToolNumber("size",Number(event.currentTarget.value))}>{#each [480,720,1080,1440,2160,4320] as size}<option value={size}>{size}px</option>{/each}</select><input aria-label="Custom output size" type="number" min="2" max="7680" step="2" value={toolNumber("size")} oninput={(event)=>setToolNumber("size",Number(event.currentTarget.value))}></div></label>
                {:else if toolValue("size_mode")==="exact"}
                  <div class="exact-size"><label><span>{language==="tr"?"Genişlik":"Width"}</span><input type="number" min="2" max="7680" step="2" value={toolNumber("output_width")} oninput={(event)=>setToolNumber("output_width",Number(event.currentTarget.value))}></label><b>×</b><label><span>{language==="tr"?"Yükseklik":"Height"}</span><input type="number" min="2" max="7680" step="2" value={toolNumber("output_height")} oninput={(event)=>setToolNumber("output_height",Number(event.currentTarget.value))}></label></div>
                  <p>{media.kind==="image"?(language==="tr"?"Görsel esnetilmeden bu tuvale sığdırılır; boş alanlar şeffaf kalır.":"The image is fitted into this canvas without stretching; unused space remains transparent."):(language==="tr"?"Tam boyut, seçtiğin kadrajı bu ölçülere ölçekler; oranlar farklıysa görüntü esneyebilir.":"Exact size scales the crop to these dimensions; mismatched ratios may stretch the image.")}</p>
                {/if}
              </section>
              {#if media.kind === "image"}
                <section>
                  <header><b>{language==="tr"?"ÇIKTI FORMATI":"OUTPUT FORMAT"}</b></header>
                  <div class="transform-options three"><button class:active={toolValue("format")==="png"} onclick={()=>setToolValue("format","png")}>PNG · LOSSLESS</button><button class:active={toolValue("format")==="webp"} onclick={()=>setToolValue("format","webp")}>WEBP</button><button class:active={toolValue("format")==="jpg"} onclick={()=>setToolValue("format","jpg")}>JPEG</button><button class:active={toolValue("format")==="bmp"} onclick={()=>setToolValue("format","bmp")}>BMP</button><button class:active={toolValue("format")==="tiff"} onclick={()=>setToolValue("format","tiff")}>TIFF</button><button class:active={toolValue("format")==="avif"} onclick={()=>setToolValue("format","avif")}>AVIF</button></div>
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
              <div class="codec-note"><b>{language==="tr"?"TAHMİNİ BOYUT":"ESTIMATED SIZE"}</b><span>{formatBytes(media.size)} → {toolValue("mode")==="target"?`${toolNumber("target_kb")} KB`:(compressionEstimateLoading?(language==="tr"?"hesaplanıyor…":"calculating…"):(compressionEstimate!=null?formatBytes(compressionEstimate):"—"))}</span></div>
              {#if renderedImageSize}<div class="codec-note"><b>{language==="tr"?"GERÇEK ÇIKTI":"ACTUAL OUTPUT"}</b><span>{formatBytes(renderedImageSize)}</span></div>{/if}
            {/if}
          </div>
          <div class="run-box">
            <button class="run" onclick={runTool} disabled={busy||qualityAnalyzing}>▶ {selected.id === "file_hash" ? (language === "tr" ? "SHA-256 hesapla" : "calculate SHA-256") : `${t("render")} ${selected.title.toLocaleLowerCase(language)}`}</button>
          </div>
        {:else}
          <div class="empty-settings"><span>←</span><p>{t("selectTool")}</p></div>
        {/if}
      </aside>
    </section>
    {/if}
  {/if}
  {#if dragActive}<div class="drop-overlay"><span>{t("dropOpen")}</span></div>{/if}
  {#if toastMessage}<aside class:error={toastKind==="error"} class="app-toast" role="alert"><span>{toastMessage}</span><button onclick={()=>{toastMessage="";window.clearTimeout(toastTimer)}} aria-label={language==="tr"?"Bildirimi kapat":"Close notification"}>×</button></aside>{/if}
</main>
