<script lang="ts">
  import { containDialog } from "../dialogFocus";
  let { language, migrationError, checking, onclose, onrepair, oncheck }:{
    language:"tr"|"en"; migrationError:string; checking:boolean; onclose:()=>void; onrepair:()=>void; oncheck:()=>void;
  }=$props();
</script>

<div class="update-layer dependency-layer">
  <button class="update-backdrop" aria-label={language === "tr" ? "FFmpeg bildirimini kapat" : "Close FFmpeg notice"} onclick={onclose}></button>
  <dialog class="update-dialog dependency-dialog panel" open use:containDialog={onclose} aria-labelledby="dependency-title">
    <header><div><span class="status-dot missing"></span><h2 id="dependency-title">{migrationError ? (language === "tr" ? "FFMPEG GÜNCELLEMESİ TAMAMLANMADI" : "FFMPEG UPDATE DID NOT FINISH") : (language === "tr" ? "FFMPEG GEREKLİ" : "FFMPEG REQUIRED")}</h2></div><button onclick={onclose} aria-label={language === "tr" ? "Kapat" : "Close"}>×</button></header>
    <div class="dependency-message"><span>!</span><div><h3>{migrationError ? (language === "tr" ? "SONRAKİ GÜNCELLEME İÇİN BİR ADIM GEREKİYOR" : "ONE STEP IS NEEDED FOR THE NEXT UPDATE") : (language === "tr" ? "MEDYA ARAÇLARI HENÜZ KULLANILAMAZ" : "MEDIA TOOLS ARE NOT READY YET")}</h3><p>{migrationError ? (language === "tr" ? "CONTAINER, FFmpeg bileşenlerini güvenli güncelleme alanına hazırlayamadı. Bu sürüm çalışmaya devam eder; ancak sonraki küçük güncellemelerden önce aşağıdaki işlemi tekrar dene. Sorun sürerse bu sürümü yeniden kur." : "CONTAINER could not prepare its FFmpeg components for future lightweight updates. This version can still run; retry below before the next update. If it continues, reinstall this version.") : (language === "tr" ? "Kurulumla gelen FFmpeg bileşenleri bulunamadı veya çalıştırılamadı. CONTAINER’ı yeniden kurup tekrar dene." : "The FFmpeg components included with CONTAINER are missing or could not start. Reinstall CONTAINER and try again.")}</p>{#if migrationError}<small class="runtime-migration-detail mono">{migrationError}</small>{/if}</div></div>
    <footer><button class="ghost" onclick={onclose}>{language === "tr" ? "ŞİMDİ DEĞİL" : "NOT NOW"}</button><button class="dependency-check" onclick={migrationError ? onrepair : oncheck} disabled={checking}>{checking ? "…" : (language === "tr" ? (migrationError ? "YENİDEN HAZIRLA" : "TEKRAR KONTROL ET") : (migrationError ? "REPAIR RUNTIME" : "CHECK AGAIN"))}</button></footer>
  </dialog>
</div>
