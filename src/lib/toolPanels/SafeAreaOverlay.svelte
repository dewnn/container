<script lang="ts">
  import { SAFE_AREAS, safeArea, safeRectangle } from "../safeArea.svelte";
  let { box, language }:{ box:{left:number;top:number;width:number;height:number}|null; language:"tr"|"en" }=$props();
  const area=$derived(safeArea.platform==="off"?null:SAFE_AREAS[safeArea.platform]);
  const safe=$derived(area?safeRectangle(area.zones):null);
</script>

{#if box&&area&&safe}
  <div class="safe-area-overlay" style={`left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px`} aria-hidden="true">
    <i class="safe-area-zone" style={`left:0;top:0;width:100%;height:${area.zones.top*100}%`}></i>
    <i class="safe-area-zone" style={`left:0;bottom:0;width:100%;height:${area.zones.bottom*100}%`}></i>
    <i class="safe-area-zone" style={`left:0;top:${area.zones.top*100}%;width:${area.zones.left*100}%;bottom:${area.zones.bottom*100}%`}></i>
    <i class="safe-area-zone" style={`right:0;top:${area.zones.top*100}%;width:${area.zones.right*100}%;bottom:${area.zones.bottom*100}%`}></i>
    <b class="safe-area-frame" style={`left:${safe.x*100}%;top:${safe.y*100}%;width:${safe.width*100}%;height:${safe.height*100}%`}><span>{area.name} · {language==="tr"?"güvenli alan":"safe area"}</span></b>
  </div>
{/if}
