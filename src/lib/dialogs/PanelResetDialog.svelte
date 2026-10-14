<script lang="ts">
  let { language, workspaceMode, onclose, onconfirm }:{ language:"tr"|"en"; workspaceMode:"toolbox"|"autocut"|"batch"; onclose:()=>void; onconfirm:()=>void }=$props();
  function mount(node:HTMLDialogElement){
    node.showModal();
    node.querySelector<HTMLButtonElement>(".panel-reset-cancel")?.focus();
    return {destroy(){if(node.open)node.close()}};
  }
</script>

<dialog class="panel-reset-dialog" use:mount oncancel={onclose} aria-labelledby="panel-reset-title" aria-describedby="panel-reset-description">
  <header><span class="panel-reset-dialog-icon" aria-hidden="true">↺</span><h2 id="panel-reset-title">{language==="tr"?"Panel düzenini sıfırla":"Reset panel layout"}</h2></header>
  <p id="panel-reset-description">{workspaceMode==="autocut"?(language==="tr"?"SmartCut yan panelleri ve zaman çizelgesi varsayılan boyutlarına dönecek.":"SmartCut side panels and timeline will return to their default sizes."):workspaceMode==="batch"?(language==="tr"?"Batch kontrol paneli varsayılan genişliğine dönecek.":"The Batch controls panel will return to its default width."):(language==="tr"?"Araçlar ve Parametreler panelleri varsayılan genişliklerine dönecek.":"Tools and Parameters will return to their default widths.")} {language==="tr"?"Proje ve düzenleme geçmişin değişmeyecek.":"Your project and editing history will stay unchanged."}</p>
  <footer><button class="panel-reset-cancel" onclick={onclose}>{language==="tr"?"İPTAL":"CANCEL"}</button><button class="panel-reset-confirm" onclick={onconfirm}>{language==="tr"?"SIFIRLA":"RESET"}</button></footer>
</dialog>
