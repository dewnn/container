<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { open } from "@tauri-apps/plugin-dialog";

  interface MediaInfo { path:string; name:string; duration:number|null; fps:number|null; audio_codec:string|null; start_timecode:string|null; kind?:string }
  interface Cut { start:number; end:number; enabled:boolean }
  interface Analysis { cuts:Cut[]; waveform:number[]; duration:number }
  interface Result { output:string; elapsed:number }
  interface Progress { percent:number; status:string }
  interface Recommendation { threshold:number; min_silence:number; min_speech:number; padding:number; noise_floor_db:number; speech_level_db:number }
  interface LinkedTrack { path:string; offset:number; timecode:boolean }

  let { media, mediaUrl, language }:{ media:MediaInfo; mediaUrl:string; language:"tr"|"en" } = $props();
  const words:Record<"tr"|"en",Record<string,string>>={
    tr:{detection:"ALGILAMA",silenceParams:"sessizlik parametreleri",threshold:"EŞİK",minSilence:"EN AZ SESSİZLİK",minSpeech:"EN AZ KONUŞMA",padding:"KENAR PAYI",help:"Eşik, Silero modelinin bir parçayı konuşma sayması için gereken güven değeridir. Yükseldikçe daha çok yer kesilir. Kenar payı kelimelerin başını ve sonunu korur.",listen:"BAŞKA BİR KAYDI DİNLE",camera:"kamera sesini kullan",analyzing:"ANALİZ EDİLİYOR…",detect:"SESSİZLİĞİ ALGILA",export:"DIŞA AKTAR",kept:"tutuldu",removed:"kaldırıldı",format:"FORMAT",quality:"KALİTE",resolution:"ÇÖZÜNÜRLÜK",high:"Yüksek",medium:"Orta",small:"Küçük",source:"Kaynak",linked:"BAĞLANTILI KAYITLAR",add:"+ EKLE",exporting:"AKTARILIYOR",cancelExport:"AKTARMAYI İPTAL ET",showOutput:"ÇIKTIYI GÖSTER",timeline:"ZAMAN ÇİZELGESİ",waveform:"ses dalgası oluşturuluyor…",skipping:"kesimler atlanıyor",playingAll:"tümü oynatılıyor",cuts:"KESİMLER",editable:"düzenlenebilir tutma bölgeleri",regions:"bölge",output:"çıktı",keep:"TUT",off:"KAPALI",empty:"Kesim listesini oluşturmak için algılamayı çalıştır.",noise:"gürültü",voice:"konuşma"},
    en:{detection:"DETECTION",silenceParams:"silence parameters",threshold:"THRESHOLD",minSilence:"MIN SILENCE",minSpeech:"MIN SPEECH",padding:"PADDING",help:"Threshold is the confidence Silero needs to count a segment as speech. Raising it cuts more. Padding protects word beginnings and endings.",listen:"LISTEN TO ANOTHER TRACK",camera:"use camera audio",analyzing:"ANALYZING…",detect:"DETECT SILENCE",export:"EXPORT",kept:"kept",removed:"removed",format:"FORMAT",quality:"QUALITY",resolution:"RESOLUTION",high:"High",medium:"Medium",small:"Small",source:"Source",linked:"LINKED TRACKS",add:"+ ADD",exporting:"EXPORTING",cancelExport:"CANCEL EXPORT",showOutput:"SHOW OUTPUT",timeline:"TIMELINE",waveform:"building waveform…",skipping:"skipping cuts",playingAll:"playing all",cuts:"CUTS",editable:"editable keep regions",regions:"regions",output:"output",keep:"KEEP",off:"OFF",empty:"Run detection to build a cut list.",noise:"noise",voice:"voice"}
  };
  const t=(key:string)=>words[language][key]??key;
  let video:HTMLVideoElement|null = $state(null);
  let stage:HTMLElement|null = $state(null);
  let threshold = $state(0.50);
  let minSilence = $state(0.25);
  let minSpeech = $state(0.15);
  let padding = $state(0.12);
  let cuts:Cut[] = $state([]);
  let waveform:number[] = $state([]);
  let waveformLoading = $state(true);
  let autoTuning = $state(false);
  let autoSummary = $state("");
  let current = $state(0);
  let playing = $state(false);
  let volume = $state(1);
  let skipRemoved = $state(true);
  let analyzing = $state(false);
  let exporting = $state(false);
  let progress = $state(0);
  let error = $state("");
  let output = $state("");
  let exportFormat = $state("mp4");
  let quality = $state("medium");
  let resolution = $state("source");
  let analysisInput = $state("");
  let linkedTracks:LinkedTrack[] = $state([]);
  let unlisten:UnlistenFn|null = null;
  let hasAnalyzed = $state(false);
  let lastAnalyzedKey = $state("");
  let waveEl:HTMLElement|null = $state(null);
  let navEl:HTMLElement|null = $state(null);
  let viewStart = $state(0);
  let viewEnd = $state(0);

  const duration = $derived(media.duration ?? 0);
  const kept = $derived(cuts.filter(c => c.enabled).reduce((n,c)=>n+Math.max(0,c.end-c.start),0));
  const removed = $derived(Math.max(0,duration-kept));
  const settingsKey = $derived(`${threshold}|${minSilence}|${minSpeech}|${padding}|${analysisInput}`);
  const viewSpan = $derived(Math.max(.001,viewEnd-viewStart));
  const zoomLevel = $derived(duration&&viewSpan ? duration/viewSpan : 1);
  const segments = $derived.by(()=>{
    const list:{start:number;end:number;kind:"keep"|"remove";enabled:boolean;index:number}[]=[]; let cursor=0;
    cuts.forEach((cut,index)=>{if(cut.start>cursor)list.push({start:cursor,end:cut.start,kind:"remove",enabled:false,index:-1});list.push({start:cut.start,end:cut.end,kind:"keep",enabled:cut.enabled,index});cursor=Math.max(cursor,cut.end)});
    if(cursor<duration)list.push({start:cursor,end:duration,kind:"remove",enabled:false,index:-1});
    return list.filter(s=>s.end>viewStart&&s.start<viewEnd);
  });
  const pct = (value:number) => duration ? `${Math.max(0,Math.min(100,value/duration*100))}%` : "0%";
  const time = (value:number) => {
    const safe=Math.max(0,Number(value)||0), h=Math.floor(safe/3600), m=Math.floor(safe%3600/60), s=Math.floor(safe%60), ms=Math.floor((safe%1)*1000);
    return `${h?String(h).padStart(2,"0")+":" : ""}${String(m).padStart(2,"0")}:${String(s).padStart(2,"0")}.${String(ms).padStart(3,"0")}`;
  };

  function seek(value:number) { if(video){ video.currentTime=Math.max(0,Math.min(duration,value)); current=video.currentTime; } }
  function togglePlay(){ if(!video)return; if(video.paused)video.play().catch(()=>{});else video.pause(); }
  function findGap(t:number){
    const active=cuts.filter(c=>c.enabled).sort((a,b)=>a.start-b.start);
    for(let i=0;i<active.length;i++){
      const c=active[i];
      if(t < c.start-0.001) return c.start;
      if(t>=c.start-0.001 && t<=c.end-0.055) return null;
      if(t>c.end-0.055 && i+1<active.length && t<active[i+1].start) return active[i+1].start;
    }
    return null;
  }
  function onTime(){
    if(!video)return; current=video.currentTime;
    if(skipRemoved && !video.paused){ const target=findGap(current); if(target!==null && Math.abs(target-current)>.03) seek(target); }
    if(!video.paused&&viewSpan<duration-.001&&(current>viewEnd-viewSpan*.06||current<viewStart)){[viewStart,viewEnd]=clampWindow(current-viewSpan*.12,current+viewSpan*.88)}
  }
  function scrub(event:MouseEvent){ const el=event.currentTarget as HTMLElement; const rect=el.getBoundingClientRect(); seek((event.clientX-rect.left)/rect.width*duration); }
  function scrubView(event:MouseEvent){if(!waveEl)return;const rect=waveEl.getBoundingClientRect();seek(viewStart+(event.clientX-rect.left)/rect.width*viewSpan)}
  function updateCut(index:number,key:"start"|"end",value:number){
    const next=cuts.map(c=>({...c})); const cut=next[index];
    const proposed=Math.max(0,Math.min(duration,Number(value)||0));
    if(key==="start")cut.start=Math.max(index?next[index-1].end:0,Math.min(cut.end-.05,proposed));
    else cut.end=Math.min(index<next.length-1?next[index+1].start:duration,Math.max(cut.start+.05,proposed));
    cuts=next;
  }
  function addCut(){ const start=Math.max(0,current-1),end=Math.min(duration,current+1);cuts=[...cuts,{start,end,enabled:true}].sort((a,b)=>a.start-b.start); }
  function jumpCut(c:Cut){seek(c.start);}
  function previousCut(){const list=cuts.filter(c=>c.enabled&&c.start<current-.05);seek(list.at(-1)?.start??0)}
  function nextCut(){const c=cuts.find(c=>c.enabled&&c.start>current+.05);seek(c?.start??duration)}

  async function analyze(){
    analyzing=true;error="";output="";
    try{const result=await invoke<Analysis>("analyze_autocut",{request:{input:media.path,analysis_input:analysisInput||null,threshold,min_silence:minSilence,min_speech:minSpeech,padding}});cuts=result.cuts;waveform=result.waveform;hasAnalyzed=true;lastAnalyzedKey=settingsKey;}
    catch(reason){error=String(reason)}finally{analyzing=false}
  }
  async function autoTune(){
    autoTuning=true;error="";autoSummary="";
    try{
      const result=await invoke<Recommendation>("recommend_autocut_settings",{path:analysisInput||media.path});
      threshold=result.threshold;minSilence=result.min_silence;minSpeech=result.min_speech;padding=result.padding;
      autoSummary=`${t("noise")} ${result.noise_floor_db.toFixed(1)} dB · ${t("voice")} ${result.speech_level_db.toFixed(1)} dB`;
      await tick(); await analyze();
    }catch(reason){error=String(reason)}finally{autoTuning=false}
  }
  async function exportCuts(){
    exporting=true;progress=0;error="";output="";
    try{const result=await invoke<Result>("export_autocut",{request:{input:media.path,cuts,format:exportFormat,quality,resolution,linked_tracks:linkedTracks}});output=result.output;progress=100;}
    catch(reason){error=String(reason)}finally{exporting=false}
  }
  async function cancel(){await invoke("cancel_job")}
  async function fullscreen(){ if(!stage)return; if(document.fullscreenElement)await document.exitFullscreen();else await stage.requestFullscreen(); }
  async function chooseAnalysis(){const path=await open({multiple:false,filters:[{name:"Audio or video",extensions:["wav","mp3","m4a","aac","flac","opus","mp4","mov","mkv","webm"]}]});if(typeof path==="string")analysisInput=path}
  function tcSeconds(value:string|null,fps:number){if(!value)return 0;const parts=value.replace(";",":").split(":").map(Number);return parts.length===4?parts[0]*3600+parts[1]*60+parts[2]+parts[3]/fps:0}
  async function addTrack(){
    const picked=await open({multiple:true,filters:[{name:"Linked media",extensions:["mp4","mov","mkv","webm","avi","wav","mp3","m4a","aac","flac","opus"]}]});
    const paths=Array.isArray(picked)?picked:typeof picked==="string"?[picked]:[]; const added:LinkedTrack[]=[];
    for(const path of paths){if(path===media.path||linkedTracks.some(t=>t.path===path))continue;const info=await invoke<MediaInfo>("probe_media",{path});const timecode=!!(media.start_timecode&&info.start_timecode);const offset=timecode?tcSeconds(info.start_timecode,media.fps||30)-tcSeconds(media.start_timecode,media.fps||30):0;added.push({path,offset,timecode})}
    linkedTracks=[...linkedTracks,...added];
  }
  const base=(path:string)=>path.split(/[\\/]/).pop()??path;
  function startEdgeDrag(event:PointerEvent,index:number,key:"start"|"end"){
    event.stopPropagation(); if(!waveEl)return; const rect=waveEl.getBoundingClientRect();
    const move=(e:PointerEvent)=>updateCut(index,key,viewStart+(e.clientX-rect.left)/rect.width*viewSpan);
    const up=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",up)};
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",up);
  }
  function clampWindow(start:number,end:number){const span=Math.max(1.5,Math.min(duration,end-start));const s=Math.max(0,Math.min(duration-span,start));return [s,Math.min(duration,s+span)] as const}
  function zoom(factor:number){const center=(viewStart+viewEnd)/2;[viewStart,viewEnd]=clampWindow(center-viewSpan*factor/2,center+viewSpan*factor/2)}
  function fit(){viewStart=0;viewEnd=duration}
  function timelineWheel(event:WheelEvent){event.preventDefault();const delta=Math.abs(event.deltaX)>Math.abs(event.deltaY)?event.deltaX:event.deltaY;if(viewSpan<duration-.001){[viewStart,viewEnd]=clampWindow(viewStart+delta*viewSpan/600,viewEnd+delta*viewSpan/600)}else seek(current+delta*duration/600)}
  function startNavDrag(event:PointerEvent,mode:"pan"|"left"|"right"){
    event.preventDefault();event.stopPropagation();if(!navEl)return;const rect=navEl.getBoundingClientRect(),x=event.clientX,start=viewStart,end=viewEnd;
    const move=(e:PointerEvent)=>{const dt=(e.clientX-x)/rect.width*duration;if(mode==="pan")[viewStart,viewEnd]=clampWindow(start+dt,end+dt);else if(mode==="left")viewStart=Math.max(0,Math.min(end-1.5,start+dt));else viewEnd=Math.min(duration,Math.max(start+1.5,end+dt))};
    const up=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",up)};window.addEventListener("pointermove",move);window.addEventListener("pointerup",up)
  }
  function navSeek(event:MouseEvent){if(!navEl)return;const rect=navEl.getBoundingClientRect();const at=(event.clientX-rect.left)/rect.width*duration;const half=viewSpan/2;[viewStart,viewEnd]=clampWindow(at-half,at+half)}
  function wavePath(values:number[]){if(values.length<2)return"";const max=Math.max(.01,...values);let p="";values.forEach((v,i)=>{const x=i/(values.length-1)*100,h=Math.sqrt(v/max)*47;p+=`${i?" L":"M"}${x.toFixed(2)},${(50-h).toFixed(2)}`});for(let i=values.length-1;i>=0;i--){const x=i/(values.length-1)*100,h=Math.sqrt(values[i]/max)*47;p+=` L${x.toFixed(2)},${(50+h).toFixed(2)}`}return p+" Z"}
  const visibleWave = $derived.by(()=>{if(!waveform.length||!duration)return"";const a=Math.floor(viewStart/duration*waveform.length),b=Math.max(a+2,Math.ceil(viewEnd/duration*waveform.length));return wavePath(waveform.slice(a,b))});
  const fullWave = $derived.by(()=>wavePath(waveform));

  $effect(()=>{
    const key=settingsKey;
    if(!hasAnalyzed||key===lastAnalyzedKey||analyzing||autoTuning)return;
    const timer=setTimeout(()=>analyze(),140);
    return()=>clearTimeout(timer);
  });
  function edgeKey(event:KeyboardEvent,index:number,key:"start"|"end"){
    if(event.key!=="ArrowLeft"&&event.key!=="ArrowRight")return;
    event.preventDefault(); const step=event.shiftKey?.01:.1; updateCut(index,key,cuts[index][key]+(event.key==="ArrowRight"?step:-step));
  }

  onMount(()=>{
    listen<Progress>("container-progress",e=>{if(exporting)progress=e.payload.percent}).then(fn=>unlisten=fn);
    const key=(e:KeyboardEvent)=>{const tag=(document.activeElement as HTMLElement)?.tagName;if(["INPUT","SELECT","TEXTAREA"].includes(tag))return;if(e.code==="Space"){e.preventDefault();togglePlay()}else if(e.key==="ArrowLeft")seek(current-5);else if(e.key==="ArrowRight")seek(current+5)};
    window.addEventListener("keydown",key);
    viewStart=0; viewEnd=duration<=90?duration:Math.min(duration,Math.max(60,Math.min(240,duration/5)));
    invoke<number[]>("compute_autocut_waveform",{path:media.path}).then(result=>waveform=result).catch(reason=>error=String(reason)).finally(()=>waveformLoading=false);
    return()=>{unlisten?.();window.removeEventListener("keydown",key)};
  });
</script>

<section class="ac-layout">
  <aside class="ac-side ac-left">
    <div class="ac-card">
      <header><div><h3>{t("detection")}</h3><p>{t("silenceParams")}</p></div><div class="detect-actions"><button onclick={autoTune} disabled={autoTuning||analyzing}>{autoTuning?"…":"AUTO"}</button><span class="ac-dot red"></span></div></header>
      <div class="ac-fields">
        <label><span>{t("threshold")} <b>{threshold.toFixed(2)}</b></span><input type="range" min="0.05" max="0.95" step="0.01" bind:value={threshold}></label>
        <label><span>{t("minSilence")} <b>{minSilence.toFixed(2)}s</b></span><input type="range" min="0.05" max="2" step="0.05" bind:value={minSilence}></label>
        <label><span>{t("minSpeech")} <b>{minSpeech.toFixed(2)}s</b></span><input type="range" min="0.05" max="2" step="0.05" bind:value={minSpeech}></label>
        <label><span>{t("padding")} <b>{padding.toFixed(2)}s</b></span><input type="range" min="0" max="1" step="0.02" bind:value={padding}></label>
        <p class="ac-help">{t("help")}</p>
        {#if autoSummary}<p class="auto-summary">AUTO · {autoSummary}</p>{/if}
        <button class="ac-secondary listen" onclick={chooseAnalysis}>{analysisInput?`${t("listen")}: ${base(analysisInput)}`:t("listen")}</button>
        {#if analysisInput}<button class="clear-source" onclick={()=>analysisInput=""}>{t("camera")}</button>{/if}
        <button class="ac-primary" onclick={analyze} disabled={analyzing||exporting}>{analyzing?t("analyzing"):t("detect")}</button>
      </div>
    </div>
    <div class="ac-card ac-export">
      <header><div><h3>{t("export")}</h3><p>{time(kept)} {t("kept")} · {time(removed)} {t("removed")}</p></div></header>
      <div class="ac-fields">
        <label><span>{t("format")}</span><select bind:value={exportFormat}><option value="mp4">MP4 Video</option><option value="fcpxml">Final Cut Pro XML</option></select></label>
        {#if exportFormat==="mp4"}
          <label><span>{t("quality")}</span><select bind:value={quality}><option value="high">{t("high")} · CRF 18</option><option value="medium">{t("medium")} · CRF 22</option><option value="small">{t("small")} · CRF 26</option></select></label>
          <label><span>{t("resolution")}</span><select bind:value={resolution}><option value="source">{t("source")}</option><option value="1080">1080p</option><option value="720">720p</option><option value="480">480p</option></select></label>
        {/if}
        <div class="linked-head"><span>{t("linked")}</span><button onclick={addTrack}>{t("add")}</button></div>
        {#each linkedTracks as track,index}
          <div class="linked-row"><span title={track.path}><b>{track.timecode?"tc":"≈"}</b> {base(track.path)}</span><input aria-label="Track offset" type="number" step="0.01" bind:value={track.offset}><button onclick={()=>linkedTracks=linkedTracks.filter((_,i)=>i!==index)}>×</button></div>
        {/each}
        <button class="ac-primary" onclick={exportCuts} disabled={exporting||analyzing||!cuts.length}>{exporting?`${t("exporting")} ${progress.toFixed(0)}%`:`${t("export")} ${exportFormat==="mp4"?"MP4":"FCPXML"}`}</button>
        {#if exporting}<button class="ac-secondary danger" onclick={cancel}>{t("cancelExport")}</button>{/if}
        {#if output}<button class="ac-secondary" onclick={()=>revealItemInDir(output)}>{t("showOutput")}</button>{/if}
      </div>
    </div>
  </aside>

  <div class="ac-main">
    <div class="ac-player ac-card" bind:this={stage}>
      <!-- svelte-ignore a11y_media_has_caption -->
      <video bind:this={video} src={mediaUrl} preload="metadata" ontimeupdate={onTime} onplay={()=>playing=true} onpause={()=>playing=false} onended={()=>playing=false}></video>
      <div class="ac-controls">
        <input class="player-seek" style={`--seek-pct:${duration ? Math.min(100,current/duration*100) : 0}%`} aria-label="Video position" type="range" min="0" max={duration} step="0.01" value={current} oninput={event=>seek(Number(event.currentTarget.value))}>
        <button onclick={()=>seek(current-15)} title="15 seconds back">−15</button>
        <button class="play" onclick={togglePlay} title="Play / Pause">{playing?"Ⅱ":"▶"}</button>
        <button onclick={()=>seek(current+15)} title="15 seconds forward">+15</button>
        <span class="ac-time mono">{time(current)} <i>/</i> {time(duration)}</span>
        <input class="volume" aria-label="Volume" type="range" min="0" max="1" step="0.05" bind:value={volume} oninput={()=>{if(video)video.volume=volume}}>
        <button onclick={fullscreen} title="Fullscreen">⛶</button>
      </div>
    </div>
    <div class="ac-timeline ac-card">
      <header class="tl-head">
        <div class="tl-title"><h3>{t("timeline")}</h3><p>{waveformLoading?t("waveform"):`${t("kept")} ${time(kept)} · ${t("removed")} ${time(removed)}`}</p></div>
        <div class="tl-buttons"><button onclick={()=>zoom(1.6)} disabled={zoomLevel<=1.001}>−</button><button onclick={()=>zoom(1/1.6)} disabled={viewSpan<=1.501}>+</button><button class="fit" onclick={fit}>{zoomLevel.toFixed(1)}×</button><i></i><button onclick={previousCut}>|◀</button><button class="tl-play" onclick={togglePlay}>{playing?"Ⅱ":"▶"}</button><button onclick={nextCut}>▶|</button></div>
        <div class="tl-status"><label class="ac-switch"><input type="checkbox" bind:checked={skipRemoved}><span></span> {skipRemoved?t("skipping"):t("playingAll")}</label><b>{time(current)} / {time(duration)}</b></div>
      </header>
      <div class="wave" bind:this={waveEl} onclick={scrubView} onwheel={timelineWheel} role="presentation">
        {#if visibleWave}<svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true"><path d={visibleWave}></path></svg>{/if}
        {#each segments as segment}
          <span class="segment" class:remove={segment.kind==="remove"} class:keep={segment.kind==="keep"&&segment.enabled} class:off={segment.kind==="keep"&&!segment.enabled} style:left={`${(segment.start-viewStart)/viewSpan*100}%`} style:width={`${(segment.end-segment.start)/viewSpan*100}%`}>
            {#if segment.kind==="keep"}
              <i role="slider" tabindex="0" aria-label={`Keep ${segment.index+1} start`} aria-valuemin="0" aria-valuemax={duration} aria-valuenow={segment.start} class="edge left" onkeydown={e=>edgeKey(e,segment.index,"start")} onpointerdown={e=>startEdgeDrag(e,segment.index,"start")}></i>
              <i role="slider" tabindex="0" aria-label={`Keep ${segment.index+1} end`} aria-valuemin="0" aria-valuemax={duration} aria-valuenow={segment.end} class="edge right" onkeydown={e=>edgeKey(e,segment.index,"end")} onpointerdown={e=>startEdgeDrag(e,segment.index,"end")}></i>
            {/if}
          </span>
        {/each}
        {#if current>=viewStart&&current<=viewEnd}<b class="playhead" style:left={`${(current-viewStart)/viewSpan*100}%`}></b>{/if}
      </div>
      <div class="ruler"><span>{time(viewStart)}</span><span>{time(viewStart+viewSpan/4)}</span><span>{time(viewStart+viewSpan/2)}</span><span>{time(viewStart+viewSpan*3/4)}</span><span>{time(viewEnd)}</span></div>
      <div class="navigator" bind:this={navEl} onclick={navSeek} role="presentation">
        {#if fullWave}<svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true"><path d={fullWave}></path></svg>{/if}
        {#each cuts as cut}<span class:off={!cut.enabled} style:left={pct(cut.start)} style:width={pct(cut.end-cut.start)}></span>{/each}
        <div class="nav-window" style:left={pct(viewStart)} style:width={pct(viewSpan)} onpointerdown={e=>startNavDrag(e,"pan")} role="presentation"><i role="separator" aria-label="Resize timeline view from left" class="nav-left" onpointerdown={e=>startNavDrag(e,"left")}></i><i role="separator" aria-label="Resize timeline view from right" class="nav-right" onpointerdown={e=>startNavDrag(e,"right")}></i></div>
        <b style:left={pct(current)}></b>
      </div>
    </div>
    {#if error}<div class="ac-error">{error}</div>{/if}
  </div>

  <aside class="ac-side ac-right ac-card">
    <header><div><h3>{t("cuts")}</h3><p>{t("editable")}</p></div><button class="ac-mini" onclick={addCut}>{t("add")}</button></header>
    <div class="cut-summary"><span><b>{cuts.length}</b> {t("regions")}</span><span><b>{time(kept)}</b> {t("output")}</span></div>
    <div class="cut-list">
      {#each cuts as cut,index}
        <article class:disabled={!cut.enabled}>
          <button class="cut-number" onclick={()=>jumpCut(cut)}>{String(index+1).padStart(2,"0")}</button>
          <div><label>IN<input type="number" min="0" max={duration} step="0.01" value={cut.start} onchange={e=>updateCut(index,"start",Number(e.currentTarget.value))}></label><label>OUT<input type="number" min="0" max={duration} step="0.01" value={cut.end} onchange={e=>updateCut(index,"end",Number(e.currentTarget.value))}></label><small>{time(cut.end-cut.start)}</small></div>
          <button class="cut-toggle" onclick={()=>{cuts=cuts.map((c,i)=>i===index?{...c,enabled:!c.enabled}:c)}}>{cut.enabled?t("keep"):t("off")}</button>
          <button class="cut-delete" onclick={()=>cuts=cuts.filter((_,i)=>i!==index)}>×</button>
        </article>
      {:else}
        <div class="cut-empty">{t("empty")}</div>
      {/each}
    </div>
  </aside>
</section>
