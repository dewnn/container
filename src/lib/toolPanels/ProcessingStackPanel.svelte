<script lang="ts">
  import type { Tool } from "../tools";
  import type { StackStep } from "../editorTypes";
  let { language, addOrUpdateStackStep, editingStackStepId, flushEditorSnapshot, moveStackStep, operationBusy, persistRecovery, processingStack, removeStackStep, selectStackStep, selected, stackDraftDirty, stackToolIds }:{ language:"tr"|"en"; addOrUpdateStackStep:()=>void; editingStackStepId:number|null; flushEditorSnapshot:()=>void; moveStackStep:(index:number,change:number)=>void; operationBusy:boolean; persistRecovery:()=>void; processingStack:StackStep[]; removeStackStep:(id:number)=>void; selectStackStep:(step:StackStep)=>unknown; selected:Tool; stackDraftDirty:()=>boolean; stackToolIds:Set<string> }=$props();
</script>

<section class="processing-stack" class:empty={!processingStack.length} aria-label={language==="tr"?"İşlem listesi":"Processing Stack"}>
  <header><b>{language==="tr"?"İŞLEM LİSTESİ":"PROCESSING STACK"}</b><small>{processingStack.length}</small></header>
  {#if processingStack.length}
    <div class="processing-stack-list">
      {#each processingStack as step,index (step.id)}
        <div class:current={editingStackStepId===step.id} class="processing-stack-step">
          <input type="checkbox" aria-label={`${step.tool.title} ${language==="tr"?"etkin":"enabled"}`} checked={step.enabled} disabled={operationBusy} onchange={(event)=>{step.enabled=event.currentTarget.checked;flushEditorSnapshot();persistRecovery()}}>
          <button class="processing-stack-name" onclick={()=>selectStackStep(step)} disabled={operationBusy}>{index+1}. {step.tool.title}</button>
          <button aria-label={language==="tr"?"Yukarı taşı":"Move up"} onclick={()=>moveStackStep(index,-1)} disabled={operationBusy||index===0||step.tool.id==="smartcut"||processingStack[index-1]?.tool.id==="smartcut"}>↑</button>
          <button aria-label={language==="tr"?"Aşağı taşı":"Move down"} onclick={()=>moveStackStep(index,1)} disabled={operationBusy||index===processingStack.length-1||step.tool.id==="smartcut"}>↓</button>
          <button aria-label={language==="tr"?"Adımı kaldır":"Remove step"} onclick={()=>removeStackStep(step.id)} disabled={operationBusy}>×</button>
        </div>
      {/each}
    </div>
  {/if}
  {#if stackToolIds.has(selected.id)}<button class="processing-stack-add" onclick={addOrUpdateStackStep} disabled={operationBusy}>{editingStackStepId!==null&&processingStack.some(step=>step.id===editingStackStepId)?(language==="tr"?"Seçili adımı güncelle":"Update selected step"):(language==="tr"?"+ Listeye ekle":"+ Add to stack")}</button>{/if}
  {#if stackDraftDirty()}<small class="processing-stack-draft">{language==="tr"?"Bu adımda kaydedilmemiş ayarlar var.":"This step has unapplied settings."}</small>{/if}
  {#if processingStack.length}<details class="stack-help"><summary>{language==="tr"?"Liste nasıl çalışır?":"How does the Stack work?"}</summary><p>{language==="tr"?"Araç önizlemesi yalnızca seçili aracı gösterir. Tüm adımların birleşmiş sonucunu renderdan sonra Stack sonucu ile izle. Cut zamanları orijinal kaynak videoya aittir; önceki kesimlerde kaldırılan bölümler dahil edilmez. Ara adımlar kayıpsız dosyalar kullanır ve ek disk alanı gerektirir.":"Tool preview shows only the selected tool. View the combined result with Stack result after rendering. Cut times refer to the original source; regions removed by earlier cuts are excluded. Intermediate steps use lossless files and require extra disk space."}</p></details>{/if}
</section>
