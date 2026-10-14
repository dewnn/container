<script lang="ts">
  import type { Tool } from "../tools";
  import { convertFileSrc } from "@tauri-apps/api/core";
  let { language, overlayPreviewImage=$bindable(), overlayPreviewStyle, selected, startOverlayDrag, toolNumber, toolValue, toolboxCurrent }:{ language:"tr"|"en"; overlayPreviewImage:HTMLImageElement|null; overlayPreviewStyle:()=>string; selected:Tool|null; startOverlayDrag:(event:PointerEvent,resizeDirection?:-1|0|1,verticalDirection?:-1|0|1)=>void; toolNumber:(key:string)=>number; toolValue:(key:string)=>string; toolboxCurrent:number }=$props();
</script>

{#if selected?.id === "image_overlay" && toolValue("image_path") && toolboxCurrent >= toolNumber("start") && toolboxCurrent <= toolNumber("end")}
  <button class="overlay-preview-box" style={overlayPreviewStyle()} onpointerdown={(event)=>startOverlayDrag(event)} aria-label={language==="tr"?"Kaplamayı taşı":"Move overlay"}>
    <img bind:this={overlayPreviewImage} src={convertFileSrc(toolValue("image_path"))} alt="Overlay preview" draggable="false" style:opacity={toolNumber("opacity")/100} />
    <i class="text-size-handle left" role="presentation" aria-label={language==="tr"?"Kaplamayı soldan boyutlandır":"Resize overlay from left"} onpointerdown={(event)=>startOverlayDrag(event,-1)}></i>
    <i class="text-size-handle right" role="presentation" aria-label={language==="tr"?"Kaplamayı sağdan boyutlandır":"Resize overlay from right"} onpointerdown={(event)=>startOverlayDrag(event,1)}></i>
    <i class="text-size-handle nw" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,-1,-1)}></i><i class="text-size-handle ne" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,1,-1)}></i><i class="text-size-handle sw" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,-1,1)}></i><i class="text-size-handle se" role="presentation" onpointerdown={(event)=>startOverlayDrag(event,1,1)}></i>
  </button>
{/if}
