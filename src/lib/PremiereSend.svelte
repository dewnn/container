<script lang="ts">
  import {invoke} from "@tauri-apps/api/core";
  import {premiere,refreshPremiere} from "./premiereBridge.svelte";
  import {reportProblem} from "./toast";
  let {path,language,disabled=false}:{path:string;language:"tr"|"en";disabled?:boolean}=$props();
  async function send(){
    if(disabled||premiere.sending)return;
    const target={...premiere.status};if(!target.connected)return;
    premiere.sending=true;
    try{
      await invoke("premiere_send",{path,project:target.project,sequenceId:target.sequence_id});
      window.dispatchEvent(new CustomEvent("container-toast",{detail:{kind:"success",message:language==="tr"?"Render için ayrı Premiere projesi ve timeline oluşturuldu.":"A separate Premiere project and matching timeline were created."}}));
    }catch(error){reportProblem(error)}finally{premiere.sending=false;void refreshPremiere()}
  }
</script>
{#if (premiere.status.connected||premiere.sending)&&/\.(mp4|mov|mkv|webm|avi|m4v)$/i.test(path)}
  <div class="premiere-output-row"><span class="premiere-connected">Premiere <small>{premiere.sending?(language==="tr"?"aktarılıyor…":"sending…"):(language==="tr"?"bağlı":"connected")}</small></span><button class="ghost premiere-send" disabled={disabled||premiere.sending} onclick={send} title={language==="tr"?"Bu video için ayrı Premiere projesi ve videoya uygun timeline oluşturur; mevcut projene eklemez.":"Create a separate Premiere project and matching timeline for this video; do not append to the current project."}>{premiere.sending?(language==="tr"?"Aktarılıyor…":"Sending…"):(language==="tr"?"Premiere’e aktar":"Send to Premiere")}</button></div>
{/if}
