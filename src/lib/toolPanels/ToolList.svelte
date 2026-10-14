<script lang="ts">
  import type { MediaKind, Tool } from "../tools";
  import type { MediaInfo } from "../editorTypes";
  import { toolCategory, toolSummary } from "../toolSummaries";
  let { language, t, activeKind, categories, chooseTool, favoriteIds, favoritesOnly=$bindable(), kindTools, media, openOtherKindMatch, otherKindMatch, search=$bindable(), selected, switchKind, toggleFavorite, visibleFavoriteCount }:{ language:"tr"|"en"; t:(key:string)=>string; activeKind:MediaKind; categories:[string,Tool[]][]; chooseTool:(tool:Tool)=>void; favoriteIds:string[]; favoritesOnly:boolean; kindTools:(kind:MediaKind)=>Tool[]; media:MediaInfo; openOtherKindMatch:()=>void; otherKindMatch:{kind:MediaKind;tool:Tool;allowed:boolean}|null; search:string; selected:Tool|null; switchKind:(kind:MediaKind)=>void; toggleFavorite:(id:string)=>void; visibleFavoriteCount:number }=$props();
</script>

<aside class="tool-pane panel">
  <div class="pane-head">
    <div><h3>{t("tools")}</h3><p>{kindTools(activeKind).length} {t("available")}</p></div>
  </div>
  <div class="tabs">
    {#each (["video","audio","image"] as MediaKind[]) as kind}
      <button class:active={activeKind === kind} onclick={() => switchKind(kind)} disabled={kind !== media?.kind && !(media?.kind === "video" && kind === "audio")}>{t(kind)}</button>
    {/each}
  </div>
  <input class="search" bind:value={search} placeholder={t("search")} />
  <button class="favorites-filter" class:active={favoritesOnly} onclick={()=>favoritesOnly=!favoritesOnly}><span>★</span>{language==="tr"?"FAVORİLER":"FAVORITES"}<b>{visibleFavoriteCount}</b></button>
  <div class="tool-scroll">
    {#each categories as [category, entries]}
      <section class="tool-group" data-category={toolCategory(entries[0].id)}>
        {#if entries.length>1&&categories.length>1}<h4>{category}</h4>{/if}
        {#each entries as tool}
          <div class="tool-entry">
            <button class="tool-row" class:active={selected?.id === tool.id} title={tool.description} onclick={() => chooseTool(tool)}>
              <i class:blue={tool.accent === "blue"} class:green={tool.accent === "green"} class:purple={tool.accent === "purple"} class:red={tool.accent === "red"} class:yellow={tool.accent === "yellow"}></i>
              <span><b>{tool.title}</b><small>{toolSummary(tool,language)}</small></span><em>›</em>
            </button>
            <button class="favorite-toggle" class:active={favoriteIds.includes(tool.id)} onclick={()=>toggleFavorite(tool.id)} title={language==="tr"?"Favoriye ekle/kaldır":"Add/remove favorite"} aria-label={language==="tr"?`${tool.title} favori`:`Favorite ${tool.title}`}>★</button>
          </div>
        {/each}
      </section>
    {/each}
    {#if categories.length===0}<div class="favorites-empty"><p>{search.trim()?(otherKindMatch?(otherKindMatch.allowed?(language==="tr"?`${otherKindMatch.tool.title}, ${t(otherKindMatch.kind)} sekmesinde.`:`${otherKindMatch.tool.title} is in the ${t(otherKindMatch.kind)} tab.`):(language==="tr"?`${otherKindMatch.tool.title} yalnızca ${otherKindMatch.kind==="image"?"görsel":otherKindMatch.kind==="audio"?"ses":"video"} dosyalarında kullanılabilir.`:`${otherKindMatch.tool.title} is available for ${otherKindMatch.kind} files only.`)):(language==="tr"?"Aramana uygun araç bulunamadı.":"No matching tools found.")):(language==="tr"?"Bu bölümde henüz favori araç yok.":"No favorite tools in this section yet.")}</p>{#if otherKindMatch?.allowed}<button class="ghost" onclick={openOtherKindMatch}>{language==="tr"?`${otherKindMatch.tool.title} aracını aç`:`Open ${otherKindMatch.tool.title}`}</button>{/if}{#if search.trim()}<button class="ghost" onclick={()=>search=""}>{language==="tr"?"Aramayı temizle":"Clear search"}</button>{/if}{#if favoritesOnly}<button class="ghost" onclick={()=>favoritesOnly=false}>{language==="tr"?"Tüm araçları göster":"Show all tools"}</button>{/if}</div>{/if}
  </div>
</aside>
