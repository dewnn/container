<script lang="ts">
  import type { Update } from "@tauri-apps/plugin-updater";
  import { containDialog } from "../dialogFocus";
  let { language, appVersion, availableUpdate, status, checking, installing, downloaded, total, onclose, oncheck, oninstall }:{
    language:"tr"|"en"; appVersion:string; availableUpdate:Update|null; status:string; checking:boolean; installing:boolean;
    downloaded:number; total:number; onclose:()=>void; oncheck:()=>void; oninstall:()=>void;
  }=$props();
  const close=()=>{if(!installing)onclose()};
</script>

<div class="update-layer">
  <button class="update-backdrop" aria-label={language === "tr" ? "Güncelleme penceresini kapat" : "Close update dialog"} onclick={close}></button>
  <dialog class="update-dialog panel" open use:containDialog={close} aria-labelledby="update-title">
    <header><div><span class="status-dot"></span><h2 id="update-title">CONTAINER UPDATE</h2></div><button onclick={onclose} disabled={installing} aria-label={language === "tr" ? "Kapat" : "Close"}>×</button></header>
    <div class="update-version"><span>v{appVersion}</span><b>→</b><strong>{availableUpdate ? `v${availableUpdate.version}` : `v${appVersion}`}</strong></div>
    <p>{status}</p>
    {#if availableUpdate?.body}<pre>{availableUpdate.body}</pre>{/if}
    {#if installing}
      <div class="update-progress"><i style:width={`${total ? Math.min(100, downloaded / total * 100) : 8}%`}></i></div>
      <small class="mono">{total ? `${(downloaded/1048576).toFixed(1)} / ${(total/1048576).toFixed(1)} MB` : (language === "tr" ? "hazırlanıyor…" : "preparing…")}</small>
    {/if}
    <footer>
      <button class="ghost" onclick={oncheck} disabled={checking || installing}>{language === "tr" ? "TEKRAR DENE" : "CHECK AGAIN"}</button>
      {#if availableUpdate}<button class="install-update" onclick={oninstall} disabled={installing}>{installing ? (language === "tr" ? "KURULUYOR…" : "INSTALLING…") : (language === "tr" ? "İNDİR VE GÜNCELLE" : "DOWNLOAD & UPDATE")}</button>{/if}
    </footer>
  </dialog>
</div>
