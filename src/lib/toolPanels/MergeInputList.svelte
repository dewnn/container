<script lang="ts">
  let { language, addMergeVideos, basename, mergeInputs, moveMergeVideo, removeMergeVideo }:{ language:"tr"|"en"; addMergeVideos:()=>unknown; basename:(path:string)=>string; mergeInputs:string[]; moveMergeVideo:(index:number,delta:number)=>void; removeMergeVideo:(index:number)=>void }=$props();
</script>

<div class="merge-list">
  <button class="merge-add" onclick={addMergeVideos}>＋ {language==="tr"?"VİDEO EKLE":"ADD VIDEOS"}</button>
  {#each mergeInputs as path,index}
    <article><b class="mono">{index+1}</b><span title={path}>{basename(path)}</span><aside><button onclick={()=>moveMergeVideo(index,-1)} disabled={index===0} aria-label={language==="tr"?"Yukarı taşı":"Move up"}>↑</button><button onclick={()=>moveMergeVideo(index,1)} disabled={index===mergeInputs.length-1} aria-label={language==="tr"?"Aşağı taşı":"Move down"}>↓</button><button onclick={()=>removeMergeVideo(index)} aria-label={language==="tr"?"Kaldır":"Remove"}>×</button></aside></article>
  {/each}
</div>
