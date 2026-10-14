<script lang="ts">
  import { SHORTCUT_GROUPS } from "../playerShortcuts";
  let { language, onclose }:{ language:"tr"|"en"; onclose:()=>void }=$props();
  const index=$derived(language==="tr"?0:1);
  function mount(node:HTMLDialogElement){
    node.showModal();
    node.querySelector<HTMLButtonElement>(".shortcuts-close")?.focus();
    return {destroy(){if(node.open)node.close()}};
  }
</script>

<dialog class="shortcuts-dialog panel" use:mount oncancel={onclose} aria-labelledby="shortcuts-title">
  <header><h2 id="shortcuts-title">{language==="tr"?"Klavye kısayolları":"Keyboard shortcuts"}</h2><button class="ghost shortcuts-close" onclick={onclose} aria-label={language==="tr"?"Kapat":"Close"}>×</button></header>
  {#each SHORTCUT_GROUPS as group}
    <section>
      <h3>{group.title[index]}</h3>
      <dl>
        {#each group.items as item}
          <div><dt>{#each item.keys as key, keyIndex}{#if keyIndex}<i>/</i>{/if}<kbd>{key}</kbd>{/each}</dt><dd>{item.label[index]}</dd></div>
        {/each}
      </dl>
    </section>
  {/each}
  <p>{language==="tr"?"Kısayollar yazı alanlarında ve kaydırıcılarda çalışmaz.":"Shortcuts are ignored while typing or adjusting a slider."}</p>
</dialog>
