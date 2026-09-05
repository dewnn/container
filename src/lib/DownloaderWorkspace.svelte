<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { reportProblem } from "./toast";

  let { language }: { language: "tr" | "en" } = $props();
  interface DownloaderStatus { ready:boolean; version:string|null }
  interface DownloaderResult { output_dir:string; details:string }
  interface DownloadFormat { id:string; label:string; detail:string; kind:string; codec:string; rank:number; height:number|null }
  interface DownloadAnalysis { title:string; uploader:string|null; duration:number|null; thumbnail_path:string|null; formats:DownloadFormat[] }
  interface DownloadProgress { percent:number; downloaded:string; speed:string }
  let status:DownloaderStatus|null=$state(null);
  let url=$state("");
  let format=$state("best");
  let analysis:DownloadAnalysis|null=$state(null);
  let analyzing=$state(false);
  let busy=$state(false);
  let message=$state("");
  let outputDir=$state("");
  let downloadProgress:DownloadProgress|null=$state(null);
  let formatKind=$state<"video"|"audio">("video");
  let preferredCodec=$state("auto");
  let preferredHeight=$state<number|null>(null);
  let unlisten:UnlistenFn|undefined;

  async function refresh(){status=await invoke<DownloaderStatus>("downloader_status").catch(()=>({ready:false,version:null}))}
  async function chooseBinary(){
    const path=await open({multiple:false,directory:false,filters:[{name:"yt-dlp",extensions:["exe"]}]});
    if(typeof path!=="string")return;
    busy=true;message="";
    try{status=await invoke<DownloaderStatus>("set_downloader_binary",{path});message=language==="tr"?"yt-dlp hazır.":"yt-dlp is ready."}catch(error){message="";reportProblem(error)}finally{busy=false}
  }
  async function download(){
    if(busy)return;busy=true;message="";downloadProgress={percent:0,downloaded:language==="tr"?"Kaynağa bağlanılıyor…":"Connecting to source…",speed:""};
    try{const result=await invoke<DownloaderResult>("download_media",{url,format});outputDir=result.output_dir;message=language==="tr"?"İndirme tamamlandı.":"Download finished."}catch(error){message="";reportProblem(error)}finally{busy=false;downloadProgress=null}
  }
  async function cancelDownload(){if(!busy)return;await invoke("cancel_job").catch(()=>{});message=language==="tr"?"İndirme iptal ediliyor…":"Cancelling download…"}
  async function analyze(){
    if(!url.trim()||busy||analyzing)return;
    analyzing=true;message="";analysis=null;
    try{
      analysis=await invoke<DownloadAnalysis>("analyze_download_url",{url});
      formatKind="video";preferredCodec="auto";
      preferredHeight=videoHeights()[0]??null;
      chooseVideo();
    }catch(error){message="";reportProblem(error)}finally{analyzing=false}
  }
  function duration(value:number|null){if(!value)return "—";const seconds=Math.round(value),minutes=Math.floor(seconds/60);return `${minutes}:${String(seconds%60).padStart(2,"0")}`}
  function codecName(value:string){const normalized=value.toLowerCase();if(normalized.includes("av01"))return "AV1";if(normalized.includes("vp09")||normalized.includes("vp9"))return "VP9";if(normalized.includes("hev")||normalized.includes("hvc"))return "H.265";if(normalized.includes("avc"))return "H.264";return value.toUpperCase()}
  function videoHeights(){const data=analysis as DownloadAnalysis|null;return data?[...new Set(data.formats.filter((item:DownloadFormat)=>item.kind==="video"&&item.height).map((item:DownloadFormat)=>item.height as number))].sort((a,b)=>b-a).slice(0,6):[]}
  function videoCodecs(){const data=analysis as DownloadAnalysis|null;return data?[...new Set(data.formats.filter((item:DownloadFormat)=>item.kind==="video").map((item:DownloadFormat)=>codecName(item.codec)))].slice(0,4):[]}
  function audioCodecs(){const data=analysis as DownloadAnalysis|null;return data?[...new Set(data.formats.filter((item:DownloadFormat)=>item.kind==="audio").map((item:DownloadFormat)=>codecName(item.codec)))].slice(0,4):[]}
  function chooseVideo(){const data=analysis as DownloadAnalysis|null;if(!data)return;const matches=data.formats.filter((item:DownloadFormat)=>item.kind==="video"&&(!preferredHeight||item.height===preferredHeight)&&(preferredCodec==="auto"||codecName(item.codec)===preferredCodec));const fallback=data.formats.find((item:DownloadFormat)=>item.kind==="video");format=(matches[0]??fallback)?.id??"best"}
  function chooseAudio(codec:string){const data=analysis as DownloadAnalysis|null;preferredCodec=codec;const id=data?.formats.find((item:DownloadFormat)=>item.kind==="audio"&&codecName(item.codec)===codec)?.id;format=id?`audio:${id}`:"audio"}
  $effect(()=>{refresh();listen<DownloadProgress>("downloader-progress",event=>downloadProgress=event.payload).then(value=>unlisten=value);return()=>unlisten?.()});
</script>

<section class="downloader-workspace">
  <aside class="downloader-side panel">
    <header><span class="status-dot" class:missing={!status?.ready}></span><div><h3>DWLNDR</h3><p>{language==="tr"?"güvenli video indirici":"safe video downloader"}</p></div></header>
    <div class="downloader-engine">
      <b>{language==="tr"?"İNDİRME MOTORU":"DOWNLOAD ENGINE"}</b>
      <strong class:ready={!!status?.ready}>{status?.ready ? `yt-dlp ${status.version}` : (language==="tr"?"yt-dlp gerekli":"yt-dlp required")}</strong>
      <p>{language==="tr"?"Doğrulanmış yt-dlp CONTAINER ile birlikte gelir. İstersen resmî bir sürümle değiştirebilirsin; çerezlere erişilmez.":"A verified yt-dlp build is included with CONTAINER. You can replace it with an official build; cookies are never accessed."}</p>
      <button class="ghost" onclick={chooseBinary} disabled={busy}>{status?.ready ? (language==="tr"?"YT-DLP’Yİ DEĞİŞTİR":"CHANGE YT-DLP") : (language==="tr"?"YT-DLP.EXE SEÇ":"CHOOSE YT-DLP.EXE")}</button>
      <a href="https://github.com/yt-dlp/yt-dlp/releases/latest" target="_blank" rel="noreferrer">{language==="tr"?"Resmî indirme sayfası ↗":"Official download page ↗"}</a>
    </div>
  </aside>
  <main class="downloader-main panel">
    <header><div><h2>{language==="tr"?"VİDEO BAĞLANTISI":"VIDEO LINK"}</h2><p>{language==="tr"?"YouTube ve yt-dlp’nin desteklediği HTTPS kaynakları":"YouTube and HTTPS sources supported by yt-dlp"}</p></div><span>HTTPS ONLY</span></header>
    <label><span>{language==="tr"?"Bağlantıyı yapıştır":"Paste a link"}</span><div class="downloader-url"><input value={url} oninput={(event)=>{url=event.currentTarget.value;analysis=null}} onkeydown={(event)=>{if(event.key==="Enter")analyze()}} placeholder="https://…" disabled={!status?.ready||busy||analyzing}><button onclick={analyze} disabled={!status?.ready||!url.trim()||busy||analyzing}>{analyzing?(language==="tr"?"ANALİZ…":"ANALYZING…"):(language==="tr"?"BAĞLANTIYI ANALİZ ET":"ANALYZE LINK")}</button></div></label>
    {#if analysis}
      <section class="download-analysis">
        {#if analysis.thumbnail_path}<img src={convertFileSrc(analysis.thumbnail_path)} alt="">{/if}
        <div><b>{analysis.title}</b><p>{analysis.uploader ?? (language==="tr"?"Kaynak bilgisi yok":"No source details")} <i>·</i> {duration(analysis.duration)} <i>·</i> {analysis.formats.length} {language==="tr"?"format":"formats"}</p><span>{language==="tr"?"Bağlantı doğrulandı; indirme seçeneklerini seçebilirsin.":"Link verified; choose a download option."}</span></div>
      </section>
    {/if}
    {#if analysis}<section class="download-options"><div class="format-categories"><button class:active={formatKind==="video"} onclick={()=>{formatKind="video";preferredCodec="auto";format="best"}}>{language==="tr"?"VİDEO + SES":"VIDEO + AUDIO"}</button><button class:active={formatKind==="audio"} onclick={()=>{formatKind="audio";preferredCodec="auto";format="audio"}}>{language==="tr"?"SADECE SES":"AUDIO ONLY"}</button></div>{#if formatKind==="video"}<h4>{language==="tr"?"VİDEO KALİTESİ":"VIDEO QUALITY"}</h4><div class="option-pills">{#each videoHeights() as height}<button class:active={preferredHeight===height} onclick={()=>{preferredHeight=height;chooseVideo()}}>{height}p</button>{/each}</div><h4>{language==="tr"?"TERCİH EDİLEN CODEC":"PREFERRED CODEC"}</h4><div class="option-pills"><button class:active={preferredCodec==="auto"} onclick={()=>{preferredCodec="auto";chooseVideo()}}>AUTO</button>{#each videoCodecs() as codec}<button class:active={preferredCodec===codec} onclick={()=>{preferredCodec=codec;chooseVideo()}}>{codec}</button>{/each}</div>{:else}<h4>{language==="tr"?"SES FORMATI":"AUDIO FORMAT"}</h4><div class="option-pills"><button class:active={format==="audio"} onclick={()=>format="audio"}>M4A</button>{#each audioCodecs() as codec}<button class:active={preferredCodec===codec} onclick={()=>chooseAudio(codec)}>{codec}</button>{/each}</div>{/if}</section>{/if}
    <button class="downloader-run" onclick={download} disabled={!status?.ready||!analysis||busy}>{busy?(language==="tr"?"İNDİRİLİYOR…":"DOWNLOADING…"):(language==="tr"?"▶ İNDİRMEYİ BAŞLAT":"▶ START DOWNLOAD")}</button>
    {#if busy}<div class="downloader-live" class:pending={!downloadProgress||downloadProgress.percent===0}><i style:width={`${downloadProgress?.percent??0}%`}></i><span>{`${(downloadProgress?.percent??0).toFixed(1)}%`}</span><small class="download-transfer"><em>{downloadProgress?.downloaded??(language==="tr"?"Kaynağa bağlanılıyor…":"Connecting to source…")}</em>{#if downloadProgress?.speed}<b>·</b><strong>{downloadProgress.speed}</strong>{/if}</small><button onclick={cancelDownload}>{language==="tr"?"İPTAL":"CANCEL"}</button></div>{/if}
    {#if message}<div class:failure={message.toLowerCase().includes("failed")||message.toLowerCase().includes("valid")||message.toLowerCase().includes("gerekli")} class="downloader-message">{message}</div>{/if}
    {#if outputDir}<button class="downloader-output" onclick={()=>revealItemInDir(outputDir)}>{language==="tr"?"İNDİRME KLASÖRÜNÜ AÇ":"OPEN DOWNLOAD FOLDER"}</button>{/if}
  </main>
</section>
