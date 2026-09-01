<script lang="ts">
  import "@fontsource-variable/geist";
  import "@fontsource-variable/geist-mono";
  import { onMount } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { check, Update } from "@tauri-apps/plugin-updater";
  import { localizedForSection, localizedTool, type Field, type MediaKind, type Tool } from "./lib/tools";
  import AutoCutWorkspace from "./lib/AutoCutWorkspace.svelte";
  import BatchWorkspace from "./lib/BatchWorkspace.svelte";

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
  interface FfmpegStatus { ready: boolean; ffmpeg_version: string | null; ffprobe_version: string | null }

  let media: MediaInfo | null = $state(null);
  let mediaUrl = $state("");
  let selected: Tool | null = $state(null);
  let activeKind: MediaKind = $state("video");
  let busy = $state(false);
  let dragActive = $state(false);
  let error = $state("");
  let output = $state("");
  let progress = $state(0);
  let jobStatus = $state("ready");
  let speed = $state("—");
  let frame = $state("—");
  let elapsed = $state(0);
  let search = $state("");
  let workspaceMode: "toolbox" | "autocut" | "batch" = $state("toolbox");
  let toolboxVideo: HTMLVideoElement | null = $state(null);
  let toolboxStage: HTMLElement | null = $state(null);
  let toolboxCurrent = $state(0);
  let toolboxPlaying = $state(false);
  let toolboxVolume = $state(1);
  let renderedImageUrl = $state("");
  let qualityAnalysis: QualityAnalysis | null = $state(null);
  let hashResult = $state("");
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
  let toolboxTimeline: HTMLElement | null = $state(null);
  let language: "tr" | "en" = $state("en");
  let theme: "dark" | "light" = $state("dark");
  let availableEncoders: string[] | null = $state(null);
  let ffmpegStatus: FfmpegStatus | null = $state(null);
  let dependencyChecking = $state(false);
  let dependencyPanel = $state(false);
  let appVersion = $state("0.4.2");
  let availableUpdate: Update | null = $state(null);
  let updatePanel = $state(false);
  let updateChecking = $state(false);
  let updateInstalling = $state(false);
  let updateStatus = $state("");
  let updateDownloaded = $state(0);
  let updateTotal = $state(0);
  const messages:Record<"tr"|"en",Record<string,string>>={
    tr:{tagline:"FFMPEG MEDYA ARAÇ KUTUSU",close:"kapat",drop:"medyayı buraya bırak",browse:"veya dosya seçmek için tıkla",landingTitle:"tek dosya. bütün araçlar.",landingCopy:"CONTAINER’ın bütün FFmpeg işlemleri, ayrıntılı ayarlar ve canlı ilerleme bilgisiyle tek çalışma alanında.",local:"yalnızca yerel işlem",untouched:"orijinal dosyalar değişmez",tools:"ARAÇLAR",available:"mevcut",video:"video",audio:"ses",image:"görsel",search:"araçlarda ara...",preview:"ÖNİZLEME",original:"ORİJİNAL",rendered:"İŞLENMİŞ",process:"İŞLEM",frame:"kare",speed:"hız",elapsed:"geçen",showOutput:"çıktıyı göster",cancelJob:"işlemi iptal et",parameters:"PARAMETRELER",defaults:"varsayılanlar",what:"NE YAPAR?",forVideo:"BU VİDEO İÇİN",choose:"dosya seç...",custom:"Özel…",render:"işle",outputNote:"Çıktı Downloads/CONTAINER Output klasörüne yazılır. Kaynak dosya değiştirilmez.",selectTool:"Bir araç seç",dropOpen:"açmak için bırak",ready:"hazır",toolbox:"ARAÇ KUTUSU"},
    en:{tagline:"FFMPEG MEDIA TOOLBOX",close:"close",drop:"drop media here",browse:"or click to browse files",landingTitle:"one file. every tool.",landingCopy:"All CONTAINER FFmpeg operations in one workspace with detailed controls and live progress.",local:"local processing only",untouched:"original files stay untouched",tools:"TOOLS",available:"available",video:"video",audio:"audio",image:"image",search:"search tools...",preview:"PREVIEW",original:"ORIGINAL",rendered:"RENDERED",process:"PROCESS",frame:"frame",speed:"speed",elapsed:"elapsed",showOutput:"show output",cancelJob:"cancel job",parameters:"PARAMETERS",defaults:"defaults",what:"WHAT DOES IT DO?",forVideo:"FOR THIS VIDEO",choose:"choose file...",custom:"Custom…",render:"render",outputNote:"Output is written to Downloads/CONTAINER Output. The source file is not changed.",selectTool:"Select a tool",dropOpen:"drop to open",ready:"ready",toolbox:"TOOLBOX"}
  };
  const t=(key:string)=>messages[language][key]??key;
  const kindTools=(kind:MediaKind)=>localizedForSection(kind,media?.kind??kind,language);
  const timelineTool = $derived.by(()=>selected ? ["cut","screenshot","gif"].includes(selected.id) : false);
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenDrop: UnlistenFn | null = null;

  const categories = $derived.by(() => {
    const list = kindTools(activeKind).filter((tool) => `${tool.title} ${tool.description}`.toLocaleLowerCase(language).includes(search.toLocaleLowerCase(language)));
    const map = new Map<string, Tool[]>();
    for (const tool of list) map.set(tool.category, [...(map.get(tool.category) ?? []), tool]);
    return [...map.entries()];
  });

  const formatBytes = (value: number) => {
    if (!Number.isFinite(value)) return "—";
    const units = ["B", "KB", "MB", "GB"];
    let amount = value;
    let unit = 0;
    while (amount >= 1024 && unit < units.length - 1) { amount /= 1024; unit++; }
    return `${amount.toFixed(unit ? 2 : 0)} ${units[unit]}`;
  };
  const formatDuration = (value: number | null) => value == null ? "—" : `${value.toFixed(2)}s`;
  const playerTime = (value: number) => {
    const safe = Math.max(0, Number(value) || 0);
    const hours = Math.floor(safe / 3600);
    const minutes = Math.floor((safe % 3600) / 60);
    const seconds = Math.floor(safe % 60);
    const millis = Math.floor((safe % 1) * 1000);
    return `${hours ? `${String(hours).padStart(2,"0")}:` : ""}${String(minutes).padStart(2,"0")}:${String(seconds).padStart(2,"0")}.${String(millis).padStart(3,"0")}`;
  };
  const basename = (path: string) => path.split(/[\\/]/).pop() ?? path;

  function chooseTool(tool: Tool) {
    const changed = selected?.id !== tool.id;
    selected = localizedTool(tool,language);
    if (selected.id === "encode" && availableEncoders) {
      const encoderField = selected.fields.find((item) => item.key === "encoder");
      if (encoderField) {
        encoderField.options = (encoderField.options ?? []).filter((option) => availableEncoders!.includes(option.value));
        if (!encoderField.options.some((option) => option.value === String(encoderField.value))) {
          encoderField.value = encoderField.options[0]?.value ?? "libx264";
        }
      }
    }
    error = "";
    output = "";
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
    if (media?.duration) {
      for (const field of selected.fields) {
        if (field.key === "end") field.value = Math.min(10, media.duration);
        if (field.key === "duration") field.value = Math.min(5, media.duration);
        if (["start", "end", "duration", "timestamp"].includes(field.key)) field.max = media.duration;
      }
    }
    const audioTrackField = selected.fields.find((item) => item.key === "audio_track");
    if (audioTrackField && media) {
      audioTrackField.options = media.audio_tracks.map((track, position) => {
        const languageLabel = track.language ? ` · ${track.language.toUpperCase()}` : "";
        const channelLabel = track.channel_layout ?? (track.channels ? `${track.channels} ch` : "audio");
        const bitrateLabel = track.bitrate ? ` · ${Math.round(track.bitrate / 1000)} kbps` : "";
        const defaultLabel = track.is_default ? (language === "tr" ? " · varsayılan" : " · default") : "";
        return { value:String(track.index), label:`${language === "tr" ? "Parça" : "Track"} ${position + 1} · ${track.codec.toUpperCase()} · ${channelLabel}${languageLabel}${bitrateLabel}${defaultLabel}` };
      });
      if (audioTrackField.options.length) audioTrackField.value = audioTrackField.options[0].value;
    }
    if (["cut","screenshot","gif"].includes(selected.id)) void loadToolboxFilmstrip();
  }

  function toolField(key:string){return selected?.fields.find(field=>field.key===key)}
  function fieldLivesOnTimeline(key:string){return selected?.id==="cut"?["start","end"].includes(key):selected?.id==="screenshot"?key==="timestamp":selected?.id==="gif"?["start","duration"].includes(key):false}
  function fieldVisible(key:string){
    if(key==="audio_track") return String(toolField("audio_mode")?.value)==="selected";
    if(selected?.id==="cut"&&key==="crf") return String(toolField("cut_mode")?.value)!=="lossless";
    if(selected?.id==="speed"&&key==="crf") return String(toolField("speed_mode")?.value)!=="lossless_video";
    if(selected?.id==="potatoify"&&["fps","video_badness","audio_badness","shrink"].includes(key)) return String(toolField("profile")?.value)==="custom";
    return true;
  }
  function toolNumber(key:string){return Number(toolField(key)?.value??0)}
  function setToolNumber(key:string,value:number){const field=toolField(key);if(field)field.value=Math.round(value*1000)/1000}
  function timelineBounds(){
    if(!selected)return {start:0,end:0};
    if(selected.id==="screenshot"){const at=toolNumber("timestamp");return {start:at,end:at}}
    if(selected.id==="gif"){const start=toolNumber("start");return {start,end:start+toolNumber("duration")}}
    return {start:toolNumber("start"),end:toolNumber("end")};
  }
  async function loadToolboxFilmstrip(){
    if(!media||media.kind!=="video"||toolboxFilmstripLoading||toolboxFilmstripUrl)return;
    toolboxFilmstripLoading=true;
    try{toolboxFilmstripUrl=await invoke<string>("compute_video_filmstrip",{path:media.path})}catch(reason){error=String(reason)}finally{toolboxFilmstripLoading=false}
  }
  function timelineAt(clientX:number){if(!toolboxTimeline||!media?.duration)return 0;const rect=toolboxTimeline.getBoundingClientRect();return Math.max(0,Math.min(media.duration,(clientX-rect.left)/rect.width*media.duration))}
  function seekTimeline(event:MouseEvent){const at=timelineAt(event.clientX);seekToolbox(at);if(selected?.id==="screenshot")setToolNumber("timestamp",at)}
  function startToolTimelineDrag(event:PointerEvent,mode:"start"|"end"|"point"|"range"){
    event.preventDefault();event.stopPropagation();if(!selected||!media?.duration)return;
    const initial=timelineBounds(),pointerStart=timelineAt(event.clientX),span=initial.end-initial.start;
    const update=(clientX:number)=>{
      const at=timelineAt(clientX),duration=media?.duration??0;
      if(mode==="point"){setToolNumber("timestamp",at);seekToolbox(at);return}
      let start=initial.start,end=initial.end;
      if(mode==="start")start=Math.min(end-.01,at);
      else if(mode==="end")end=Math.max(start+.01,at);
      else{start=Math.max(0,Math.min(duration-span,initial.start+(at-pointerStart)));end=start+span}
      if(selected?.id==="gif"){setToolNumber("start",start);setToolNumber("duration",Math.max(.01,end-start))}
      else{setToolNumber("start",start);setToolNumber("end",end)}
      seekToolbox(mode==="end"?end:start);
    };
    update(event.clientX);const move=(moveEvent:PointerEvent)=>update(moveEvent.clientX);const stop=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",stop)};window.addEventListener("pointermove",move);window.addEventListener("pointerup",stop)
  }
  function timelineHandleKey(event:KeyboardEvent,mode:"start"|"end"|"point"){
    if(event.key!=="ArrowLeft"&&event.key!=="ArrowRight"||!media?.duration)return;
    event.preventDefault();const delta=(event.shiftKey?.01:.1)*(event.key==="ArrowRight"?1:-1),bounds=timelineBounds();
    if(mode==="point"){const value=Math.max(0,Math.min(media.duration,bounds.start+delta));setToolNumber("timestamp",value);seekToolbox(value);return}
    const start=mode==="start"?Math.max(0,Math.min(bounds.end-.01,bounds.start+delta)):bounds.start;
    const end=mode==="end"?Math.min(media.duration,Math.max(start+.01,bounds.end+delta)):bounds.end;
    if(selected?.id==="gif"){setToolNumber("start",start);setToolNumber("duration",end-start)}else{setToolNumber("start",start);setToolNumber("end",end)}
    seekToolbox(mode==="start"?start:end)
  }

  function switchKind(kind:MediaKind){
    if(!media)return;
    const allowed=kind===media.kind||(media.kind==="video"&&kind==="audio");
    if(!allowed)return;
    activeKind=kind; search="";
    const first=kindTools(kind)[0]; selected=first?localizedTool(first,language):null;
    if(selected)chooseTool(selected);
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
      filters: [{ name: "Media", extensions: ["mp4","mov","mkv","avi","webm","m4v","mp3","wav","m4a","aac","flac","opus","jpg","jpeg","png","webp","bmp","tif","tiff"] }],
    });
    if (typeof path === "string") await loadMedia(path);
  }

  async function loadMedia(path: string) {
    if (busy) return;
    const dependency = ffmpegStatus ?? await refreshFfmpegStatus();
    if (!dependency.ready) {
      dependencyPanel = true;
      error = language === "tr" ? "FFmpeg ve FFprobe bulunamadı. Devam etmek için ikisini PATH içine kur." : "FFmpeg and FFprobe were not found. Install both on PATH to continue.";
      jobStatus = "ffmpeg missing";
      return;
    }
    error = "";
    output = "";
    jobStatus = "probing media";
    try {
      media = await invoke<MediaInfo>("probe_media", { path });
      activeKind = media.kind;
      mediaUrl = convertFileSrc(path);
      workspaceMode = "toolbox";
      toolboxCurrent = 0;
      toolboxPlaying = false;
      renderedImageUrl = "";
      qualityAnalysis = null;
      imageCompare = 50;
      imageZoom = 1;
      imageBaseScale = 1;
      imagePanX = 0;
      imagePanY = 0;
      imageDragging = false;
      imageViewInitialized = false;
      toolboxFilmstripUrl="";toolboxFilmstripLoading=false;
      const first = kindTools(activeKind)[0];
      if (first) chooseTool(first);
      progress = 0;
      jobStatus = "ready";
    } catch (reason) {
      media = null;
      selected = null;
      error = String(reason);
      jobStatus = "error";
    }
  }

  function closeMedia() {
    if (busy) return;
    media = null;
    selected = null;
    mediaUrl = "";
    output = "";
    error = "";
    progress = 0;
    jobStatus = "ready";
    toolboxCurrent = 0;
    toolboxPlaying = false;
    renderedImageUrl = "";
    qualityAnalysis = null;
    imageCompare = 50;
    imageZoom = 1;
    imageBaseScale = 1;
    imagePanX = 0;
    imagePanY = 0;
    imageDragging = false;
    imageViewInitialized = false;
    toolboxFilmstripUrl="";toolboxFilmstripLoading=false;
  }

  function seekToolbox(value: number) {
    if (!toolboxVideo) return;
    const duration = toolboxVideo.duration || media?.duration || 0;
    toolboxVideo.currentTime = Math.max(0, Math.min(duration, value));
    toolboxCurrent = toolboxVideo.currentTime;
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
    return Object.fromEntries(tool.fields.map((field) => [field.key, String(field.value)]));
  }

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
    if (tool.id === "interpolation" && media?.fps) {
      const fps = Number(params.fps);
      if (fps <= media.fps || fps > 2400 || fps % 60 !== 0) return `Interpolation FPS ${media.fps.toFixed(2)} değerinden yüksek, 60'ın katı ve en fazla 2400 olmalı.`;
    }
    if (tool.id === "frame_blend" && media?.fps && Number(params.fps) >= media.fps) return `Frame Blending hedefi ${media.fps.toFixed(2)} FPS değerinden düşük olmalı.`;
    if (["cut", "gif"].includes(tool.id) && Number(params.start) >= Number(params.end ?? Number(params.start) + Number(params.duration))) {
      if (tool.id === "cut") return "Bitiş zamanı başlangıçtan büyük olmalı.";
    }
    if (tool.id === "replace_audio" && !params.audio_path) return "Önce replacement audio dosyasını seç.";
    if (tool.id === "text" && !params.text.trim()) return "Yazı boş olamaz.";
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
    if (!media || !selected || busy) return;
    const validation = validate(selected);
    if (validation) { error = validation; return; }
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
      if (selected.id === "smart_quality") {
        qualityAnalysis = await invoke<QualityAnalysis>("analyze_quality", {
          request: {
            input: media.path,
            goal: String(toolField("goal")?.value ?? "balanced"),
            sample_duration: Number(toolField("sample_duration")?.value ?? 2),
          },
        });
        elapsed = qualityAnalysis.elapsed;
        progress = 100;
        jobStatus = language === "tr" ? "analiz tamamlandı" : "analysis complete";
        return;
      }
      if (selected.id === "file_hash") {
        hashResult = await invoke<string>("hash_file", { path: media.path });
        elapsed = (performance.now() - started) / 1000;
        progress = 100;
        jobStatus = language === "tr" ? "SHA-256 hesaplandı" : "SHA-256 calculated";
        return;
      }
      const result = await invoke<JobResult>("run_operation", {
        request: { input: media.path, operation: selected.id, params: paramsFrom(selected) },
      });
      output = result.output;
      if (media.kind === "image") {
        renderedImageUrl = `${convertFileSrc(result.output)}?render=${Date.now()}`;
        imageCompare = 50;
      }
      elapsed = result.elapsed;
      progress = 100;
      jobStatus = "complete";
    } catch (reason) {
      error = String(reason);
      elapsed = (performance.now() - started) / 1000;
      jobStatus = String(reason).toLowerCase().includes("cancel") ? "cancelled" : "failed";
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
    const path = await open({ multiple: false, filters: [{ name: "Audio", extensions: field.accept ?? [] }] });
    if (typeof path === "string") field.value = path;
  }

  function recommendation(): string {
    if (!selected || !media) return "";
    if (selected.id === "noise") return media.height && media.height <= 720 ? "Safe 3 · Recommended 6" : "Safe 4 · Recommended 8";
    if (selected.id === "deep_fry") return "Safe 2 · Recommended 4";
    if (selected.id === "distortion") return "Safe 1 · Recommended 3";
    if (selected.id === "color") return "Safe 1.03 / 1.05 / 0 · Recommended 1.08 / 1.10 / 0";
    if (selected.id === "corruption") return "Safe 1 · Recommended 2";
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
    if (ffmpegStatus?.ready) dependencyPanel = false;
    else if (showWhenMissing) dependencyPanel = true;
    return ffmpegStatus;
  }

  async function checkForUpdates(manual = true) {
    if (updateChecking || updateInstalling) return;
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
      updateInstalling = false;
    }
  }

  onMount(() => {
    const saved=localStorage.getItem("container-language");
    language=saved==="tr"||saved==="en"?saved:navigator.language.toLowerCase().startsWith("tr")?"tr":"en";
    document.documentElement.lang=language;
    const savedTheme=localStorage.getItem("container-theme");
    theme=savedTheme==="dark"||savedTheme==="light"?savedTheme:window.matchMedia("(prefers-color-scheme: light)").matches?"light":"dark";
    document.documentElement.dataset.theme=theme;
    void updateWindowIcon(theme);
    void getVersion().then((version) => appVersion = version).catch(() => {});
    void invoke<string | null>("startup_media_path").then((path) => { if (path) void loadMedia(path); }).catch(() => {});
    // Run both checks on every launch. The UI stays quiet unless the user
    // needs FFmpeg or a newer signed release is available.
    void (async () => {
      await refreshFfmpegStatus(true);
      await checkForUpdates(false);
    })();
    void invoke<string[]>("available_encoders").then((encoders) => {
      availableEncoders = encoders;
      if (selected?.id === "encode") {
        const source = kindTools(activeKind).find((tool) => tool.id === "encode");
        if (source) chooseTool(source);
      }
    }).catch(() => { availableEncoders = null; });
    const playerKeys = (event: KeyboardEvent) => {
      if (workspaceMode !== "toolbox" || media?.kind !== "video") return;
      const tag = (document.activeElement as HTMLElement | null)?.tagName;
      if (tag && ["INPUT", "SELECT", "TEXTAREA"].includes(tag)) return;
      if (event.code === "Space") { event.preventDefault(); toggleToolboxPlayer(); }
      else if (event.key === "ArrowLeft") seekToolbox(toolboxCurrent - 5);
      else if (event.key === "ArrowRight") seekToolbox(toolboxCurrent + 5);
    };
    window.addEventListener("keydown", playerKeys);
    listen<ProgressEvent>("container-progress", (event) => {
      progress = Math.max(0, Math.min(100, event.payload.percent));
      speed = event.payload.speed || "—";
      frame = event.payload.frame || "—";
      elapsed = event.payload.time;
      jobStatus = event.payload.status || jobStatus;
    }).then((fn) => unlistenProgress = fn);

    getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") dragActive = true;
      if (event.payload.type === "leave") dragActive = false;
      if (event.payload.type === "drop") {
        dragActive = false;
        const path = event.payload.paths[0];
        if (path) loadMedia(path);
      }
    }).then((fn) => unlistenDrop = fn);

    return () => { unlistenProgress?.(); unlistenDrop?.(); window.removeEventListener("keydown", playerKeys); };
  });

  function setLanguage(next:"tr"|"en"){
    if(next===language)return;
    const selectedId=selected?.id;
    language=next; localStorage.setItem("container-language",next); document.documentElement.lang=next;
    if(selectedId){const translated=kindTools(activeKind).find(tool=>tool.id===selectedId);if(translated)chooseTool(translated)}
  }
  function setTheme(next:"dark"|"light"){
    theme=next;localStorage.setItem("container-theme",next);document.documentElement.dataset.theme=next;void updateWindowIcon(next);
  }
  async function updateWindowIcon(next:"dark"|"light"){
    try{const response=await fetch(next==="dark"?"/logo-dark.png":"/logo-light.png");await getCurrentWindow().setIcon(await response.arrayBuffer())}catch{/* Browser preview has no Tauri window. */}
  }
</script>

<main class="shell" class:drag-active={dragActive}>
  <header class="topbar">
    <span class="brand"><img class="brand-logo" src={theme==="dark"?"/logo-dark.png":"/logo-light.png"} alt="CONTAINER logo">CONTAINER</span>
    {#if media}
      <span class="slash">/</span><span class="filename mono">{media.name}</span>
      <div class="chips mono">
        <span><b>dur</b>{formatDuration(media.duration)}</span><em>·</em>
        {#if media.width}<span><b>res</b>{media.width}×{media.height}</span><em>·</em>{/if}
        {#if media.fps}<span><b>fps</b>{media.fps.toFixed(3)}</span><em>·</em>{/if}
        <span><b>codec</b>{media.codec}</span><em>·</em><span><b>size</b>{formatBytes(media.size)}</span>
      </div>
      <div class="language-switch"><button class:active={language==="tr"} onclick={()=>setLanguage("tr")}>TR</button><button class:active={language==="en"} onclick={()=>setLanguage("en")}>EN</button><i></i><button class="theme-button" class:active={theme==="dark"} title={language==="tr"?"Koyu tema":"Dark theme"} aria-label={language==="tr"?"Koyu tema":"Dark theme"} onclick={()=>setTheme("dark")}>☾</button><button class="theme-button" class:active={theme==="light"} title={language==="tr"?"Açık tema":"Light theme"} aria-label={language==="tr"?"Açık tema":"Light theme"} onclick={()=>setTheme("light")}>☀</button></div>
      <button class="update-trigger" class:available={!!availableUpdate} class:checking={updateChecking} onclick={() => checkForUpdates(true)} title={language === "tr" ? "Güncellemeleri denetle" : "Check for updates"}><b>↻</b><span>{availableUpdate ? `v${availableUpdate.version}` : (language === "tr" ? "GÜNCELLE" : "UPDATE")}</span>{#if availableUpdate}<i></i>{/if}</button>
      <button class="ghost top-cancel" onclick={closeMedia} disabled={busy}>{t("close")}</button>
    {:else}
      <div class="language-switch landing-language"><button class:active={language==="tr"} onclick={()=>setLanguage("tr")}>TR</button><button class:active={language==="en"} onclick={()=>setLanguage("en")}>EN</button><i></i><button class="theme-button" class:active={theme==="dark"} title={language==="tr"?"Koyu tema":"Dark theme"} aria-label={language==="tr"?"Koyu tema":"Dark theme"} onclick={()=>setTheme("dark")}>☾</button><button class="theme-button" class:active={theme==="light"} title={language==="tr"?"Açık tema":"Light theme"} aria-label={language==="tr"?"Açık tema":"Light theme"} onclick={()=>setTheme("light")}>☀</button></div>
      <button class="update-trigger" class:available={!!availableUpdate} class:checking={updateChecking} onclick={() => checkForUpdates(true)} title={language === "tr" ? "Güncellemeleri denetle" : "Check for updates"}><b>↻</b><span>{availableUpdate ? `v${availableUpdate.version}` : (language === "tr" ? "GÜNCELLE" : "UPDATE")}</span>{#if availableUpdate}<i></i>{/if}</button>
    {/if}
  </header>

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

  {#if dependencyPanel && !updatePanel}
    <div class="update-layer dependency-layer">
      <button class="update-backdrop" aria-label={language === "tr" ? "FFmpeg bildirimini kapat" : "Close FFmpeg notice"} onclick={() => dependencyPanel = false}></button>
      <dialog class="update-dialog dependency-dialog panel" open aria-labelledby="dependency-title">
        <header><div><span class="status-dot missing"></span><h2 id="dependency-title">{language === "tr" ? "FFMPEG GEREKLİ" : "FFMPEG REQUIRED"}</h2></div><button onclick={() => dependencyPanel = false} aria-label={language === "tr" ? "Kapat" : "Close"}>×</button></header>
        <div class="dependency-message"><span>!</span><div><h3>{language === "tr" ? "MEDYA ARAÇLARI HENÜZ KULLANILAMAZ" : "MEDIA TOOLS ARE NOT READY YET"}</h3><p>{language === "tr" ? "CONTAINER, bilgisayarındaki FFmpeg ve FFprobe’yu kullanır. Resmî indirme sayfasından güncel full build’i kur, bin klasörünü PATH’e ekle ve ardından tekrar kontrol et." : "CONTAINER uses FFmpeg and FFprobe installed on your computer. Install a current full build from the official download page, add its bin folder to PATH, then check again."}</p></div></div>
        <footer><button class="ghost" onclick={() => dependencyPanel = false}>{language === "tr" ? "ŞİMDİ DEĞİL" : "NOT NOW"}</button><button class="dependency-check" onclick={() => refreshFfmpegStatus(true)} disabled={dependencyChecking}>{dependencyChecking ? "…" : (language === "tr" ? "TEKRAR KONTROL ET" : "CHECK AGAIN")}</button><button class="install-update" onclick={() => openUrl("https://ffmpeg.org/download.html#build-windows")}>{language === "tr" ? "FFMPEG İNDİR" : "DOWNLOAD FFMPEG"}</button></footer>
      </dialog>
    </div>
  {/if}

  {#if !media}
    <section class="landing">
      <button class="dropzone" class:active={dragActive} onclick={selectMedia} disabled={ffmpegStatus !== null && !ffmpegStatus.ready}>
        <span class="drop-icon">↳</span>
        <h1>{t("drop")}</h1>
        <p>{t("browse")}</p>
        <div class="format-row"><span>{t("video")}</span><span>{t("audio")}</span><span>{t("image")}</span></div>
      </button>
      {#if ffmpegStatus && !ffmpegStatus.ready}
        <section class="dependency-card">
          <div><span>!</span><div><h3>{language === "tr" ? "FFMPEG GEREKLİ" : "FFMPEG REQUIRED"}</h3><p>{language === "tr" ? "CONTAINER dosyaları işlemez; bilgisayarındaki FFmpeg ve FFprobe’yu kullanır. Full build kurup bin klasörünü PATH’e ekle, ardından uygulamayı yeniden başlat." : "CONTAINER uses FFmpeg and FFprobe installed on your computer. Install a full build, add its bin folder to PATH, then restart the app."}</p></div></div>
          <aside><button class="ghost" onclick={() => openUrl("https://ffmpeg.org/download.html#build-windows")}>{language === "tr" ? "FFMPEG İNDİR" : "DOWNLOAD FFMPEG"}</button><button class="dependency-check" onclick={() => refreshFfmpegStatus(true)} disabled={dependencyChecking}>{dependencyChecking ? "…" : (language === "tr" ? "TEKRAR KONTROL ET" : "CHECK AGAIN")}</button></aside>
        </section>
      {/if}
      <div class="landing-copy motto-only">
        <h2>{t("landingTitle")}</h2>
      </div>
      <footer><span class="status-dot" class:missing={ffmpegStatus !== null && !ffmpegStatus.ready}></span> ffmpeg {ffmpegStatus?.ready ? t("ready") : (dependencyChecking ? "checking" : "required")}</footer>
    </section>
  {:else}
    <nav class="mode-tabs">
        <button class:active={workspaceMode === "toolbox"} onclick={() => workspaceMode="toolbox"}>{t("toolbox")}</button>
        {#if media.kind === "video"}<button class:active={workspaceMode === "autocut"} onclick={() => workspaceMode="autocut"}>SMARTCUT</button>{/if}
        <button class:active={workspaceMode === "batch"} onclick={() => workspaceMode="batch"}>{language === "tr" ? "TOPLU" : "BATCH"}</button>
    </nav>
    {#if workspaceMode === "autocut" && media.kind === "video"}
      <AutoCutWorkspace {media} {mediaUrl} {language} />
    {:else if workspaceMode === "batch"}
      <BatchWorkspace initialPath={media.path} {language} {availableEncoders} />
    {:else}
    <section class="workspace">
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
        <div class="tool-scroll">
          {#each categories as [category, entries]}
            <section class="tool-group">
              <h4>{category}</h4>
              {#each entries as tool}
                <button class="tool-row" class:active={selected?.id === tool.id} onclick={() => chooseTool(tool)}>
                  <i class:blue={tool.accent === "blue"} class:green={tool.accent === "green"} class:purple={tool.accent === "purple"} class:red={tool.accent === "red"} class:yellow={tool.accent === "yellow"}></i>
                  <span><b>{tool.title}</b><small>{tool.description}</small></span><em>›</em>
                </button>
              {/each}
            </section>
          {/each}
        </div>
      </aside>

      <section class="center-stack" class:timeline-active={timelineTool}>
        <div class="preview panel">
          <div class="preview-head"><span>{t("preview")}</span><span class="mono">{t(media.kind).toUpperCase()} · {media.codec.toUpperCase()}</span></div>
          <div class="media-stage" class:ac-player={media.kind === "video"} class:toolbox-player={media.kind === "video"} bind:this={toolboxStage}>
            {#if media.kind === "video"}
              <!-- svelte-ignore a11y_media_has_caption -->
              <video bind:this={toolboxVideo} src={mediaUrl} preload="metadata" ontimeupdate={() => { if (toolboxVideo) toolboxCurrent = toolboxVideo.currentTime; }} onplay={() => toolboxPlaying = true} onpause={() => toolboxPlaying = false} onended={() => toolboxPlaying = false}></video>
              <div class="ac-controls">
                <input class="player-seek" style={`--seek-pct:${media.duration ? Math.min(100, toolboxCurrent / media.duration * 100) : 0}%`} aria-label="Video position" type="range" min="0" max={media.duration ?? 0} step="0.01" value={toolboxCurrent} oninput={(event) => seekToolbox(Number(event.currentTarget.value))}>
                <button onclick={() => seekToolbox(toolboxCurrent - 15)} title="15 seconds back">−15</button>
                <button class="play" onclick={toggleToolboxPlayer} title="Play / Pause">{toolboxPlaying ? "Ⅱ" : "▶"}</button>
                <button onclick={() => seekToolbox(toolboxCurrent + 15)} title="15 seconds forward">+15</button>
                <span class="ac-time mono">{playerTime(toolboxCurrent)} <i>/</i> {playerTime(media.duration ?? 0)}</span>
                <input class="volume" aria-label="Volume" type="range" min="0" max="1" step="0.05" bind:value={toolboxVolume} oninput={() => { if (toolboxVideo) toolboxVideo.volume = toolboxVolume; }}>
                <button onclick={fullscreenToolboxPlayer} title="Fullscreen">⛶</button>
              </div>
            {:else if media.kind === "audio"}
              <div class="audio-visual"><div class="disc">◉</div><h2>{media.name}</h2><p>{media.codec.toUpperCase()} · {formatDuration(media.duration)}</p><audio src={mediaUrl} controls></audio></div>
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
            {#if busy}<div class="busy-mask"><span></span><b>{jobStatus}</b><small>{progress.toFixed(1)}%</small></div>{/if}
          </div>
        </div>

        {#if timelineTool && media.duration}
          <div class="tool-timeline panel">
            <header><div><h3>TIMELINE</h3><p>{selected?.id === "screenshot" ? (language==="tr"?"kare zamanını seç":"choose frame time") : (language==="tr"?"çıktı aralığını seç":"choose export range")}</p></div><b class="mono">{selected?.id === "screenshot" ? playerTime(timelineBounds().start) : `${playerTime(timelineBounds().start)} — ${playerTime(timelineBounds().end)}`}</b></header>
            <div class="tool-wave" bind:this={toolboxTimeline} onclick={seekTimeline} role="presentation">
              {#if toolboxFilmstripUrl}<img class="filmstrip" src={toolboxFilmstripUrl} alt="Video filmstrip" draggable="false">{:else}<span class="wave-loading">{toolboxFilmstripLoading ? (language==="tr"?"video kareleri hazırlanıyor…":"building video frames…") : "—"}</span>{/if}
              {#if selected?.id === "screenshot"}
                <i class="timeline-point" role="slider" tabindex="0" aria-label="Timestamp" aria-valuemin="0" aria-valuemax={media.duration} aria-valuenow={timelineBounds().start} style:left={`${timelineBounds().start/media.duration*100}%`} onkeydown={(event)=>timelineHandleKey(event,"point")} onpointerdown={(event)=>startToolTimelineDrag(event,"point")}><b></b></i>
              {:else}
                <div class="timeline-selection" style:left={`${timelineBounds().start/media.duration*100}%`} style:width={`${Math.max(0,timelineBounds().end-timelineBounds().start)/media.duration*100}%`} onpointerdown={(event)=>startToolTimelineDrag(event,"range")} role="presentation">
                  <i class="timeline-edge left" role="slider" tabindex="0" aria-label="Start" aria-valuemin="0" aria-valuemax={timelineBounds().end} aria-valuenow={timelineBounds().start} onkeydown={(event)=>timelineHandleKey(event,"start")} onpointerdown={(event)=>startToolTimelineDrag(event,"start")}></i><i class="timeline-edge right" role="slider" tabindex="0" aria-label="End" aria-valuemin={timelineBounds().start} aria-valuemax={media.duration} aria-valuenow={timelineBounds().end} onkeydown={(event)=>timelineHandleKey(event,"end")} onpointerdown={(event)=>startToolTimelineDrag(event,"end")}></i>
                </div>
              {/if}
              <em class="timeline-playhead" style:left={`${toolboxCurrent/media.duration*100}%`}></em>
            </div>
            <div class="tool-ruler mono"><span>{playerTime(0)}</span><span>{playerTime(media.duration/4)}</span><span>{playerTime(media.duration/2)}</span><span>{playerTime(media.duration*3/4)}</span><span>{playerTime(media.duration)}</span></div>
          </div>
        {/if}

        <div class="job panel">
          <div class="job-head">
            <div><h3>{t("process")}</h3><p class="mono">{jobStatus}</p></div>
            <div class="job-stats mono"><span><b>{t("frame")}</b>{frame}</span><span><b>{t("speed")}</b>{speed}</span><span><b>{t("elapsed")}</b>{elapsed.toFixed(1)}s</span></div>
          </div>
          <div class="progress-track"><div style:width={`${progress}%`}></div></div>
          <div class="job-foot">
            <span class="mono">{output ? basename(output) : selected ? `${selected.title} ready to run` : "select a tool"}</span>
            <div>
              {#if output}<button class="ghost" onclick={() => revealItemInDir(output)}>{t("showOutput")}</button>{/if}
              {#if busy}<button class="danger" onclick={cancelJob}>{t("cancelJob")}</button>{/if}
            </div>
          </div>
          {#if error}<div class="error-box">{error}</div>{/if}
        </div>
      </section>

      <aside class="settings panel">
        {#if selected}
          <div class="pane-head"><div><h3>{t("parameters")}</h3><p>{selected.category}</p></div><button class="reset" onclick={() => { if (selected) chooseTool(kindTools(activeKind).find((item) => item.id === selected?.id) ?? selected); }}>{t("defaults")}</button></div>
          <div class="selected-title"><span class="index mono">{String(kindTools(activeKind).findIndex((tool) => tool.id === selected?.id) + 1).padStart(2,"0")}</span><div><h2>{selected.title}</h2><p>{selected.description}</p></div></div>
          <div class="explain"><b>{t("what")}</b><p>{selected.detail}</p></div>
          {#if recommendation()}<div class="recommend"><b>{t("forVideo")}</b><span>{recommendation()}</span></div>{/if}
          <div class="field-list">
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
              <ul>
                <li><b>H.264</b><span>{language === "tr" ? "En uyumlu ve çoğu kullanım için en güvenli seçim." : "Most compatible and the safest choice for general use."}</span></li>
                <li><b>HEVC</b><span>{language === "tr" ? "Aynı kalitede daha küçük olabilir; eski cihazlarda destek zayıftır." : "Can be smaller at the same quality, but older devices may not support it."}</span></li>
                <li><b>VP9 / AV1</b><span>{language === "tr" ? "Daha verimli fakat CPU ile oldukça yavaştır. MKV çıktısı kullanılır." : "More efficient but much slower on CPU. Output uses MKV."}</span></li>
              </ul>
              <small>{language === "tr" ? "Listede yalnızca bu bilgisayarda gerçek bir test karesi kodlayabilen encoder’lar gösterilir. Auto, 10-bit HEVC/VP9/AV1 kaynağını mümkün olduğunda 10-bit korur; H.264 için uyumlu 8-bit 4:2:0 kullanır." : "Only encoders that successfully encode a real test frame on this PC are shown. Auto preserves 10-bit for HEVC/VP9/AV1 when possible and uses compatible 8-bit 4:2:0 for H.264."}</small>
            </div>
          {/if}
          {#if ["encode","cut","remux","extract_audio"].includes(selected.id) && media.audio_tracks.length}
            <div class="codec-note"><b>{language === "tr" ? "SES PARÇALARI" : "AUDIO TRACKS"}</b><span>{language === "tr" ? `${media.audio_tracks.length} parça bulundu. Ana varsayılandır; Tümü parçaları ayrı tutar; Birleştir hepsini tek dengeli ses parçasında toplar.` : `${media.audio_tracks.length} track(s) found. Main is the default; All keeps tracks separate; Merge combines them into one normalized track.`}</span></div>
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
            {#if selected.id === "smart_quality"}
              <div class="quality-guide">
                <h4>{language === "tr" ? "PUANLARI NASIL OKUMALISIN?" : "HOW TO READ THE SCORES"}</h4>
                <p>{language === "tr" ? "VMAF, sıkıştırılmış görüntüyü kaynağa benzerlik açısından 0–100 arasında ölçer. Puan yükseldikçe görüntü kaynağa daha çok benzer. CRF yükseldikçe dosya genellikle küçülür fakat kalite düşer." : "VMAF measures how similar the compressed picture is to the source from 0–100. A higher score is closer to the source. A higher CRF usually makes a smaller file but lowers quality."}</p>
                <ul>
                  <li><b>95–100</b><span>{language === "tr" ? "Neredeyse kayıpsız görünür." : "Looks nearly transparent."}</span></li>
                  <li><b>90–94</b><span>{language === "tr" ? "Çoğu kullanım için çok iyi." : "Very good for most uses."}</span></li>
                  <li><b>85–89</b><span>{language === "tr" ? "İyi; hareket ve dokuda fark çıkabilir." : "Good; motion and textures may differ."}</span></li>
                  <li><b>&lt;85</b><span>{language === "tr" ? "Kalite kaybı belirginleşir." : "Quality loss becomes obvious."}</span></li>
                </ul>
                <small>{language === "tr" ? "Hız için örnekler en fazla 720p ölçülür. Boyut tahmini yalnızca video akışıdır; ses ve kapsayıcı birkaç MB ekleyebilir." : "For speed, samples are measured at up to 720p. The size estimate covers video only; audio and container overhead may add a few MB."}</small>
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
                  <p>{language === "tr" ? `Kalite / Sıkıştırma aracında CRF ${qualityAnalysis.recommended_crf} seçersen bu videoda hedeflediğin dengeye en yakın sonucu alman beklenir. Bu bir tahmindir; bütün video işlenmediği için kesin dosya boyutu sahnelere göre değişebilir.` : `Using CRF ${qualityAnalysis.recommended_crf} in Quality / Compression should give the closest match to your chosen goal for this video. It is an estimate; final size may vary because the complete video was not encoded.`}</p>
                </div>
              {/if}
            {/if}
            {#if selected.id === "file_hash" && hashResult}
              <div class="quality-result">
                <div class="quality-verdict"><span>SHA-256</span><code>{hashResult}</code></div>
                <button class="ghost" onclick={() => navigator.clipboard.writeText(hashResult)}>{language === "tr" ? "özeti kopyala" : "copy hash"}</button>
              </div>
            {/if}
          </div>
          <div class="run-box">
            <button class="run" onclick={runTool} disabled={busy}>▶ {selected.id === "smart_quality" ? (language === "tr" ? "kaliteyi analiz et" : "analyze quality") : selected.id === "file_hash" ? (language === "tr" ? "SHA-256 hesapla" : "calculate SHA-256") : `${t("render")} ${selected.title.toLocaleLowerCase(language)}`}</button>
            <p>{selected.id === "smart_quality" ? (language === "tr" ? "Yalnızca geçici örnekler oluşturulur; final çıktı ve kaynak değişmez." : "Only temporary samples are created; no final output or source changes.") : selected.id === "file_hash" ? (language === "tr" ? "Yalnızca dosya okunur; yeni dosya oluşturulmaz." : "The file is only read; no output file is created.") : t("outputNote")}</p>
          </div>
        {:else}
          <div class="empty-settings"><span>←</span><p>{t("selectTool")}</p></div>
        {/if}
      </aside>
    </section>
    {/if}
  {/if}
  {#if dragActive}<div class="drop-overlay"><span>{t("dropOpen")}</span></div>{/if}
</main>
