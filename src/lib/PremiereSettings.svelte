<script lang="ts">
  import {invoke} from "@tauri-apps/api/core";
  import {premiere,refreshPremiere} from "./premiereBridge.svelte";
  import {reportProblem} from "./toast";
  let {language}:{language:"tr"|"en"}=$props();let message=$state("");let confirmation=$state<HTMLDialogElement>();
  let cleanupConfirmation=$state<HTMLDialogElement>();let cleaning=$state(false);
  async function cleanProjects(){
    if(cleaning||premiere.installing||premiere.sending)return;
    cleaning=true;message="";
    try{const count=await invoke<number>("premiere_clean_projects");message=language==="tr"?`${count} proje dosyası Geri Dönüşüm Kutusu’na taşındı.`:`${count} project files moved to Recycle Bin.`}catch(error){reportProblem(error)}finally{cleaning=false}
  }
  async function install(){
    if(cleaning||premiere.installing||premiere.sending)return;
    premiere.installing=true;message="";
    try{await invoke("premiere_install",{consent:true});await refreshPremiere();message=language==="tr"?"Hazır. Premiere’i yeniden başlat.":"Ready. Restart Premiere."}catch(error){reportProblem(error)}finally{premiere.installing=false}
  }
  async function remove(){
    if(cleaning||premiere.installing||premiere.sending)return;
    premiere.installing=true;message="";
    try{await invoke("premiere_remove");await refreshPremiere();message=language==="tr"?"Bağlantı kaldırıldı. Premiere’i yeniden başlat.":"Disconnected. Restart Premiere."}catch(error){reportProblem(error)}finally{premiere.installing=false}
  }
</script>
{#if premiere.status.supported}
<section class="workspace-layout premiere-settings"><h3>Premiere Pro 2025 / 2026</h3>
  <p>{premiere.status.connected?(language==="tr"?"Bağlı":"Connected"):premiere.status.installed?(language==="tr"?"Premiere’i aç":"Open Premiere"):(language==="tr"?"Renderlarını Premiere’e gönder.":"Send your renders to Premiere.")}</p>
  {#if premiere.status.installed}<button class="ghost" disabled={cleaning||premiere.installing||premiere.sending} onclick={()=>confirmation?.showModal()} title={language==="tr"?"Bağlantı bileşenini güncelle veya onar":"Update or repair the connection"}>{language==="tr"?"Güncelle":"Update"}</button><button class="ghost" disabled={cleaning||premiere.installing||premiere.sending} onclick={remove} title={language==="tr"?"Premiere bağlantısını kaldır":"Remove the Premiere connection"}>{language==="tr"?"Bağlantıyı kaldır":"Disconnect"}</button>{:else}<button class="ghost" disabled={cleaning||premiere.installing||premiere.sending} onclick={()=>confirmation?.showModal()}>{language==="tr"?"Bağlan":"Connect"}</button>{/if}
  <button class="ghost" disabled={cleaning||premiere.installing||premiere.sending} onclick={()=>cleanupConfirmation?.showModal()}>{language==="tr"?"Proje dosyalarını temizle":"Clean project files"}</button>
  {#if message}<p role="status">{message}</p>{/if}
  <dialog bind:this={cleanupConfirmation} class="premiere-confirm-dialog panel" aria-labelledby="premiere-clean-title">
    <header><h2 id="premiere-clean-title">{language==="tr"?"Proje dosyalarını temizle":"Clean project files"}</h2></header>
    <p>{language==="tr"?"Önce Premiere’i kapat. CONTAINER’ın otomatik oluşturduğu Premiere projeleri, içlerinde sonradan yaptığın düzenlemelerle birlikte Geri Dönüşüm Kutusu’na taşınır. Render dosyaların ve diğer projelerin değişmez.":"Close Premiere first. Premiere projects automatically created by CONTAINER, including any edits you made in them, will move to Recycle Bin. Render files and other projects stay unchanged."}</p>
    <footer><button class="ghost" onclick={()=>cleanupConfirmation?.close()}>{language==="tr"?"Vazgeç":"Cancel"}</button><button class="premiere-confirm-install" onclick={()=>{cleanupConfirmation?.close();void cleanProjects()}}>{language==="tr"?"Temizle":"Clean"}</button></footer>
  </dialog>
  <dialog bind:this={confirmation} class="premiere-confirm-dialog panel" aria-labelledby="premiere-confirm-title">
    <header><span class="premiere-mark" aria-hidden="true">Pr</span><h2 id="premiere-confirm-title">{language==="tr"?"Premiere’e bağlan":"Connect Premiere"}</h2></header>
    <p>{language==="tr"?"Bu kullanıcı için bağlantı kurulacak. Projelerin ve render dosyaların değişmez.":"Connect for this user only. Your projects and renders stay unchanged."}</p>
    <div class="premiere-setup-warning"><b>{language==="tr"?"Adobe izin ayarı":"Adobe permission setting"}</b><p>{language==="tr"?"CEP 12 PlayerDebugMode açılacak. Bu paylaşılan Adobe ayarı, diğer imzasız CEP eklentilerinin de yüklenmesine izin verir.":"CEP 12 PlayerDebugMode will be enabled. This shared Adobe setting also allows other unsigned CEP extensions to load."}</p></div>
    <small>{language==="tr"?"Sonrasında Premiere’i yeniden başlat.":"Restart Premiere afterward."}</small>
    <footer><button class="ghost" onclick={()=>confirmation?.close()}>{language==="tr"?"Vazgeç":"Cancel"}</button><button class="premiere-confirm-install" onclick={()=>{confirmation?.close();void install()}}>{language==="tr"?"Devam":"Continue"}</button></footer>
  </dialog>
</section>
{/if}
