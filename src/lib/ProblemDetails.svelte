<script lang="ts">
  import {renderProblem} from "./renderFeedback";
  let {reason,language}:{reason:string;language:"tr"|"en"}=$props();
</script>
<div class="problem-details" role="alert">
  <p>{renderProblem(reason,language)??(language==="tr"?"İşlem tamamlanamadı. Ayarları ve kaynak dosyayı kontrol ederek yeniden dene.":"The operation could not finish. Check the settings and source file, then retry.")}</p>
  <details><summary>{language==="tr"?"Teknik ayrıntılar":"Technical details"}</summary><pre>{reason}</pre></details>
</div>
<style>.problem-details{font-size:11px;min-width:0}.problem-details p{margin:0 0 6px}.problem-details summary{cursor:pointer}.problem-details pre{white-space:pre-wrap;overflow-wrap:anywhere;max-height:150px;overflow:auto;font-size:10px}</style>
