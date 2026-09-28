<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { armCompletionSound, playCompletionSound } from "./completionSound";
  import { reportProblem } from "./toast";
  import { untrack } from "svelte";

  let { language, hasMedia, onback, onbusychange, onopenmedia }: { language: "tr" | "en"; hasMedia:boolean; onback:()=>void; onbusychange?: (value:boolean)=>void; onopenmedia: (path:string)=>Promise<void> } = $props();
  interface DownloaderStatus { ready:boolean; version:string|null }
  interface DownloaderResult { output_dir:string; output_file:string|null; details:string }
  interface DownloadFormat { id:string; label:string; detail:string; kind:string; codec:string; rank:number; height:number|null }
  interface DownloadAnalysis { title:string; uploader:string|null; duration:number|null; thumbnail_path:string|null; formats:DownloadFormat[] }
  interface DownloadProgress { percent:number; downloaded:string; speed:string }
  interface DownloadHistoryEntry { path:string; name:string }
  let status:DownloaderStatus|null=$state(null);
  let url=$state("");
  let format=$state("best");
  let analysis:DownloadAnalysis|null=$state(null);
  let analyzing=$state(false);
  let busy=$state(false);
  let message=$state("");
  let outputFile=$state("");
  let history=$state<DownloadHistoryEntry[]>([]);
  let historyBusy=$state(false);
  let historyRequest=0;
  let deletingPath=$state("");
  let pendingDelete=$state<DownloadHistoryEntry|null>(null);
  let downloadProgress:DownloadProgress|null=$state(null);
  let formatKind=$state<"video"|"audio">("video");
  let preferredCodec=$state("auto");
  let preferredHeight=$state<number|null>(null);
  let unlisten:UnlistenFn|undefined;
  let temporaryThumbnail:string|null=null;

  $effect(()=>onbusychange?.(busy||analyzing||!!deletingPath));

  function releaseThumbnail(path:string|null|undefined){
    if(!path||path!==temporaryThumbnail)return;
    temporaryThumbnail=null;
    void invoke("remove_download_thumbnail",{path}).catch(()=>{});
  }

  async function refresh(){status=await invoke<DownloaderStatus>("downloader_status").catch(()=>({ready:false,version:null}))}
  async function refreshHistory(){
    const request=++historyRequest;
    historyBusy=true;
    try{
      const entries=await invoke<DownloadHistoryEntry[]>("list_download_history");
      if(request!==historyRequest)return;
      history=entries;
      if(outputFile&&!history.some(entry=>entry.path===outputFile))outputFile="";
    }catch(error){if(request===historyRequest)reportProblem(error)}finally{if(request===historyRequest)historyBusy=false}
  }
  function mountDeleteDialog(node:HTMLDialogElement){
    node.showModal();
    node.querySelector<HTMLButtonElement>(".panel-reset-cancel")?.focus();
    return {destroy(){if(node.open)node.close()}};
  }
  async function deleteHistoryEntry(){
    const entry=pendingDelete;
    if(!entry||busy||analyzing||deletingPath)return;
    pendingDelete=null;
    deletingPath=entry.path;
    try{
      await invoke("delete_download_history_entry",{path:entry.path});
      if(outputFile===entry.path)outputFile="";
      await refreshHistory();
    }catch(error){reportProblem(error)}finally{deletingPath=""}
  }
  async function chooseBinary(){
    const path=await open({multiple:false,directory:false,filters:[{name:"yt-dlp",extensions:["exe"]}]});
    if(typeof path!=="string")return;
    busy=true;message="";
    try{status=await invoke<DownloaderStatus>("set_downloader_binary",{path});message=language==="tr"?"yt-dlp hazır.":"yt-dlp is ready."}catch(error){message="";reportProblem(error)}finally{busy=false}
  }
  async function download(){
    if(busy)return;armCompletionSound();busy=true;message="";outputFile="";downloadProgress={percent:0,downloaded:language==="tr"?"Kaynağa bağlanılıyor…":"Connecting to source…",speed:""};
    try{const result=await invoke<DownloaderResult>("download_media",{url,format});outputFile=result.output_file??"";message=language==="tr"?"İndirme tamamlandı.":"Download finished.";await refreshHistory();await playCompletionSound()}catch(error){message="";reportProblem(error)}finally{busy=false;downloadProgress=null}
  }
  async function cancelDownload(){if(!busy)return;await invoke("cancel_job").catch(()=>{});message=language==="tr"?"İndirme iptal ediliyor…":"Cancelling download…"}
  async function analyze(){
    if(!url.trim()||busy||analyzing)return;
    releaseThumbnail(analysis?.thumbnail_path);analyzing=true;message="";outputFile="";analysis=null;
    try{
      analysis=await invoke<DownloadAnalysis>("analyze_download_url",{url});
      temporaryThumbnail=analysis.thumbnail_path;
      formatKind="video";preferredCodec="auto";
      preferredHeight=videoHeights()[0]??null;
      chooseVideo();
    }catch(error){message="";reportProblem(error)}finally{analyzing=false}
  }
  function isYouTube(){try{const host=new URL(url).hostname.toLowerCase();return host==="youtu.be"||host==="youtube.com"||host.endsWith(".youtube.com")}catch{return false}}
  function duration(value:number|null){if(!value)return "—";const seconds=Math.round(value),minutes=Math.floor(seconds/60);return `${minutes}:${String(seconds%60).padStart(2,"0")}`}
  function codecName(value:string){const normalized=value.toLowerCase();if(normalized.includes("av01"))return "AV1";if(normalized.includes("vp09")||normalized.includes("vp9"))return "VP9";if(normalized.includes("hev")||normalized.includes("hvc"))return "H.265";if(normalized.includes("avc"))return "H.264";return value.toUpperCase()}
  function videoHeights(){const data=analysis as DownloadAnalysis|null;return data?[...new Set(data.formats.filter((item:DownloadFormat)=>item.kind==="video"&&item.height).map((item:DownloadFormat)=>item.height as number))].sort((a,b)=>b-a).slice(0,6):[]}
  function videoCodecs(){const data=analysis as DownloadAnalysis|null;return data?[...new Set(data.formats.filter((item:DownloadFormat)=>item.kind==="video").map((item:DownloadFormat)=>codecName(item.codec)))].slice(0,4):[]}
  function chooseVideo(){const data=analysis as DownloadAnalysis|null;if(!data)return;if(!videoHeights().length){format="best";return}const matches=data.formats.filter((item:DownloadFormat)=>item.kind==="video"&&(!preferredHeight||item.height===preferredHeight)&&(preferredCodec==="auto"||codecName(item.codec)===preferredCodec));const fallback=data.formats.find((item:DownloadFormat)=>item.kind==="video");format=(matches[0]??fallback)?.id??"best"}
  $effect(()=>{let disposed=false;untrack(()=>{void refresh();void refreshHistory()});const onFocus=()=>{if(!disposed)void refreshHistory()};window.addEventListener("focus",onFocus);listen<DownloadProgress>("downloader-progress",event=>downloadProgress=event.payload).then(value=>{if(disposed)value();else unlisten=value});return()=>{disposed=true;historyRequest++;window.removeEventListener("focus",onFocus);unlisten?.();releaseThumbnail(temporaryThumbnail)}});
</script>

<section class="downloader-workspace compact-downloader">
  <details class="engine-details" open={status!==null&&!status.ready}>
    <summary><span class="status-dot" class:missing={!status?.ready}></span>{language==="tr"?"Motor bilgisi":"Engine information"}</summary>
    <div class="downloader-engine">
      <b>{language==="tr"?"İNDİRME MOTORU":"DOWNLOAD ENGINE"}</b>
      <strong class:ready={!!status?.ready}>{status?.ready ? `yt-dlp ${status.version}` : (language==="tr"?"yt-dlp gerekli":"yt-dlp required")}</strong>
      <p>{language==="tr"?"CONTAINER, doğrulanmış bir yt-dlp sürümüyle gelir. İstersen kendi resmî sürümünü seçebilirsin. Tarayıcı çerezlerine erişilmez.":"A verified yt-dlp build is included with CONTAINER. You can replace it with an official build; cookies are never accessed."}</p>
      <button class="ghost" onclick={chooseBinary} disabled={busy}>{status?.ready ? (language==="tr"?"YT-DLP’Yİ DEĞİŞTİR":"CHANGE YT-DLP") : (language==="tr"?"YT-DLP.EXE SEÇ":"CHOOSE YT-DLP.EXE")}</button>
      <a href="https://github.com/yt-dlp/yt-dlp/releases/latest" target="_blank" rel="noreferrer">{language==="tr"?"Resmî indirme sayfası ↗":"Official download page ↗"}</a>
    </div>
  </details>
  <main class="downloader-main">
    <header class="download-intro">
      <span class="brand-logo-stack download-mark" aria-hidden="true"><img class="brand-logo brand-logo-dark" src="/mark-dark.svg" alt=""><img class="brand-logo brand-logo-light" src="/mark-light.svg" alt=""></span>
      <h2>DWLNDR</h2>
      <p>{language==="tr"?"Bağlantıyı yapıştır, ne indireceğini seç.":"Paste a link. Make it yours."}</p>
    </header>
    <label><span>{language==="tr"?"Bağlantıyı yapıştır":"Paste a link"}</span><div class="downloader-url"><input value={url} oninput={(event)=>{releaseThumbnail(analysis?.thumbnail_path);url=event.currentTarget.value;analysis=null;outputFile="";message=""}} onkeydown={(event)=>{if(event.key==="Enter")analyze()}} placeholder="https://…" disabled={!status?.ready||busy||analyzing}><button onclick={analyze} disabled={!status?.ready||!url.trim()||busy||analyzing}>{analyzing?(language==="tr"?"ANALİZ…":"ANALYZING…"):(language==="tr"?"BAĞLANTIYI ANALİZ ET":"ANALYZE LINK")}</button></div></label>
    {#if analysis}
      <section class="download-analysis">
        {#if analysis.thumbnail_path}<img src={convertFileSrc(analysis.thumbnail_path)} alt="" onload={()=>releaseThumbnail(analysis?.thumbnail_path)} onerror={()=>releaseThumbnail(analysis?.thumbnail_path)}>{/if}
        <div><b>{analysis.title}</b><p>{analysis.uploader ?? (language==="tr"?"Kaynak bilgisi yok":"No source details")} <i>·</i> {duration(analysis.duration)} <i>·</i> {analysis.formats.length} {language==="tr"?"format":"formats"}</p><span>{language==="tr"?"Bağlantı doğrulandı; indirme seçeneklerini seçebilirsin.":"Link verified; choose a download option."}</span></div>
      </section>
    {/if}
    {#if analysis}<section class="download-options"><div class="format-categories"><button class:active={formatKind==="video"} onclick={()=>{formatKind="video";preferredCodec="auto";chooseVideo()}}>{language==="tr"?"VİDEO + SES":"VIDEO + AUDIO"}</button><button class:active={formatKind==="audio"} onclick={()=>{formatKind="audio";preferredCodec="auto";format="audio-format:m4a"}}>{language==="tr"?"SADECE SES":"AUDIO ONLY"}</button></div>{#if formatKind==="video"}<h4>{language==="tr"?"VİDEO KALİTESİ":"VIDEO QUALITY"}</h4><div class="option-pills">{#each videoHeights() as height}<button class:active={preferredHeight===height} onclick={()=>{preferredHeight=height;chooseVideo()}}>{height}p</button>{:else}<span>{language==="tr"?"En iyi mevcut kalite otomatik seçilecek.":"Best available quality will be selected automatically."}</span>{/each}</div>{#if isYouTube()}<h4>{language==="tr"?"TERCİH EDİLEN CODEC":"PREFERRED CODEC"}</h4><div class="option-pills"><button class:active={preferredCodec==="auto"} onclick={()=>{preferredCodec="auto";chooseVideo()}}>AUTO</button>{#each videoCodecs() as codec}<button class:active={preferredCodec===codec} onclick={()=>{preferredCodec=codec;chooseVideo()}}>{codec}</button>{/each}</div>{/if}{:else}<h4>{language==="tr"?"SES ÇIKTI FORMATI":"AUDIO OUTPUT FORMAT"}</h4><div class="option-pills"><button class:active={format==="audio-format:m4a"} onclick={()=>format="audio-format:m4a"}>M4A</button><button class:active={format==="audio-format:opus"} onclick={()=>format="audio-format:opus"}>OPUS</button></div>{/if}</section>{/if}
    {#if analysis}<button class="downloader-run" onclick={download} disabled={!status?.ready||busy}>{busy?(language==="tr"?"İNDİRİLİYOR…":"DOWNLOADING…"):(language==="tr"?"↓ İNDİR":"↓ DOWNLOAD")}</button>{/if}
    {#if busy}<div class="downloader-live" class:pending={!downloadProgress||downloadProgress.percent===0}><i style:width={`${downloadProgress?.percent??0}%`}></i><span>{`${(downloadProgress?.percent??0).toFixed(1)}%`}</span><small class="download-transfer"><em>{downloadProgress?.downloaded??(language==="tr"?"Kaynağa bağlanılıyor…":"Connecting to source…")}</em>{#if downloadProgress?.speed}<b>·</b><strong>{downloadProgress.speed}</strong>{/if}</small><button onclick={cancelDownload}>{language==="tr"?"İPTAL":"CANCEL"}</button></div>{/if}
    {#if message}<div class:failure={message.toLowerCase().includes("failed")||message.toLowerCase().includes("valid")||message.toLowerCase().includes("gerekli")} class="downloader-message">{message}</div>{/if}
    {#if outputFile}<div class="download-complete-actions"><button class="download-open-editor" onclick={()=>onopenmedia(outputFile)}>{formatKind==="video"?(language==="tr"?"ZAMAN ÇİZELGESİNDE AÇ →":"OPEN IN TIMELINE →"):(language==="tr"?"DÜZENLEMEK İÇİN AÇ →":"OPEN IN EDITOR →")}</button><button class="downloader-output" onclick={()=>revealItemInDir(outputFile).catch(reportProblem)}>{language==="tr"?"DOSYAYI GÖSTER":"SHOW FILE"}</button></div>{/if}
    <section class="download-history" aria-label={language==="tr"?"İndirilenler":"Downloads"}>
      <header><h3>{language==="tr"?"İNDİRİLENLER":"DOWNLOADS"}</h3><button onclick={refreshHistory} disabled={historyBusy||busy} aria-label={language==="tr"?"İndirilenleri yenile":"Refresh downloads"}>↻</button></header>
      {#if history.length}
        <ul>{#each history as entry (entry.path)}<li><span title={entry.path}>{entry.name}</span><button class="history-show" onclick={()=>revealItemInDir(entry.path).catch(reportProblem)} disabled={!!deletingPath}>{language==="tr"?"GÖSTER":"SHOW"}</button><button class="history-delete" onclick={()=>pendingDelete=entry} disabled={busy||analyzing||!!deletingPath} aria-label={language==="tr"?`${entry.name} dosyasını sil`:`Delete ${entry.name}`} title={language==="tr"?"Geri Dönüşüm Kutusu’na taşı":"Move to Recycle Bin"}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3m3 0-1 13H7L6 7m4 4v6m4-6v6"/></svg></button></li>{/each}</ul>
      {:else}<p>{language==="tr"?"Henüz indirilen dosya yok.":"No downloads yet."}</p>{/if}
    </section>
    <div class="download-back-actions">
      <button class="downloader-back" class:editor={hasMedia} onclick={onback} disabled={busy||analyzing||!!deletingPath} title={busy||analyzing||!!deletingPath?(language==="tr"?"İşlem tamamlanana veya iptal edilene kadar bekle":"Wait until the operation finishes or is cancelled"):(hasMedia?(language==="tr"?"Açık çalışmaya dön":"Return to current work"):(language==="tr"?"Ana menüye dön":"Back to main menu"))}>
        <span class="download-back-arrow" aria-hidden="true">←</span>
        <span class="download-back-label">{hasMedia?(language==="tr"?"ÇALIŞMAYA DÖN":"BACK TO EDITOR"):(language==="tr"?"GERİ":"BACK")}</span>
      </button>
    </div>
  </main>
  <footer class="download-note">{language==="tr"?"Doğrudan cihazına kaydedilir · Hesap ve tarayıcı çerezi kullanılmaz":"Saved to your device · No account or browser cookies"}</footer>
</section>

{#if pendingDelete}
  <dialog class="panel-reset-dialog download-delete-dialog" use:mountDeleteDialog oncancel={()=>pendingDelete=null} aria-labelledby="download-delete-title" aria-describedby="download-delete-description">
    <header><span class="panel-reset-dialog-icon download-delete-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M9 7V4h6v3m3 0-1 13H7L6 7m4 4v6m4-6v6"/></svg></span><h2 id="download-delete-title">{language==="tr"?"İndirmeyi sil":"Delete download"}</h2></header>
    <p id="download-delete-description"><strong>{pendingDelete.name}</strong><br>{language==="tr"?"Bu dosya Geri Dönüşüm Kutusu’na taşınacak.":"This file will be moved to the Recycle Bin."}</p>
    <footer><button class="panel-reset-cancel" onclick={()=>pendingDelete=null}>{language==="tr"?"İPTAL":"CANCEL"}</button><button class="panel-reset-confirm download-delete-confirm" onclick={deleteHistoryEntry}>{language==="tr"?"SİL":"DELETE"}</button></footer>
  </dialog>
{/if}

<style>
  .compact-downloader{height:auto;min-height:calc(100dvh - 64px);display:flex;flex-direction:column;align-items:center;gap:0;padding:24px 24px 20px;overflow:auto}
  .engine-details{width:min(100%,680px);flex:none;color:var(--muted);font:10px var(--mono)}
  .engine-details summary{display:flex;align-items:center;justify-content:center;gap:9px;cursor:pointer;padding:8px;list-style:none}
  .engine-details summary::-webkit-details-marker{display:none}
  .engine-details summary::after{content:"+";font-size:14px}
  .engine-details[open] summary::after{content:"−"}
  .engine-details .downloader-engine{display:flex;flex-wrap:wrap;gap:12px;padding:16px;margin-top:8px;border:1px solid var(--border);border-radius:10px;background:var(--panel)}
  .engine-details .downloader-engine p{flex-basis:100%;max-width:none}
  .engine-details .downloader-engine a{white-space:normal}
  .compact-downloader .downloader-main{flex:none;width:min(100%,680px);min-height:0;margin:auto 0;padding:32px 0;background:transparent;border:0;box-shadow:none}
  .compact-downloader .download-intro{display:flex;flex-direction:column;justify-content:center;gap:12px;padding:0 0 28px;border:0;text-align:center}
  .compact-downloader .download-intro h2{font:700 26px var(--mono);letter-spacing:.12em}
  .compact-downloader .download-intro p{font-size:13px;margin:0}
  .compact-downloader .download-intro .download-mark{width:72px;height:72px;padding:0;border:0;border-radius:17px;margin-bottom:6px;background:transparent}
  .download-mark{flex:none}
  .download-mark :global(img){width:100%;height:100%;object-fit:contain}
  .compact-downloader .downloader-main>label{margin:0}
  .compact-downloader .downloader-main input{font-size:12px;padding:15px}
  .compact-downloader .download-analysis{margin:18px 0 0;background:var(--panel);border-color:var(--border);padding:12px;gap:12px}
  .download-analysis>div{min-width:0;overflow-wrap:anywhere}
  .compact-downloader .download-analysis img{width:96px;height:60px;flex:none}
  .compact-downloader .download-options{margin:14px 0 0;padding:14px}
  .compact-downloader .option-pills{font-size:12px;color:var(--muted);line-height:1.5}
  .compact-downloader .downloader-run{margin:14px 0 0;background:var(--text);color:var(--panel);padding:14px}
  .compact-downloader .downloader-live{margin:12px 0 0}
  .compact-downloader .downloader-message{margin:12px 0 0}
  .download-complete-actions{display:flex;align-items:center;justify-content:center;flex-wrap:wrap;gap:10px;margin:12px 0 0}
  .download-open-editor{min-height:34px;padding:8px 14px;border:1px solid var(--border);border-radius:6px;background:var(--text);color:var(--panel);font:700 10px var(--mono);cursor:pointer}
  .download-open-editor:hover{filter:brightness(.88)}
  .compact-downloader .downloader-output{margin:0;align-self:center;padding:8px}
  .download-note{flex:none;text-align:center;color:var(--muted-2);font:10px var(--mono);line-height:1.7;padding-top:16px}
  .download-history{margin:28px 0 0;border:1px solid var(--border);border-radius:9px;background:var(--panel);overflow:hidden}
  .download-history header{display:flex;align-items:center;justify-content:space-between;padding:11px 14px;border-bottom:1px solid var(--border)}
  .download-history h3{margin:0;color:var(--text);font:700 10px var(--mono);letter-spacing:.08em}
  .download-history header button{border:0;background:transparent;color:var(--muted);font-size:18px;cursor:pointer}
  .download-history ul{list-style:none;margin:0;padding:0;max-height:210px;overflow:auto}
  .download-history li{display:flex;align-items:center;gap:8px;min-width:0;padding:9px 12px;border-bottom:1px solid var(--border)}
  .download-history li:last-child{border-bottom:0}
  .download-history li span{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--text);font:11px var(--sans)}
  .download-history li button{flex:none;border:1px solid var(--border);border-radius:5px;background:var(--surface);color:var(--muted);cursor:pointer}
  .download-history li button:hover:not(:disabled){color:var(--text);border-color:var(--muted)}
  .download-history li .history-show{padding:6px 8px;font:9px var(--mono)}
  .download-history li .history-delete{display:grid;place-items:center;width:28px;height:28px;padding:4px}
  .download-history li .history-delete:hover:not(:disabled){color:var(--red);border-color:var(--red)}
  .download-history li svg{width:16px;height:16px}
  .download-history>p{padding:13px 14px;color:var(--muted);font:11px var(--sans)}
  .download-back-actions{display:flex;justify-content:center;margin:32px 0 0}
  .download-back-actions .downloader-back{display:inline-flex;align-items:center;justify-content:center;gap:8px;min-width:100px;min-height:36px;padding:0 16px;font-size:10px;font-weight:600}
  .download-back-actions .downloader-back.editor{min-width:166px}
  .download-back-arrow{font:14px/1 var(--sans)}
  .download-back-label{white-space:nowrap}
  .download-back-actions .downloader-back:focus-visible{outline:2px solid var(--blue);outline-offset:2px}
  .download-delete-dialog p strong{display:inline-block;max-width:100%;color:var(--text);overflow-wrap:anywhere}
  .download-delete-dialog .download-delete-icon{color:var(--red)}
  .download-delete-dialog .download-delete-icon svg{width:17px;height:17px}
  .download-delete-dialog footer .download-delete-confirm{border-color:var(--red);background:var(--red);color:#190607}
  @media(max-width:600px){.compact-downloader{padding:16px}.compact-downloader .downloader-url{flex-direction:column}.compact-downloader .downloader-url button{padding:12px}}
</style>
