<script lang="ts">
  import type { Tool } from "../tools";
  import type { MediaInfo, StackStep } from "../editorTypes";
  import { imageOutputFormat, imageTargetSupported } from "../imageCompressionUi";
  let { language, t, busy, media, operationBusy, processingStack, qualityAnalyzing, runProcessingStack, runTool, selected, stackDraftDirty, stackQuality=$bindable(), toolValue }:{ language:"tr"|"en"; t:(key:string)=>string; busy:boolean; media:MediaInfo; operationBusy:boolean; processingStack:StackStep[]; qualityAnalyzing:boolean; runProcessingStack:()=>unknown; runTool:()=>unknown; selected:Tool; stackDraftDirty:()=>boolean; stackQuality:string; toolValue:(key:string)=>string }=$props();
</script>

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
    <button class="run stack-render" aria-label={language==="tr"?"İşlem listesini renderla":"Render processing stack"} onclick={runProcessingStack} disabled={busy||qualityAnalyzing||!processingStack.some(step=>step.enabled)||stackDraftDirty()}><span>▶ {language==="tr"?"LİSTEYİ İŞLE":"RENDER STACK"}</span><small>{processingStack.filter(step=>step.enabled).length} {language==="tr"?"adım":"steps"}</small></button>
    <button class="stack-render-single" onclick={runTool} disabled={busy||qualityAnalyzing}>{language==="tr"?"Yalnızca açık aracı işle":"Render current tool only"}</button>
  {:else}
    <button class="run" onclick={runTool} disabled={busy||qualityAnalyzing||(selected.id==="image_compressor"&&toolValue("mode")==="target"&&!imageTargetSupported(imageOutputFormat(media.path,toolValue("format"))))}>▶ {selected.id === "file_hash" ? (language === "tr" ? "SHA-256 HESAPLA" : "CALCULATE SHA-256") : `${t("render")}${language==="tr"?" · ":" "}${selected.title.toLocaleUpperCase(language)}`}</button>
  {/if}
</div>
