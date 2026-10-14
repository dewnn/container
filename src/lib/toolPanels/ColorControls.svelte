<script lang="ts">
  import type { Field } from "../tools";
  let { language, adjustColor, applyColorPreset, colorGroups, colorLabels, colorPreviewVisible=$bindable(), colorValueLabel, displayedColorValue, rangePercent, resetColorFilters, resetColorKey, setToolValue, toolField, toolValue }:{ language:"tr"|"en"; adjustColor:(key:string,value:number)=>void; applyColorPreset:(preset:string)=>void; colorGroups:{title:string;trTitle:string;keys:string[]}[]; colorLabels:Record<string,string>; colorPreviewVisible:boolean; colorValueLabel:(key:string)=>string; displayedColorValue:(key:string)=>number; rangePercent:(value:number,min:number,max:number)=>number; resetColorFilters:()=>void; resetColorKey:(key:string)=>void; setToolValue:(key:string,value:string)=>void; toolField:(key:string)=>Field|undefined; toolValue:(key:string)=>string }=$props();
</script>

<div class="color-workspace">
  <div class="color-top-actions"><button class="reset-filters" onclick={resetColorFilters}>{language==="tr"?"Sıfırla":"Reset"}</button><button class:active={!colorPreviewVisible} class="compare-color" onclick={()=>colorPreviewVisible=!colorPreviewVisible}>{colorPreviewVisible?(language==="tr"?"Öncesini göster":"Show before"):(language==="tr"?"Sonrasını göster":"Show after")}</button></div>
  <section class="color-presets"><h4>{language==="tr"?"Hızlı görünümler":"Quick looks"}</h4><div>{#each [["natural",language==="tr"?"Doğal":"Natural"],["cinematic",language==="tr"?"Sinematik":"Cinematic"],["warm",language==="tr"?"Sıcak":"Warm"],["cold",language==="tr"?"Soğuk":"Cold"],["bw",language==="tr"?"Siyah-beyaz":"B&W"]] as preset}<button onclick={()=>applyColorPreset(preset[0])}>{preset[1]}</button>{/each}</div></section>
  {#each colorGroups as group}
    <section class="color-group"><h4>{language==="tr"?group.trTitle:group.title}</h4>
      {#each group.keys as key}
        {@const field=toolField(key)}
        {#if field}
          <label class="color-control">
            <span><b>{language==="tr"?(field.label):colorLabels[key]}</b><em>{colorValueLabel(key)}</em><button type="button" aria-label={language==="tr"?`${field.label} sıfırla`:`Reset ${colorLabels[key]}`} title={language==="tr"?"Sıfırla":"Reset"} onclick={(event)=>{event.preventDefault();resetColorKey(key)}}>↻</button></span>
            <input aria-label={language==="tr"?field.label:colorLabels[key]} type="range" style={`--range-pct:${rangePercent(displayedColorValue(key),Number(field.min),Number(field.max))}%`} min={field.min} max={field.max} step={field.step} value={displayedColorValue(key)} oninput={(event)=>adjustColor(key,Number(event.currentTarget.value))}>
          </label>
        {/if}
      {/each}
      {#if group.title === "Cleanup"}
        <div class="denoise-control">
          <span>{language==="tr"?"Gürültü azaltma":"Denoise"}</span>
          <div class="segmented">{#each [["off",language==="tr"?"Kapalı":"Off"],["low",language==="tr"?"Az":"Low"],["medium",language==="tr"?"Orta":"Medium"],["high",language==="tr"?"Yüksek":"High"]] as mode}<button class:active={toolValue("denoise")===mode[0]} onclick={()=>setToolValue("denoise",mode[0])}>{mode[1]}</button>{/each}</div>
        </div>
      {/if}
      {#if group.title === "Style"}
        <button class="ghost color-grayscale" aria-pressed={toolValue("grayscale")==="on"} onclick={()=>setToolValue("grayscale",toolValue("grayscale")==="on"?"off":"on")}>{language==="tr"?"Gri tonlama":"Grayscale"}</button>
      {/if}
    </section>
  {/each}
  <section class="color-group"><h4>{language==="tr"?"Tarama":"Interlace"}</h4><div class="segmented">{#each ["off","auto","on"] as mode}<button class:active={toolValue("deinterlace")===mode} onclick={()=>setToolValue("deinterlace",mode)}>{language==="tr"?({off:"kapalı",auto:"otomatik",on:"açık"} as Record<string,string>)[mode]:mode}</button>{/each}</div></section>
</div>
