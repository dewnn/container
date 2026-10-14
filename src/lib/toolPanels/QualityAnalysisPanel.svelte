<script lang="ts">
  import type { Tool } from "../tools";
  import type { QualityAnalysis } from "../editorTypes";
  let { language, analyzeCompression, applyQualityRecommendation, busy, qualityAnalysis, qualityAnalyzing, qualityRating, selected }:{ language:"tr"|"en"; analyzeCompression:()=>unknown; applyQualityRecommendation:()=>void; busy:boolean; qualityAnalysis:QualityAnalysis|null; qualityAnalyzing:boolean; qualityRating:(value:string)=>string; selected:Tool }=$props();
</script>

<div class="quality-guide">
  <h4>{language === "tr" ? "CRF VE AKILLI ANALİZ" : "CRF & SMART ANALYSIS"}</h4>
  <p>{language === "tr" ? "CRF 0 gerçek kayıpsızdır fakat çok büyük dosya üretir. CRF 16 çok yüksek kalitedir; 17 de kayıpsız değildir. Değer yükseldikçe dosya küçülür ve kalite kademeli azalır. VMAF analizi kaynak videoya uygun değeri ölçer." : "CRF 0 is truly lossless but creates a very large file. CRF 16 is very high quality; 17 is not lossless either. Higher values reduce size and gradually reduce quality. VMAF can measure a suitable value for this source."}</p>
  <ul>
    <li><b>95–100</b><span>{language === "tr" ? "Neredeyse kayıpsız görünür." : "Looks nearly transparent."}</span></li>
    <li><b>90–94</b><span>{language === "tr" ? "Çoğu kullanım için çok iyi." : "Very good for most uses."}</span></li>
    <li><b>85–89</b><span>{language === "tr" ? "İyi; hareket ve dokuda fark çıkabilir." : "Good; motion and textures may differ."}</span></li>
    <li><b>&lt;85</b><span>{language === "tr" ? "Kalite kaybı belirginleşir." : "Quality loss becomes obvious."}</span></li>
  </ul>
  <small>{language === "tr" ? "Hız için örnekler en fazla 720p ölçülür. Boyut tahmini yalnızca video akışıdır; ses ve kapsayıcı birkaç MB ekleyebilir." : "For speed, samples are measured at up to 720p. The size estimate covers video only; audio and container overhead may add a few MB."}</small>
  <button class="analyze-video" onclick={analyzeCompression} disabled={qualityAnalyzing||busy}>{qualityAnalyzing?(language==="tr"?"Analiz ediliyor…":"Analyzing…"):(language==="tr"?"Videoyu analiz et":"Analyze Video")}</button>
</div>
{#if qualityAnalysis}
  <div class="quality-result">
    <div class="quality-verdict"><span>{language === "tr" ? "ÖNERİLEN" : "RECOMMENDED"}</span><b>CRF {qualityAnalysis.recommended_crf}</b><small>{language === "tr" ? `Hedef VMAF ${qualityAnalysis.target_vmaf.toFixed(0)} · ${qualityAnalysis.sample_count} bölgeden ${qualityAnalysis.sampled_seconds.toFixed(1)} sn incelendi` : `Target VMAF ${qualityAnalysis.target_vmaf.toFixed(0)} · ${qualityAnalysis.sampled_seconds.toFixed(1)} sec across ${qualityAnalysis.sample_count} regions`}</small></div>
    <div class="quality-table">
      <div class="quality-table-head"><span>CRF</span><span>VMAF</span><span>{language === "tr" ? "TAHMİN" : "ESTIMATE"}</span></div>
      {#each qualityAnalysis.candidates as candidate}
        <article class:chosen={candidate.crf === qualityAnalysis.recommended_crf}>
          <b>{candidate.crf}</b><strong>{candidate.vmaf.toFixed(1)}</strong><span>~{candidate.estimated_size_mb.toFixed(1)} MB</span>
          <small>{qualityRating(candidate.rating)}</small>
        </article>
      {/each}
    </div>
    <p>{language === "tr" ? `CRF ${qualityAnalysis.recommended_crf}, seçtiğin kalite hedefi için önerilir. Bu bir tahmindir; kesin dosya boyutu sahnelere göre değişebilir.` : `CRF ${qualityAnalysis.recommended_crf} is recommended for the selected quality goal. This is an estimate; final size may vary by scene.`}</p>
    <button class="apply-quality" onclick={applyQualityRecommendation}>{language==="tr"?`Öneriyi uygula · CRF ${qualityAnalysis.recommended_crf}`:`Apply Recommendation · CRF ${qualityAnalysis.recommended_crf}`}</button>
  </div>
{/if}
