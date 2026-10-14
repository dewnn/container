<script lang="ts">
  import { containDialog } from "../dialogFocus";
  let { language, message, cleaning, onclose, onclean }:{ language:"tr"|"en"; message:string; cleaning:boolean; onclose:()=>void; onclean:()=>void }=$props();
  const close=()=>{if(!cleaning)onclose()};
</script>

<div class="update-layer output-clean-layer">
  <button class="update-backdrop" aria-label={language==="tr"?"Çıktı temizleme penceresini kapat":"Close output cleanup dialog"} onclick={close}></button>
  <dialog class="update-dialog output-clean-dialog panel" open use:containDialog={close} aria-labelledby="output-clean-title">
    <header><div><span class="output-clean-dialog-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M9 7V4h6v3m3 0-1 13H7L6 7m4 4v6m4-6v6"/></svg></span><h2 id="output-clean-title">{language==="tr"?"CONTAINER OUTPUT TEMİZLE":"CLEAN CONTAINER OUTPUT"}</h2></div><button onclick={onclose} disabled={cleaning} aria-label={language==="tr"?"Kapat":"Close"}>×</button></header>
    <p>{language==="tr"?"Downloads/CONTAINER Output içindeki tüm çıktılar Geri Dönüşüm Kutusu’na taşınacak; klasör yerinde kalacak.":"Everything inside Downloads/CONTAINER Output will be moved to the Recycle Bin; the folder itself will remain."}</p>
    {#if message}<small class="output-clean-error">{message}</small>{/if}
    <footer><button class="ghost" onclick={onclose} disabled={cleaning}>{language==="tr"?"İPTAL":"CANCEL"}</button><button class="clean-confirm" onclick={onclean} disabled={cleaning}>{cleaning?"…":(language==="tr"?"ÇIKTILARI TEMİZLE":"CLEAN OUTPUT")}</button></footer>
  </dialog>
</div>
