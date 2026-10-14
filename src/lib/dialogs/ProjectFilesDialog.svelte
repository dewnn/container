<script lang="ts">
  import type { ProjectResource } from "../projectResources";
  import { containDialog } from "../dialogFocus";
  let { language, checks, onclose }:{ language:"tr"|"en"; checks:{resource:ProjectResource;exists:boolean}[]; onclose:()=>void }=$props();
</script>

<div class="update-layer project-files-layer">
  <button class="update-backdrop" aria-label={language==="tr"?"Proje dosyaları penceresini kapat":"Close project files"} onclick={onclose}></button>
  <dialog class="update-dialog project-files-dialog panel" open use:containDialog={onclose} aria-labelledby="project-files-title">
    <header><div><span class="status-dot"></span><h2 id="project-files-title">{language==="tr"?"PROJE DOSYALARI":"PROJECT FILES"}</h2></div><button aria-label={language==="tr"?"Kapat":"Close"} onclick={onclose}>×</button></header>
    <p>{language==="tr"?"Proje dosyası medya içermez. Projeyi başka yere taşıyacaksan kaynakları aynı klasör düzeniyle yanında tut; kayıt sırasında göreli yollar da saklanır.":"Project files do not contain media. Keep the sources in the same folder layout when moving a project; relative paths are saved as a fallback."}</p>
    <div class="project-file-list">{#each checks as item}<div><b class:missing={!item.exists}>{item.exists?"✓":"!"}</b><span><strong>{item.resource.label}</strong><small title={item.resource.path}>{item.resource.path}</small></span></div>{/each}</div>
    {#if checks.some(item=>!item.exists)}<p class="project-files-warning">{language==="tr"?"Eksik dosyaları yeniden bağlamadan bu proje tam olarak işlenemez.":"Missing files must be relinked before the project can be processed completely."}</p>{/if}
    <footer><button class="ghost" onclick={onclose}>{language==="tr"?"KAPAT":"CLOSE"}</button></footer>
  </dialog>
</div>
