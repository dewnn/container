<script lang="ts">
  import {openPath,revealItemInDir} from "@tauri-apps/plugin-opener";
  import {reportProblem} from "./toast";
  import PremiereSend from "./PremiereSend.svelte";
  let {path,language,onedit,editDisabled=false}:{path:string;language:"tr"|"en";onedit?:(path:string)=>void;editDisabled?:boolean}=$props();
  const playable=$derived(/\.(mp4|mov|mkv|webm|mp3|wav|m4a|aac|flac|ogg)$/i.test(path));
</script>
<div class="output-actions">
  <button class="ghost" onclick={()=>openPath(path).catch(reportProblem)}>{playable?(language==="tr"?"oynat":"play"):(language==="tr"?"Aç":"Open")}</button>
  <button class="ghost" onclick={()=>revealItemInDir(path).catch(reportProblem)}>{language==="tr"?"klasörde göster":"show in folder"}</button>
  {#if onedit}<button class="ghost" disabled={editDisabled} onclick={()=>onedit?.(path)}>{language==="tr"?"Düzenleyicide aç":"Open in editor"}</button>{/if}
  <PremiereSend {path} {language} disabled={editDisabled}/>
</div>
