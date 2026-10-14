<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { RecentFiles } from "./recentFiles.svelte";
  let { recent, language, disabled=false, onopen }:{ recent:RecentFiles; language:"tr"|"en"; disabled?:boolean; onopen:(path:string)=>void }=$props();
  let missing=$state(new Set<string>());
  const icons={video:"▶",audio:"♪",image:"▣",project:"◆"};
  const kindLabels={tr:{video:"Video",audio:"Ses",image:"Görsel",project:"Proje"},en:{video:"Video",audio:"Audio",image:"Image",project:"Project"}};
  const name=(path:string)=>path.split(/[\\/]/).pop()??path;
  const folder=(path:string)=>path.slice(0,Math.max(0,path.length-name(path).length-1));
  function when(time:number){
    const minutes=Math.round((Date.now()-time)/60000);
    if(minutes<1)return language==="tr"?"az önce":"just now";
    if(minutes<60)return language==="tr"?`${minutes} dk önce`:`${minutes} min ago`;
    const hours=Math.round(minutes/60);
    if(hours<24)return language==="tr"?`${hours} saat önce`:`${hours} h ago`;
    return new Date(time).toLocaleDateString(language==="tr"?"tr-TR":"en-US",{day:"numeric",month:"short"});
  }
  // Files may have been moved or deleted since they were opened.
  $effect(()=>{
    const paths=recent.items.map(item=>item.path);
    let cancelled=false;
    void Promise.all(paths.map(async path=>[path,await invoke<boolean>("project_media_available",{path}).catch(()=>true)] as const))
      .then(results=>{if(!cancelled)missing=new Set(results.filter(([,exists])=>!exists).map(([path])=>path))});
    return()=>{cancelled=true};
  });
</script>

{#if recent.items.length}
  <section class="recent-files" aria-labelledby="recent-files-title">
    <header><h3 id="recent-files-title">{language==="tr"?"Son açılanlar":"Recent files"}</h3><button class="recent-clear" onclick={()=>recent.clear()}>{language==="tr"?"Listeyi temizle":"Clear list"}</button></header>
    <ul>
      {#each recent.items as item (item.path)}
        {@const gone=missing.has(item.path)}
        <li class:missing={gone}>
          <button class="recent-open" disabled={disabled||gone} title={item.path} onclick={()=>onopen(item.path)}>
            <i aria-hidden="true">{icons[item.kind]}</i>
            <span><b>{name(item.path)}</b><small>{gone?(language==="tr"?"Dosya bulunamadı":"File not found"):folder(item.path)}</small></span>
            <em>{kindLabels[language][item.kind]} · {when(item.openedAt)}</em>
          </button>
          <button class="recent-remove" aria-label={language==="tr"?`${name(item.path)} listeden kaldır`:`Remove ${name(item.path)} from the list`} title={language==="tr"?"Listeden kaldır":"Remove from list"} onclick={()=>recent.remove(item.path)}>×</button>
        </li>
      {/each}
    </ul>
  </section>
{/if}
