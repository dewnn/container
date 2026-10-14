<script lang="ts">
  import type { Tool } from "../tools";
  import type { MediaInfo } from "../editorTypes";
  let { language, media, selected, discordBudget, encoderQualityMode, toolNumber, upscaleDimensions }:{ language:"tr"|"en"; media:MediaInfo; selected:Tool; discordBudget:()=>{videoKbps:number;screenLike:boolean}|null; encoderQualityMode:()=>string; toolNumber:(key:string)=>number; upscaleDimensions:(targetEdge:number)=>{width:number;height:number}|null }=$props();
</script>

{#if selected.id === "discord_compressor"}
  {@const budget = discordBudget()}
  <details class="discord-help">
    <summary><b>?</b><span>{language === "tr" ? "SIKIŞTIRMA REHBERİ" : "COMPRESSION GUIDE"}</span></summary>
  <div class="discord-guide">
    <h4>{language === "tr" ? "KALİTE NASIL KORUNUYOR?" : "HOW QUALITY IS PRESERVED"}</h4>
    <ol>
      <li><b>1</b><span>{language === "tr" ? "Önce seçilen MB sınırından gerçek toplam bitrate hesaplanır." : "The real total bitrate is calculated from the selected MB limit."}</span></li>
      <li><b>2</b><span>{language === "tr" ? "Akıllı ses, dar bütçede 64; orta bütçede 96; rahat bütçede 128 kbps AAC seçer." : "Smart audio uses 64 kbps for tight, 96 kbps for medium, and 128 kbps AAC for roomy budgets."}</span></li>
      <li><b>3</b><span>{language === "tr" ? "Normal videoda çözünürlük ve FPS birlikte ayarlanır. Düşük hareketli ekran/yazı videosu algılanırsa okunabilirlik ve akıcılık için kaynak çözünürlük ile FPS korunur." : "Resolution and FPS are adjusted together for normal footage. For detected low-motion screen/text video, source resolution and FPS are preserved for readability and smoothness."}</span></li>
      <li><b>4</b><span>{language === "tr" ? "İki geçiş, sakin sahnelerden artırdığı alanı hareketli sahnelere verir; çıktı büyük kalırsa güvenli bitrate ile tekrar dener." : "Two-pass encoding gives bits saved on calm scenes to complex motion and retries safely if the result is oversized."}</span></li>
    </ol>
    <div class="codec-note"><b>H.264</b><span>{language === "tr" ? "Discord ve cihazlarla en güvenli uyumluluk." : "Safest compatibility across Discord and devices."}</span><b>H.265</b><span>{language === "tr" ? "Aynı boyutta daha iyi görüntü verebilir; eski cihazlarda siyah ekran veya yalnız ses riski vardır." : "Can look better at the same size; older clients may show a black screen or audio only."}</span></div>
    {#if budget && budget.videoKbps < 180}
      <p class="budget-warning">{language === "tr" ? `Bu video için sınır çok dar: görüntüye yalnızca yaklaşık ${budget.videoKbps} kbps kalıyor. Akıllı mod ${budget.screenLike ? "ekran/yazı içeriğini algıladığı için kaynak çözünürlük ve FPS" : "240p / 15 FPS"} kullanacak. Daha temiz görüntü için en etkili çözüm 50 MB seçmek veya videoyu kısaltmaktır.` : `This target is extremely tight: only about ${budget.videoKbps} kbps remains for video. Smart mode will use ${budget.screenLike ? "source resolution and FPS because screen/text content was detected" : "240p / 15 FPS"}. Choosing 50 MB or trimming the video is the most effective quality improvement.`}</p>
    {:else if budget && budget.videoKbps < 750}
      <p class="budget-warning mild">{language === "tr" ? `Bütçe sınırlı (~${budget.videoKbps} kbps). Akıllı çözünürlük ve FPS önerilir.` : `The budget is limited (~${budget.videoKbps} kbps). Smart resolution and FPS are recommended.`}</p>
    {/if}
    <small>{language === "tr" ? "Önerilen Discord başlangıçları: 20 MB → en fazla 480p, 50 MB → 720p, 100 MB → 1080p, 500 MB → kaynak. Uzun videolarda Akıllı mod bunlardan daha aşağı inebilir." : "Suggested Discord starting points: 20 MB → up to 480p, 50 MB → 720p, 100 MB → 1080p, 500 MB → source. Smart mode may go lower for long videos."}</small>
  </div>
  </details>
{/if}
{#if selected.id === "encode"}
  <div class="quality-guide">
    <h4>{language === "tr" ? "KAYNAK VE CODEC REHBERİ" : "SOURCE & CODEC GUIDE"}</h4>
    <p><b>{language === "tr" ? "Kaynak:" : "Source:"}</b> {media.pixel_format ?? "unknown"}{media.bits_per_raw_sample ? ` · ${media.bits_per_raw_sample}-bit` : ""}{media.color_transfer ? ` · ${media.color_transfer}` : ""}</p>
    <p><b>{encoderQualityMode()} {toolNumber("crf")}:</b> {language==="tr"?(encoderQualityMode()==="CRF"&&toolNumber("crf")===0?"CPU'da kayıpsız; dosya çok büyük.":"Düşük değer = daha temiz ve daha büyük dosya."):(encoderQualityMode()==="CRF"&&toolNumber("crf")===0?"Lossless on CPU; very large file.":"Lower value = cleaner picture and larger file.")}</p>
    <ul>
      <li><b>H.264</b><span>{language === "tr" ? "En uyumlu seçenek." : "Best compatibility."}</span></li>
      <li><b>HEVC</b><span>{language === "tr" ? "Daha küçük; eski cihaz desteği zayıf." : "Smaller; weaker legacy support."}</span></li>
      <li><b>VP9 / AV1</b><span>{language === "tr" ? "Verimli; CPU'da yavaş." : "Efficient; slow on CPU."}</span></li>
    </ul>
    <small>{language === "tr" ? "Yalnızca bu bilgisayarda testten geçen encoder’lar listelenir. Auto uyumlu bit derinliğini korur." : "Only encoders verified on this PC are listed. Auto preserves compatible bit depth."}</small>
  </div>
{/if}
{#if selected.id === "upscale" && media.width && media.height}
  {@const upscaleOutput=upscaleDimensions(toolNumber("target_edge"))}
  <div class="upscale-summary">
    <header><b>VIDEO</b><small>{language==="tr"?"Kaynak algılandı":"Source detected"}</small></header>
    <div><span>{language==="tr"?"Kaynak":"Source"}</span><strong>{media.width}×{media.height}{media.fps?` · ${media.fps.toFixed(2)} FPS`:""}</strong></div>
    <div><span>{language==="tr"?"Çıktı":"Output"}</span><strong>{upscaleOutput?`${upscaleOutput.width}×${upscaleOutput.height}`:"—"}{media.fps?` · ${media.fps.toFixed(2)} FPS`:""}</strong></div>
    <div><span>{language==="tr"?"Ölçekleme":"Scaling"}</span><strong>Lanczos · {language==="tr"?"yüksek kalite":"high quality"}</strong></div>
    <div><span>{language==="tr"?"Ses":"Audio"}</span><strong>{language==="tr"?"uyumluysa kopyala":"copy when compatible"}</strong></div>
    <div><span>{language==="tr"?"Kodlayıcı":"Encoder"}</span><strong>H.264 · CRF 14</strong></div>
  </div>
{/if}
{#if ["encode","cut","remux","extract_audio"].includes(selected.id) && media.audio_tracks.length}
  <div class="codec-note"><b>{language === "tr" ? "SES PARÇALARI" : "AUDIO TRACKS"}</b><span>{language === "tr" ? `${media.audio_tracks.length} parça bulundu. Ana varsayılandır; Tümü parçaları ayrı tutar; Birleştir hepsini tek dengeli ses parçasında toplar.` : `${media.audio_tracks.length} track(s) found. Main is the default; All keeps tracks separate; Merge combines them into one normalized track.`}</span></div>
{/if}
