<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import PremiereSend from "../PremiereSend.svelte";
  import ProblemDetails from "../ProblemDetails.svelte";
  import RenderFeedback from "../RenderFeedback.svelte";
  let { language, t, busy, cancelJob, continueEditingOutput, elapsed, error, frame, jobStatus, jobStatusLabel, operationBusy, output, outputStale, playRenderedOutput, progress, showToast, speed }:{ language:"tr"|"en"; t:(key:string)=>string; busy:boolean; cancelJob:()=>unknown; continueEditingOutput:(path?:string)=>unknown; elapsed:number; error:string; frame:string; jobStatus:string; jobStatusLabel:(value:string)=>string; operationBusy:boolean; output:string; outputStale:boolean; playRenderedOutput:()=>unknown; progress:number; showToast:(reason:unknown,kind?:"error"|"info"|"success")=>void; speed:string }=$props();
</script>

<div class="job panel">
  <div class="job-head">
    <div class="job-status"><h3>{t("process")}</h3><div class="job-status-line"><p class="mono">{outputStale?(language==="tr"?"ayarlar değişti · yeniden işle":"settings changed · render again"):jobStatusLabel(jobStatus)}</p></div></div>
    <div class="job-meta">
      <div class="job-stats mono" class:idle={!busy&&!output&&elapsed===0}><span><b>{t("frame")}</b>{frame}</span><span><b>{t("speed")}</b>{speed}</span><span><b>{t("elapsed")}</b>{elapsed.toFixed(1)}s</span></div>
      {#if output||busy}<div class="job-actions">{#if output}
        <button class="ghost job-action play-render" disabled={operationBusy} onclick={playRenderedOutput} title={language==="tr"?"Son çıktıyı aç":"Open the last output"}>▶ {output.endsWith(".frames")?(language==="tr"?"Kareler":"Frames"):/\.(png|jpe?g|webp|bmp|tiff?|avif|gif)$/i.test(output)?(language==="tr"?"Görseli aç":"Open image"):(language==="tr"?"Oynat":"Play")}</button>
        <button class="ghost job-action" onclick={() => {if(outputStale)showToast(language==="tr"?"Bu eski çıktı. Yeni ayarları görmek için yeniden işle.":"This is the old output. Render again to see the new settings.","info");revealItemInDir(output)}}>{outputStale?(language==="tr"?"Eski çıktı · güncellenmedi":"Old output · not updated"):t("showOutput")}</button><button class="ghost job-action" disabled={operationBusy||outputStale} title={outputStale?(language==="tr"?"Ayarlar değişti; önce yeniden işle.":"Settings changed; render again first."):undefined} onclick={()=>continueEditingOutput()}>{language==="tr"?"Kaynak olarak aç":"Open as source"}</button>{/if}{#if busy}<button class="danger job-action" onclick={cancelJob}>{t("cancelJob")}</button>{/if}</div>{/if}
    </div>
  </div>
  <div class="job-feedback"><PremiereSend path={output} {language} disabled={operationBusy||outputStale}/><RenderFeedback running={busy} {progress} {language} {speed} {elapsed} {output}/></div>
  <div class="progress-track"><div style:width={`${progress}%`}></div></div>
  {#if error}<div class="error-box"><ProblemDetails reason={error} {language}/></div>{/if}
</div>
