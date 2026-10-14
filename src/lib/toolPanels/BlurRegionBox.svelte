<script lang="ts">
  import type { Tool } from "../tools";
  let { mediaBoxStyle, selected, startEffectRegion, toolNumber, toolValue, transformHandles, transformSourceBox=$bindable() }:{ mediaBoxStyle:()=>string; selected:Tool|null; startEffectRegion:(event:PointerEvent,mode:"move"|"n"|"s"|"e"|"w"|"nw"|"ne"|"sw"|"se")=>void; toolNumber:(key:string)=>number; toolValue:(key:string)=>string; transformHandles:readonly ("n"|"s"|"e"|"w"|"nw"|"ne"|"sw"|"se")[]; transformSourceBox:HTMLElement|null }=$props();
</script>

{#if selected?.id === "blur_pixelate"}
  <div class="transform-source-box" bind:this={transformSourceBox} style={mediaBoxStyle()}>
    <div class="effect-region" class:pixelated={toolValue("effect")==="pixelate"} style:left={`${toolNumber("region_x")}%`} style:top={`${toolNumber("region_y")}%`} style:width={`${toolNumber("region_w")}%`} style:height={`${toolNumber("region_h")}%`} style={`--effect-strength:${Math.max(1,toolNumber("strength")/3)}px`} onpointerdown={(event)=>startEffectRegion(event,"move")} role="presentation">
      {#each transformHandles as handle}<button class={`crop-handle ${handle}`} aria-label={`Resize effect ${handle}`} onpointerdown={(event)=>startEffectRegion(event,handle)}></button>{/each}
    </div>
  </div>
{/if}
