<script lang="ts">
  import type { MediaInfo } from "../editorTypes";
  import type { ImageView } from "../imageView.svelte";
  let { language, t, view, media, mediaUrl, renderedImageUrl, outputStale }:{ language:"tr"|"en"; t:(key:string)=>string; view:ImageView; media:MediaInfo; mediaUrl:string; renderedImageUrl:string; outputStale:boolean }=$props();
</script>

<div class="image-viewport" class:dragging={view.dragging} bind:this={view.viewport} onwheel={view.wheel} onpointerdown={view.startPan} ondblclick={view.resetView} role="presentation">
  {#if renderedImageUrl&&!outputStale}
    <div class="image-compare" onkeydown={view.compareKey} role="slider" tabindex="0" aria-label={language==="tr"?"Orijinal ve işlenmiş görsel karşılaştırması":"Original and rendered image comparison"} aria-valuemin="0" aria-valuemax="100" aria-valuenow={view.compare}>
      <img class="compare-rendered zoomable-image" style:transform={view.transform()} src={renderedImageUrl} alt={`Rendered ${media.name}`} draggable="false" />
      <div class="compare-original-clip" style:clip-path={`inset(0 ${100-view.compare}% 0 0)`}>
        <img class="compare-original zoomable-image" style:transform={view.transform()} src={mediaUrl} alt={`Original ${media.name}`} draggable="false" onload={view.initialize} />
      </div>
      <span class="compare-label original">{t("original")}</span><span class="compare-label rendered">{t("rendered")}</span>
      <button class="compare-handle" style:left={`${view.compare}%`} onpointerdown={view.startCompare} title={language === "tr" ? "Karşılaştırma çizgisini sürükle" : "Drag the comparison line"}><b>↔</b></button>
    </div>
  {:else}
    <img class="zoomable-image" style:transform={view.transform()} src={mediaUrl} alt={media.name} draggable="false" onload={view.initialize} />
  {/if}
  <div class="image-view-controls">
    <button onclick={() => view.setZoom(view.zoom / 1.2)} aria-label={language === "tr" ? "Uzaklaştır" : "Zoom out"}>−</button>
    <span class="mono">{Math.round(view.zoom * 100)}%</span>
    <button onclick={() => view.setZoom(view.zoom * 1.2)} aria-label={language === "tr" ? "Yakınlaştır" : "Zoom in"}>+</button>
    <button class="fit" onclick={view.resetView}>{language === "tr" ? "SIĞDIR" : "FIT"}</button>
  </div>
  <span class="image-view-hint mono">{language === "tr" ? "TEKERLEK: YAKINLAŞTIR · SÜRÜKLE: TAŞI" : "SCROLL: ZOOM · DRAG: PAN"}</span>
</div>
