<script lang="ts">
  import RecentFilesList from "./RecentFilesList.svelte";
  import type { RecentFiles } from "./recentFiles.svelte";
  interface ComponentStatus { ready:boolean }
  let { language, t, recent, onopenrecent, autoEncoderTuning, recovery, restoring, dragActive, ffmpegStatus, downloaderStatus, dependencyChecking, cleanupMessage, ondiscard, onrestore, onselect, oncheck, ondownloader }:{
    language:"tr"|"en"; t:(key:string)=>string; recent:RecentFiles; onopenrecent:(path:string)=>void; autoEncoderTuning:boolean; recovery:{mediaPath:string;savedAt:number}|null; restoring:boolean;
    dragActive:boolean; ffmpegStatus:ComponentStatus|null; downloaderStatus:ComponentStatus|null; dependencyChecking:boolean; cleanupMessage:string;
    ondiscard:()=>void; onrestore:()=>void; onselect:()=>void; oncheck:()=>void; ondownloader:()=>void;
  }=$props();
  const basename=(path:string)=>path.split(/[\\/]/).pop()??path;
</script>

<section class="landing">
  {#if autoEncoderTuning}
    <div class="first-run-tuning" role="status" aria-live="polite">
      <i aria-hidden="true"></i>
      <div><b>{language==="tr"?"PERFORMANS AYARLANIYOR":"TUNING PERFORMANCE"}</b><span>{language==="tr"?"Kısa ve sessizdir; yalnızca ekran kartı veya FFmpeg değişince tekrarlanır.":"A short silent test; it repeats only when the GPU or FFmpeg changes."}</span></div>
    </div>
  {/if}
  {#if recovery}
    <section class="recovery-card panel">
      <div><span>↻</span><div><h3>{language==="tr"?"ÖNCEKİ ÇALIŞMA BULUNDU":"PREVIOUS WORK FOUND"}</h3><p><b>{basename(recovery.mediaPath)}</b> · {new Date(recovery.savedAt).toLocaleString(language==="tr"?"tr-TR":"en-US")}</p></div></div>
      <aside><button class="ghost" onclick={ondiscard}>{language==="tr"?"VAZGEÇ":"DISCARD"}</button><button class="restore-session" onclick={onrestore} disabled={restoring}>{restoring?"…":(language==="tr"?"ÇALIŞMAYI GERİ YÜKLE":"RESTORE WORK")}</button></aside>
    </section>
  {/if}
  <button class="dropzone" class:active={dragActive} onclick={onselect} disabled={ffmpegStatus !== null && !ffmpegStatus.ready}>
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
      <aside><button class="dependency-check" onclick={oncheck} disabled={dependencyChecking}>{dependencyChecking ? "…" : (language === "tr" ? "TEKRAR KONTROL ET" : "CHECK AGAIN")}</button></aside>
    </section>
  {/if}
  <RecentFilesList {recent} {language} disabled={restoring||(ffmpegStatus!==null&&!ffmpegStatus.ready)} onopen={onopenrecent}/>
  <div class="landing-copy motto-only">
    <h2>{t("landingTitle")}</h2>
  </div>
  <footer class="landing-engine-status"><span class="status-dot" class:missing={(ffmpegStatus !== null && !ffmpegStatus.ready)||(downloaderStatus!==null&&!downloaderStatus.ready)}></span> {ffmpegStatus?.ready&&downloaderStatus?.ready ? `FFMPEG · FFPROBE · YT-DLP ${t("ready").toUpperCase()}` : (dependencyChecking||downloaderStatus===null ? (language==="tr"?"BİLEŞENLER KONTROL EDİLİYOR":"CHECKING COMPONENTS") : (language==="tr"?"MEDYA BİLEŞENLERİ GEREKLİ":"MEDIA COMPONENTS REQUIRED"))}</footer>
  <button class="downloader-quick-trigger" onclick={ondownloader} title={language==="tr"?"Bağlantıdan indir":"Download from link"}>
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 3v11M8 10l4 4 4-4M5 18v2a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2" /></svg>
    <span>{language==="tr"?"Bağlantıdan indir":"Download from link"}</span>
  </button>
  {#if cleanupMessage}<div class="output-clean-toast" role="status">{cleanupMessage}</div>{/if}
</section>
