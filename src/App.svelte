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
  import { armCompletionSound, playCompletionSound } from "./lib/completionSound";
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
  interface FontOption { name:string; path:string }
  interface TextLayer { id:number; text:string; x:number; y:number; size:number; color:string; opacity:number; fontName:string; font_path:string; outline:number; outline_color:string; shadow:number; shadow_color:string; background:boolean; background_color:string; background_opacity:number; background_padding:number }
  interface EditorSnapshot { media:MediaInfo; mediaUrl:string; selected:Tool|null; activeKind:MediaKind; output:string; renderedImageUrl:string; colorEnabled:Record<string,boolean>; colorPreviewVisible:boolean; textLayers:TextLayer[]; activeTextId:number|null; qualityAnalysis:QualityAnalysis|null; customNumberFields:Record<string,boolean> }

  const windowIconBuffers = {
    dark: fetch("/logo-dark.png").then(response => response.arrayBuffer()),
    light: fetch("/logo-light.png").then(response => response.arrayBuffer())
  };

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
  let autoCutWorkspace:{undo:()=>void;redo:()=>void}|null=$state(null);
  let batchWorkspace:{undo:()=>void;redo:()=>void}|null=$state(null);
  let autoCutCanUndo=$state(false),autoCutCanRedo=$state(false);
  let batchCanUndo=$state(false),batchCanRedo=$state(false);
  let toolboxVideo: HTMLVideoElement | null = $state(null);
  let toolboxStage: HTMLElement | null = $state(null);
  let toolboxCanvas: HTMLElement | null = $state(null);
  let transformCanvasWidth = $state(0);
  let transformCanvasHeight = $state(0);
  let transformSourceBox: HTMLElement | null = $state(null);
  let toolboxCurrent = $state(0);
  let toolboxPlaying = $state(false);
  let toolboxVolume = $state(1);
  let renderedImageUrl = $state("");
  let qualityAnalysis: QualityAnalysis | null = $state(null);
  let qualityAnalyzing = $state(false);
  let hashResult = $state("");
  let colorEnabled: Record<string,boolean> = $state({});
  let colorPreviewVisible = $state(true);
  let textLayers: TextLayer[] = $state([]);
  let activeTextId: number | null = $state(null);
  let systemFonts: FontOption[] = $state([]);
  let fontsLoading = $state(false);
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
  let toolboxTimeline: HTMLElement | null = $state(null);
  let timelineHover = $state<number|null>(null);
  let language: "tr" | "en" = $state("en");
  let theme: "dark" | "light" = $state(document.documentElement.dataset.theme === "light" ? "light" : "dark");
  let availableEncoders: string[] | null = $state(null);
  let ffmpegStatus: FfmpegStatus | null = $state(null);
  let dependencyChecking = $state(false);
  let dependencyPanel = $state(false);
  let appVersion = $state("0.5.2");
  let availableUpdate: Update | null = $state(null);
  let updatePanel = $state(false);
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
  const kindTools=(kind:MediaKind)=>localizedForSection(kind,media?.kind??kind,language);
  const timelineTool = $derived.by(()=>selected ? ["cut","screenshot","gif"].includes(selected.id) : false);
  const canUndo = $derived(!busy&&(workspaceMode==="toolbox"?editHistoryIndex>0:workspaceMode==="autocut"?autoCutCanUndo:batchCanUndo));
  const canRedo = $derived(!busy&&(workspaceMode==="toolbox"?editHistoryIndex>=0&&editHistoryIndex<editHistory.length-1:workspaceMode==="autocut"?autoCutCanRedo:batchCanRedo));
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenDrop: UnlistenFn | null = null;

  function cloneEditorValue<T>(value:T):T{return JSON.parse(JSON.stringify(value)) as T}

  function captureEditorSnapshot():EditorSnapshot|null{
    if(!media)return null;
    return cloneEditorValue({media,mediaUrl,selected,activeKind,output,renderedImageUrl,colorEnabled,colorPreviewVisible,textLayers,activeTextId,qualityAnalysis,customNumberFields});
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
  function applyEditorSnapshot(snapshot:EditorSnapshot,direction:"undo"|"redo"){
    historyApplying=true;
    toolboxVideo?.pause();
    media=cloneEditorValue(snapshot.media);mediaUrl=snapshot.mediaUrl;selected=cloneEditorValue(snapshot.selected);activeKind=snapshot.activeKind;output=snapshot.output;renderedImageUrl=snapshot.renderedImageUrl;colorEnabled=cloneEditorValue(snapshot.colorEnabled);colorPreviewVisible=snapshot.colorPreviewVisible;textLayers=cloneEditorValue(snapshot.textLayers);activeTextId=snapshot.activeTextId;qualityAnalysis=cloneEditorValue(snapshot.qualityAnalysis);customNumberFields=cloneEditorValue(snapshot.customNumberFields);toolboxPlaying=false;toolboxCurrent=0;error="";jobStatus=language==="tr"?(direction==="undo"?"geri alındı":"ileri alındı"):(direction==="undo"?"undone":"redone");
    requestAnimationFrame(()=>historyApplying=false);
  }
  function undoEditor(){if(busy)return;if(workspaceMode==="autocut"){autoCutWorkspace?.undo();return}if(workspaceMode==="batch"){batchWorkspace?.undo();return}flushEditorSnapshot();if(editHistoryIndex<=0)return;editHistoryIndex-=1;applyEditorSnapshot(editHistory[editHistoryIndex],"undo")}
  function redoEditor(){if(busy)return;if(workspaceMode==="autocut"){autoCutWorkspace?.redo();return}if(workspaceMode==="batch"){batchWorkspace?.redo();return}if(editHistoryIndex>=editHistory.length-1)return;editHistoryIndex+=1;applyEditorSnapshot(editHistory[editHistoryIndex],"redo")}

  $effect(()=>{
    const snapshot=captureEditorSnapshot();
    if(!snapshot||historyApplying)return;
    const signature=snapshotSignature(snapshot);
    const timer=window.setTimeout(()=>{const current=captureEditorSnapshot();if(!historyApplying&&current&&signature===snapshotSignature(current))commitEditorSnapshot(snapshot)},280);
    return()=>window.clearTimeout(timer);
  });

  const categories = $derived.by(() => {
    const list = kindTools(activeKind).filter((tool) => `${tool.title} ${tool.description}`.toLocaleLowerCase(language).includes(search.toLocaleLowerCase(language)));
    const map = new Map<string, Tool[]>();
    for (const tool of list) map.set(tool.category, [...(map.get(tool.category) ?? []), tool]);
    const entries=[...map.entries()];
    if(activeKind==="image")entries.sort(([left],[right])=>left==="Utilities"?1:right==="Utilities"?-1:0);
    return entries;
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
  const rangePercent = (value:number,min:number,max:number) => Math.max(0,Math.min(100,(value-min)/Math.max(.000001,max-min)*100));
  const playerTime = (value: number) => {
    const safe = Math.max(0, Number(value) || 0);
    const hours = Math.floor(safe / 3600);
    const minutes = Math.floor((safe % 3600) / 60);
    const seconds = Math.floor(safe % 60);
    const millis = Math.floor((safe % 1) * 1000);
    return `${hours ? `${String(hours).padStart(2,"0")}:` : ""}${String(minutes).padStart(2,"0")}:${String(seconds).padStart(2,"0")}.${String(millis).padStart(3,"0")}`;
  };
  const basename = (path: string) => path.split(/[\\/]/).pop() ?? path;
  const upscaleStandards = [720,1080,1440,2160,4320];

  function upscaleDimensions(targetEdge:number){
    if(!media?.width||!media?.height)return null;
    const landscape=media.width>=media.height,ratio=media.width/media.height;
    const even=(value:number)=>Math.max(2,Math.round(value/2)*2);
    const width=landscape?even(targetEdge*ratio):even(targetEdge);
    const height=landscape?even(targetEdge):even(targetEdge/ratio);
    return {width,height,targetEdge};
  }
  function upscaleTargets(){
    if(!media?.width||!media?.height)return [];
    const sourceEdge=Math.min(media.width,media.height);
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

  function chooseTool(tool: Tool) {
    const changed = selected?.id !== tool.id;
    if(changed){
      colorEnabled={};
      colorPreviewVisible=true;
      textLayers=[];
      activeTextId=null;
    }
    selected = localizedTool(tool,language);
    if(selected.id==="text")void ensureSystemFonts();
    configureUpscale(selected);
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
  function resetSelectedTool(){
    if(!selected)return;
    const source=kindTools(activeKind).find(item=>item.id===selected?.id);
    if(source)selected=localizedTool(source,language);
    colorEnabled={};colorPreviewVisible=true;textLayers=[];activeTextId=null;qualityAnalysis=null;error="";
  }

  function toolField(key:string){return selected?.fields.find(field=>field.key===key)}
  function fieldLivesOnTimeline(key:string){return selected?.id==="cut"?["start","end"].includes(key):selected?.id==="screenshot"?key==="timestamp":selected?.id==="gif"?["start","duration"].includes(key):false}
  function fieldVisible(key:string){
    if(selected?.id==="transform" && key!=="crf") return false;
    if(selected?.id==="color" || selected?.id==="text") return false;
    if(selected?.id==="compression"&&!qualityAdvanced)return false;
    if(key==="audio_track") return String(toolField("audio_mode")?.value)==="selected";
    if(selected?.id==="cut"&&key==="crf") return String(toolField("cut_mode")?.value)!=="lossless";
    if(selected?.id==="speed"&&key==="crf") return String(toolField("speed_mode")?.value)!=="lossless_video";
    if(selected?.id==="potatoify"&&["fps","video_badness","audio_badness","shrink"].includes(key)) return String(toolField("profile")?.value)==="custom";
    if(selected?.id==="image_potatoify"&&["quality","times","scale"].includes(key)) return String(toolField("profile")?.value)==="custom";
    return true;
  }
  function toolNumber(key:string){return Number(toolField(key)?.value??0)}
  function setToolNumber(key:string,value:number){const field=toolField(key);if(field)field.value=Math.round(value*1000)/1000}
  function toolValue(key:string){return String(toolField(key)?.value??"")}
  function setToolValue(key:string,value:string){const field=toolField(key);if(field)field.value=value}
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
    const geometry=selected?.id==="transform"?transformPreviewStyle():neutralPreviewStyle();
    return `${geometry};${colorPreviewStyle()}`;
  }

  function activeText(){return textLayers.find(layer=>layer.id===activeTextId)??null}
  async function ensureSystemFonts(){
    if(systemFonts.length||fontsLoading)return systemFonts;
    fontsLoading=true;
    try{systemFonts=await invoke<FontOption[]>("list_system_fonts")}catch(reason){error=String(reason)}finally{fontsLoading=false}
    return systemFonts;
  }
  async function addTextLayer(){
    const fonts=await ensureSystemFonts();
    const font=fonts.find(item=>item.name.toLowerCase()==="impact")??fonts.find(item=>item.name.toLowerCase().startsWith("arial"))??fonts[0];
    if(!font){error=language==="tr"?"Bilgisayarda kullanılabilir font bulunamadı.":"No usable system font was found.";return}
    const layer:TextLayer={id:nextTextId++,text:`${language==="tr"?"Yazı":"Text"} ${textLayers.length+1}`,x:50,y:50,size:64,color:"#ffffff",opacity:100,fontName:font.name,font_path:font.path,outline:0,outline_color:"#000000",shadow:0,shadow_color:"#000000",background:false,background_color:"#000000",background_opacity:65,background_padding:12};
    textLayers=[...textLayers,layer];activeTextId=layer.id;
  }
  function updateTextLayer(patch:Partial<TextLayer>){textLayers=textLayers.map(layer=>layer.id===activeTextId?{...layer,...patch}:layer)}
  function removeTextLayer(id:number){textLayers=textLayers.filter(layer=>layer.id!==id);if(activeTextId===id)activeTextId=textLayers[0]?.id??null}
  function textLayerStyle(layer:TextLayer){
    const box=mediaDisplayBox();if(!box||!media?.width)return "display:none";
    const scale=box.width/media.width;
    const outline=Math.max(0,layer.outline*scale),shadow=Math.max(0,layer.shadow*scale),padding=Math.max(0,layer.background_padding*scale);
    return `left:${(box.stageWidth-box.width)/2+box.width*layer.x/100}px;top:${(box.stageHeight-box.height)/2+box.height*layer.y/100}px;font-size:${Math.max(8,layer.size*scale)}px;color:${layer.color};opacity:${layer.opacity/100};font-family:${JSON.stringify(layer.fontName)};-webkit-text-stroke:${outline}px ${layer.outline_color};text-shadow:${shadow?`${shadow}px ${shadow}px ${Math.max(1,shadow*.7)}px ${layer.shadow_color}`:"none"};background:${layer.background?hexWithAlpha(layer.background_color,layer.background_opacity):"transparent"};padding:${layer.background?`${padding}px`:"3px 8px"}`;
  }
  function chooseTextFont(path:string){const font=systemFonts.find(item=>item.path===path);if(font)updateTextLayer({fontName:font.name,font_path:font.path})}
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
  function startTextDrag(event:PointerEvent,layer:TextLayer,resizeDirection:-1|0|1=0){
    if(!toolboxCanvas||!media?.width)return;event.preventDefault();event.stopPropagation();activeTextId=layer.id;
    const box=mediaDisplayBox();if(!box)return;const mediaWidth=media.width;const startX=event.clientX,startY=event.clientY,origin={...layer};
    const move=(moveEvent:PointerEvent)=>{
      if(resizeDirection){const delta=(moveEvent.clientX-startX)/box.width*mediaWidth*resizeDirection;updateTextLayer({size:Math.max(8,Math.min(600,origin.size+delta))});return}
      updateTextLayer({x:Math.max(0,Math.min(100,origin.x+(moveEvent.clientX-startX)/box.width*100)),y:Math.max(0,Math.min(100,origin.y+(moveEvent.clientY-startY)/box.height*100))});
    };
    const stop=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",stop)};
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",stop);
  }
  const transformPresets = ["off","free","16:9","9:16","1:1","4:5","4:3","2:3","3:2","191:100"];
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
    if(!toolboxCanvas||!media?.width||!media?.height)return null;
    const stageWidth=transformCanvasWidth||toolboxCanvas.clientWidth,stageHeight=transformCanvasHeight||toolboxCanvas.clientHeight,ratio=media.width/media.height;
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
    if(!toolboxCanvas||!media?.width||!media?.height)return null;
    const stageWidth=transformCanvasWidth||toolboxCanvas.clientWidth,stageHeight=transformCanvasHeight||toolboxCanvas.clientHeight,rotation=Number(toolValue("rotate")),swapped=rotation===90||rotation===270;
    const ratio=swapped?media.height/media.width:media.width/media.height;
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
  function transformPreviewStyle(){
    if(selected?.id!=="transform")return "";
    const box=transformDisplayBox();if(!box)return "";
    const rotation=Number(toolValue("rotate"));
    const width=box.swapped?box.height:box.width,height=box.swapped?box.width:box.height;
    const flipX=toolValue("flip_h")==="true"?-1:1,flipY=toolValue("flip_v")==="true"?-1:1;
    return `position:absolute;left:50%;top:50%;width:${width}px;height:${height}px;transform:translate(-50%,-50%) scale(${flipX},${flipY}) rotate(${rotation}deg)`;
  }
  function setCropPreset(mode:string){
    setToolValue("crop_mode",mode);
    if(mode==="off"){setToolNumber("crop_x",0);setToolNumber("crop_y",0);setToolNumber("crop_w",100);setToolNumber("crop_h",100);return}
    if(mode==="free"){
      setToolNumber("crop_x",0);setToolNumber("crop_y",0);setToolNumber("crop_w",100);setToolNumber("crop_h",100);
      return;
    }
    const [rw,rh]=mode.split(":").map(Number),rotation=Number(toolValue("rotate"));
    const sourceRatio=rotation===90||rotation===270?(media?.height??9)/(media?.width??16):(media?.width??16)/(media?.height??9),target=rw/rh;
    let width=100,height=100;
    if(sourceRatio>target)width=target/sourceRatio*100;else height=sourceRatio/target*100;
    setToolNumber("crop_x",(100-width)/2);setToolNumber("crop_y",(100-height)/2);setToolNumber("crop_w",width);setToolNumber("crop_h",height);
  }
  function setTransformRotation(value:number){
    const mode=toolValue("crop_mode");setToolValue("rotate",String((value+360)%360));
    if(!["off","free"].includes(mode))setCropPreset(mode);
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
    const stop=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",stop)};
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",stop);
  }
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
  function seekTimeline(event:MouseEvent){
    const at=timelineAt(event.clientX);seekToolbox(at);
    if(selected?.id==="screenshot"){setToolNumber("timestamp",at);return}
    if(!media?.duration||!selected||!["cut","gif"].includes(selected.id))return;
    const bounds=timelineBounds(),span=Math.max(.01,bounds.end-bounds.start),start=Math.max(0,Math.min(media.duration-span,at)),end=Math.min(media.duration,start+span);
    if(selected.id==="gif"){setToolNumber("start",start);setToolNumber("duration",end-start)}else{setToolNumber("start",start);setToolNumber("end",end)}
  }
  function hoverTimeline(event:PointerEvent){if(!toolboxTimeline)return;const rect=toolboxTimeline.getBoundingClientRect();timelineHover=Math.max(0,Math.min(100,(event.clientX-rect.left)/rect.width*100))}
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
    const first=kindTools(kind)[0];
    if(first)chooseTool(first);else selected=null;
  }
  function setWorkspaceMode(mode:"toolbox"|"autocut"|"batch"){
    if(mode!==workspaceMode&&workspaceMode==="toolbox")resetSelectedTool();
    workspaceMode=mode;
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
      resetEditorHistory();
    } catch (reason) {
      media = null;
      selected = null;
      editHistory = [];
      editHistoryIndex = -1;
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
    editHistory = [];
    editHistoryIndex = -1;
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
    const params=Object.fromEntries(tool.fields.map((field) => [field.key, String(field.value)]));
    if(tool.id==="color")for(const key of ["brightness","contrast","saturation","gamma","hue","temperature","sharpen","blur","deband","vignette"])params[`${key}_enabled`]=String(colorOn(key));
    if(tool.id==="text")params.layers=JSON.stringify(textLayers);
    return params;
  }

  async function analyzeCompression(){
    if(!media||selected?.id!=="compression"||qualityAnalyzing||busy)return;
    const analyzedPath=media.path;
    qualityAnalyzing=true;error="";qualityAnalysis=null;jobStatus=language==="tr"?"kalite analiz ediliyor":"analyzing quality";
    try{
      const result=await invoke<QualityAnalysis>("analyze_quality",{request:{input:analyzedPath,goal:toolValue("goal")||"balanced",sample_duration:toolNumber("sample_duration")||2}});
      if(selected?.id==="compression"&&media?.path===analyzedPath){qualityAnalysis=result;elapsed=result.elapsed;progress=100;jobStatus=language==="tr"?"analiz tamamlandı":"analysis complete"}
    }catch(reason){error=String(reason);jobStatus="failed"}finally{qualityAnalyzing=false}
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
    if (tool.id === "interpolation" && media?.fps) {
      const fps = Number(params.fps);
      if (fps <= media.fps || fps > 2400 || fps % 60 !== 0) return `Interpolation FPS ${media.fps.toFixed(2)} değerinden yüksek, 60'ın katı ve en fazla 2400 olmalı.`;
    }
    if (tool.id === "frame_blend" && media?.fps && Number(params.fps) >= media.fps) return `Frame Blending hedefi ${media.fps.toFixed(2)} FPS değerinden düşük olmalı.`;
    if (tool.id === "upscale" && media?.width && media?.height && Number(params.target_edge) <= Math.min(media.width,media.height)) return language==="tr"?"Bu kaynak için daha yüksek bir standart çözünürlük hedefi yok.":"There is no higher standard resolution target for this source.";
    if (["cut", "gif"].includes(tool.id) && Number(params.start) >= Number(params.end ?? Number(params.start) + Number(params.duration))) {
      if (tool.id === "cut") return "Bitiş zamanı başlangıçtan büyük olmalı.";
    }
    if (tool.id === "replace_audio" && !params.audio_path) return "Önce replacement audio dosyasını seç.";
    if (tool.id === "text" && (!textLayers.length || textLayers.some(layer=>!layer.text.trim()))) return language==="tr"?"En az bir dolu yazı katmanı ekle.":"Add at least one non-empty text layer.";
    if(tool.id==="color"&&!Object.values(colorEnabled).some(Boolean)&&toolValue("denoise")==="off"&&toolValue("grayscale")!=="on"&&toolValue("deinterlace")==="off")return language==="tr"?"Önce en az bir video filtresini etkinleştir.":"Enable at least one video filter.";
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
      const result = await invoke<JobResult>("run_operation", {
        request: { input: media.path, operation: selected.id, params: paramsFrom(selected) },
      });
      output = result.output;
      const appliedTool=selected.id;
      if(["color","text"].includes(appliedTool)){
        media=await invoke<MediaInfo>("probe_media",{path:result.output});
        mediaUrl=`${convertFileSrc(result.output)}?applied=${Date.now()}`;
        toolboxCurrent=0;toolboxPlaying=false;
        const fresh=kindTools(activeKind).find(tool=>tool.id===appliedTool);
        if(fresh)selected=localizedTool(fresh,language);
        colorEnabled={};textLayers=[];activeTextId=null;
      }
      if (media.kind === "image") {
        renderedImageUrl = `${convertFileSrc(result.output)}?render=${Date.now()}`;
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
    theme=document.documentElement.dataset.theme==="light"?"light":"dark";
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
      if (!media) return;
      if (event.ctrlKey && !event.altKey && event.key.toLowerCase() === "z") {
        event.preventDefault();
        if (event.shiftKey) redoEditor(); else undoEditor();
        return;
      }
      if (workspaceMode !== "toolbox") return;
      if (media.kind !== "video" || event.ctrlKey || event.altKey || event.metaKey) return;
      const tag = (document.activeElement as HTMLElement | null)?.tagName;
      if (tag && ["INPUT", "SELECT", "TEXTAREA"].includes(tag)) return;
      if (event.code === "Space") { event.preventDefault(); toggleToolboxPlayer(); }
      else if (event.key === "ArrowLeft") seekToolbox(toolboxCurrent - 5);
      else if (event.key === "ArrowRight") seekToolbox(toolboxCurrent + 5);
    };
    const blockBrowserMenu = (event: MouseEvent) => event.preventDefault();
    window.addEventListener("keydown", playerKeys);
    window.addEventListener("contextmenu", blockBrowserMenu);
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

    return () => { unlistenProgress?.(); unlistenDrop?.(); window.removeEventListener("keydown", playerKeys); window.removeEventListener("contextmenu", blockBrowserMenu); };
  });

  function setLanguage(next:"tr"|"en"){
    if(next===language)return;
    const selectedId=selected?.id;
    language=next; localStorage.setItem("container-language",next); document.documentElement.lang=next;
    if(selectedId){const translated=kindTools(activeKind).find(tool=>tool.id===selectedId);if(translated)chooseTool(translated)}
  }
  function setTheme(next:"dark"|"light"){
    if(next===theme)return;
    const root=document.documentElement;
    root.classList.add("theme-changing");
    root.dataset.theme=next;
    root.style.colorScheme=next;
    document.querySelector<HTMLMetaElement>('meta[name="theme-color"]')?.setAttribute("content",next==="light"?"#f3f5f8":"#09090b");
    theme=next;localStorage.setItem("container-theme",next);void updateWindowIcon(next);
    requestAnimationFrame(()=>requestAnimationFrame(()=>root.classList.remove("theme-changing")));
  }
  async function updateWindowIcon(next:"dark"|"light"){
    try{await getCurrentWindow().setIcon((await windowIconBuffers[next]).slice(0))}catch{/* Browser preview has no Tauri window. */}
  }
</script>

<main class="shell" class:drag-active={dragActive}>
  <header class="topbar">
    <span class="brand"><span class="brand-logo-stack" aria-hidden="true"><img class="brand-logo brand-logo-dark" src="/logo-dark.png" alt="" decoding="sync"><img class="brand-logo brand-logo-light" src="/logo-light.png" alt="" decoding="sync"></span>CONTAINER</span>
    {#if media}
      <span class="slash">/</span><span class="filename mono">{media.name}</span>
      <div class="chips mono">
        <span><b>dur</b>{formatDuration(media.duration)}</span><em>·</em>
        {#if media.width}<span><b>res</b>{media.width}×{media.height}</span><em>·</em>{/if}
        {#if media.fps}<span><b>fps</b>{media.fps.toFixed(3)}</span><em>·</em>{/if}
        <span><b>codec</b>{media.codec}</span><em>·</em><span><b>size</b>{formatBytes(media.size)}</span>
      </div>
      <div class="history-actions"><button onclick={undoEditor} disabled={!canUndo} title={language==="tr"?"Geri al · Ctrl+Z":"Undo · Ctrl+Z"} aria-label={language==="tr"?"Geri al":"Undo"}>↶</button><button onclick={redoEditor} disabled={!canRedo} title={language==="tr"?"İleri al · Ctrl+Shift+Z":"Redo · Ctrl+Shift+Z"} aria-label={language==="tr"?"İleri al":"Redo"}>↷</button></div>
      <div class="language-switch theme-only"><button class="theme-button" class:active={theme==="dark"} title={language==="tr"?"Koyu tema":"Dark theme"} aria-label={language==="tr"?"Koyu tema":"Dark theme"} onclick={()=>setTheme("dark")}>☾</button><button class="theme-button" class:active={theme==="light"} title={language==="tr"?"Açık tema":"Light theme"} aria-label={language==="tr"?"Açık tema":"Light theme"} onclick={()=>setTheme("light")}>☀</button></div>
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
        <button class:active={workspaceMode === "toolbox"} onclick={() => setWorkspaceMode("toolbox")}>{t("toolbox")}</button>
        {#if media.kind === "video"}<button class:active={workspaceMode === "autocut"} onclick={() => setWorkspaceMode("autocut")}>SMARTCUT</button>{/if}
        <button class:active={workspaceMode === "batch"} onclick={() => setWorkspaceMode("batch")}>{language === "tr" ? "TOPLU" : "BATCH"}</button>
    </nav>
    {#if workspaceMode === "autocut" && media.kind === "video"}
      <AutoCutWorkspace bind:this={autoCutWorkspace} {media} {mediaUrl} {language} onhistorychange={(undo:boolean,redo:boolean)=>{autoCutCanUndo=undo;autoCutCanRedo=redo}} />
    {:else if workspaceMode === "batch"}
      <BatchWorkspace bind:this={batchWorkspace} initialPath={media.path} {language} {availableEncoders} onhistorychange={(undo:boolean,redo:boolean)=>{batchCanUndo=undo;batchCanRedo=redo}} />
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
              <div class="video-canvas" bind:this={toolboxCanvas}>
              <!-- svelte-ignore a11y_media_has_caption -->
              <video bind:this={toolboxVideo} style={previewVideoStyle()} src={mediaUrl} preload="metadata" ontimeupdate={() => { if (toolboxVideo) toolboxCurrent = toolboxVideo.currentTime; }} onplay={() => toolboxPlaying = true} onpause={() => toolboxPlaying = false} onended={() => toolboxPlaying = false}></video>
              {#if selected?.id === "transform" && toolValue("crop_mode") !== "off"}
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
              {#if selected?.id === "text"}
                <div class="text-preview-layer">
                  {#each textLayers as layer (layer.id)}
                    <button class="preview-text" class:active={activeTextId===layer.id} style={textLayerStyle(layer)} onclick={()=>activeTextId=layer.id} onpointerdown={(event)=>startTextDrag(event,layer)}>
                      <i class="text-size-handle left" role="presentation" aria-label="Resize text from left" onpointerdown={(event)=>startTextDrag(event,layer,-1)}></i>
                      <span>{layer.text}</span>
                      <i class="text-size-handle right" role="presentation" aria-label="Resize text from right" onpointerdown={(event)=>startTextDrag(event,layer,1)}></i>
                    </button>
                  {/each}
                </div>
              {/if}
              </div>
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
            {:else if selected?.id === "transform"}
              <div class="transform-image-canvas" bind:this={toolboxCanvas}>
                <img style={transformPreviewStyle()} src={mediaUrl} alt={media.name} draggable="false" />
                {#if toolValue("crop_mode") !== "off"}
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
            {#if busy}<div class="busy-mask"><span></span><b>{jobStatus}</b><small>{progress.toFixed(1)}%</small></div>{/if}
          </div>
        </div>

        {#if timelineTool && media.duration}
          <div class="tool-timeline panel">
            <header><div><h3>TIMELINE</h3><p>{selected?.id === "screenshot" ? (language==="tr"?"kare zamanını seç":"choose frame time") : (language==="tr"?"çıktı aralığını seç":"choose export range")}</p></div><b class="mono">{selected?.id === "screenshot" ? playerTime(timelineBounds().start) : `${playerTime(timelineBounds().start)} — ${playerTime(timelineBounds().end)}`}</b></header>
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
          <div class="pane-head"><div><h3>{t("parameters")}</h3><p>{selected.category}</p></div><button class="reset" onclick={resetSelectedTool}>{t("defaults")}</button></div>
          <div class="selected-title"><span class="index mono">{String(kindTools(activeKind).findIndex((tool) => tool.id === selected?.id) + 1).padStart(2,"0")}</span><div><h2>{selected.title}</h2><p>{selected.description}</p></div></div>
          {#if !["transform","text","color"].includes(selected.id)}<div class="explain"><b>{t("what")}</b><p>{selected.detail}</p></div>{/if}
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
                <div class="text-tabs">{#each textLayers as layer,index (layer.id)}<button class:active={activeTextId===layer.id} onclick={()=>activeTextId=layer.id}>{index+1}. {layer.text||"—"}</button>{/each}</div>
                {@const layer=activeText()}
                {#if layer}
                  <label class="field"><span>{language==="tr"?"Yazı":"Text"}</span><input type="text" value={layer.text} oninput={(event)=>updateTextLayer({text:event.currentTarget.value})}></label>
                  <label class="field"><span>{language==="tr"?"Font":"Font"}</span><select value={layer.font_path} onchange={(event)=>chooseTextFont(event.currentTarget.value)}>{#each systemFonts as font}<option value={font.path}>{font.name}</option>{/each}</select></label>
                  <div class="text-color-editor">
                    <span>{language==="tr"?"Renk":"Color"}</span>
                    <div class="text-color-row"><i style:background={layer.color}></i><input aria-label="Hex color" value={layer.color} maxlength="7" onchange={(event)=>setTextColor(event.currentTarget.value)}></div>
                    <div class="text-swatches">{#each textColors as color}<button class:active={layer.color===color} style:background={color} aria-label={`Use ${color}`} onclick={()=>setTextColor(color)}></button>{/each}</div>
                  </div>
                  <label class="field"><span>{language==="tr"?"Yazı boyutu":"Font size"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.size,8,600)}%`} min="8" max="600" step="1" value={layer.size} oninput={(event)=>updateTextLayer({size:Number(event.currentTarget.value)})}><small class="hint">{Math.round(layer.size)} px</small></label>
                  <label class="field"><span>{language==="tr"?"Opaklık":"Opacity"}<small>%</small></span><input type="range" style={`--range-pct:${rangePercent(layer.opacity,0,100)}%`} min="0" max="100" step="1" value={layer.opacity} oninput={(event)=>updateTextLayer({opacity:Number(event.currentTarget.value)})}><small class="hint">{Math.round(layer.opacity)}%</small></label>
                  <details class="text-style-options">
                    <summary>{language==="tr"?"Kontur, gölge ve arka plan":"Outline, shadow & background"}</summary>
                    <label class="field"><span>{language==="tr"?"Kontur":"Outline"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.outline,0,20)}%`} min="0" max="20" step="1" value={layer.outline} oninput={(event)=>updateTextLayer({outline:Number(event.currentTarget.value)})}><small class="hint">{layer.outline}px</small></label>
                    <label class="field"><span>{language==="tr"?"Kontur rengi":"Outline color"}</span><input type="text" value={layer.outline_color} maxlength="7" onchange={(event)=>/^#[0-9a-f]{6}$/i.test(event.currentTarget.value)&&updateTextLayer({outline_color:event.currentTarget.value})}></label>
                    <label class="field"><span>{language==="tr"?"Gölge":"Shadow"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.shadow,0,30)}%`} min="0" max="30" step="1" value={layer.shadow} oninput={(event)=>updateTextLayer({shadow:Number(event.currentTarget.value)})}><small class="hint">{layer.shadow}px</small></label>
                    <label class="color-toggle"><input type="checkbox" checked={layer.background} onchange={(event)=>updateTextLayer({background:event.currentTarget.checked})}><span>{language==="tr"?"Arka plan kutusu":"Background box"}</span></label>
                    {#if layer.background}
                      <label class="field"><span>{language==="tr"?"Arka plan rengi":"Background color"}</span><input type="text" value={layer.background_color} maxlength="7" onchange={(event)=>/^#[0-9a-f]{6}$/i.test(event.currentTarget.value)&&updateTextLayer({background_color:event.currentTarget.value})}></label>
                      <label class="field"><span>{language==="tr"?"Arka plan opaklığı":"Background opacity"}<small>%</small></span><input type="range" style={`--range-pct:${rangePercent(layer.background_opacity,0,100)}%`} min="0" max="100" step="1" value={layer.background_opacity} oninput={(event)=>updateTextLayer({background_opacity:Number(event.currentTarget.value)})}><small class="hint">{layer.background_opacity}%</small></label>
                      <label class="field"><span>{language==="tr"?"İç boşluk":"Padding"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.background_padding,0,80)}%`} min="0" max="80" step="1" value={layer.background_padding} oninput={(event)=>updateTextLayer({background_padding:Number(event.currentTarget.value)})}><small class="hint">{layer.background_padding}px</small></label>
                    {/if}
                  </details>
                  <button class="remove-text" onclick={()=>removeTextLayer(layer.id)}>{language==="tr"?"Seçili yazıyı kaldır":"Remove selected text"}</button>
                  <p class="text-help">{language==="tr"?"Yazıyı önizlemede sürükle; iki yanındaki tutamaçlardan boyutlandır.":"Drag text in the preview; resize it from either side handle."}</p>
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
              <p><b>Quality / {encoderQualityMode()} {toolNumber("crf")}:</b> {language==="tr"?(encoderQualityMode()==="CRF"&&toolNumber("crf")===0?"CPU kodlamada gerçek kayıpsız; dosya çok büyük olur.":"Yüksek kalite ayarıdır, gerçek kayıpsız değildir. Sayı düştükçe kalite ve dosya boyutu artar."):(encoderQualityMode()==="CRF"&&toolNumber("crf")===0?"Truly lossless for CPU encoding; file size will be very large.":"A high-quality setting, not mathematically lossless. Lower values increase quality and file size.")}</p>
              <ul>
                <li><b>H.264</b><span>{language === "tr" ? "En uyumlu ve çoğu kullanım için en güvenli seçim." : "Most compatible and the safest choice for general use."}</span></li>
                <li><b>HEVC</b><span>{language === "tr" ? "Aynı kalitede daha küçük olabilir; eski cihazlarda destek zayıftır." : "Can be smaller at the same quality, but older devices may not support it."}</span></li>
                <li><b>VP9 / AV1</b><span>{language === "tr" ? "Daha verimli fakat CPU ile oldukça yavaştır. MKV çıktısı kullanılır." : "More efficient but much slower on CPU. Output uses MKV."}</span></li>
              </ul>
              <small>{language === "tr" ? "Listede yalnızca bu bilgisayarda gerçek bir test karesi kodlayabilen encoder’lar gösterilir. Auto, 10-bit HEVC/VP9/AV1 kaynağını mümkün olduğunda 10-bit korur; H.264 için uyumlu 8-bit 4:2:0 kullanır." : "Only encoders that successfully encode a real test frame on this PC are shown. Auto preserves 10-bit for HEVC/VP9/AV1 when possible and uses compatible 8-bit 4:2:0 for H.264."}</small>
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
          {#if selected.id === "transform"}
            <div class="transform-controls">
              <section>
                <header><b>CROP</b><small>{toolValue("crop_mode")==="off" ? (language==="tr"?"kapalı":"off") : `${toolNumber("crop_w").toFixed(1)}% × ${toolNumber("crop_h").toFixed(1)}%`}</small></header>
                <div class="transform-options crop-options">
                  {#each transformPresets as preset}<button class:active={toolValue("crop_mode")===preset} onclick={()=>setCropPreset(preset)}>{preset==="191:100"?"1.91:1":preset.toUpperCase()}</button>{/each}
                </div>
                {#if toolValue("crop_mode")!=="off"}<p>{language==="tr"?"Kadrajı önizlemede sürükle; kenar ve köşelerden serbestçe boyutlandır.":"Drag the frame in the preview; resize freely from its edges and corners."}</p>{/if}
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
                  <p>{language==="tr"?"Tam boyut, seçtiğin kadrajı bu ölçülere ölçekler; oranlar farklıysa görüntü esneyebilir.":"Exact size scales the crop to these dimensions; mismatched ratios may stretch the image."}</p>
                {/if}
              </section>
              {#if media.kind === "image"}
                <section>
                  <header><b>{language==="tr"?"ÇIKTI FORMATI":"OUTPUT FORMAT"}</b></header>
                  <div class="transform-options three"><button class:active={toolValue("format")==="png"} onclick={()=>setToolValue("format","png")}>PNG · LOSSLESS</button><button class:active={toolValue("format")==="webp"} onclick={()=>setToolValue("format","webp")}>WEBP · LOSSLESS</button><button class:active={toolValue("format")==="jpg"} onclick={()=>setToolValue("format","jpg")}>JPEG</button></div>
                </section>
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
            {#if selected.id === "compression"}
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
          </div>
          <div class="run-box">
            <button class="run" onclick={runTool} disabled={busy||qualityAnalyzing}>▶ {selected.id === "file_hash" ? (language === "tr" ? "SHA-256 hesapla" : "calculate SHA-256") : `${t("render")} ${selected.title.toLocaleLowerCase(language)}`}</button>
            <p>{selected.id === "file_hash" ? (language === "tr" ? "Yalnızca dosya okunur; yeni dosya oluşturulmaz." : "The file is only read; no output file is created.") : t("outputNote")}</p>
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
