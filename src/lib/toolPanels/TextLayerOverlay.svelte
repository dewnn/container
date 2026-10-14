<script lang="ts">
  import type { Tool } from "../tools";
  import type { TextLayer } from "../editorTypes";
  let { language, activeTextId, selected, startTextDrag, textLayerStyle, textLayers, textPreviewCanvas=$bindable(), textPreviewCanvasStyle, wrappedText }:{ language:"tr"|"en"; activeTextId:number|null; selected:Tool|null; startTextDrag:(event:PointerEvent,layer:TextLayer,resizeDirection?:-1|0|1,verticalDirection?:-1|0|1)=>void; textLayerStyle:(layer:TextLayer)=>string; textLayers:TextLayer[]; textPreviewCanvas:HTMLCanvasElement|null; textPreviewCanvasStyle:()=>string; wrappedText:(layer:TextLayer,canvasWidth?:number)=>string }=$props();
</script>

{#if selected?.id === "text"}
  <div class="text-preview-layer">
    <canvas class="text-preview-canvas" bind:this={textPreviewCanvas} style={textPreviewCanvasStyle()}></canvas>
    {#each textLayers as layer (layer.id)}
      <div class="preview-text" class:active={activeTextId===layer.id} style={textLayerStyle(layer)} role="group" aria-label={`${language==="tr"?"Yazı katmanı":"Text layer"}: ${layer.text}`} onpointerdown={(event)=>startTextDrag(event,layer)}>
        <i class="text-size-handle left" role="presentation" aria-label={language==="tr"?"Yazıyı soldan boyutlandır":"Resize text from left"} onpointerdown={(event)=>startTextDrag(event,layer,-1)}></i>
        <span>{wrappedText(layer)}</span>
        <i class="text-size-handle right" role="presentation" aria-label={language==="tr"?"Yazıyı sağdan boyutlandır":"Resize text from right"} onpointerdown={(event)=>startTextDrag(event,layer,1)}></i>
        <i class="text-size-handle nw" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,-1,-1)}></i><i class="text-size-handle ne" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,1,-1)}></i><i class="text-size-handle sw" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,-1,1)}></i><i class="text-size-handle se" role="presentation" onpointerdown={(event)=>startTextDrag(event,layer,1,1)}></i>
      </div>
    {/each}
  </div>
{/if}
