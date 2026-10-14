<script lang="ts">
  import type { FontOption, TextLayer } from "../editorTypes";
  let { language, activeText, activeTextId=$bindable(), addTextLayer, applyTextPreset, chooseTextFont, deleteTextPreset, positionText, rangePercent, removeTextLayer, renameTextPreset, resizeTextLayer, saveTextPreset, selectedTextPreset=$bindable(), setTextColor, systemFonts, textColors, textLayers, textPresetBusy, textPresetName=$bindable(), textPresets, updateTextLayer, updateTextPreset }:{ language:"tr"|"en"; activeText:()=>TextLayer|null; activeTextId:number|null; addTextLayer:()=>unknown; applyTextPreset:()=>unknown; chooseTextFont:(path:string)=>unknown; deleteTextPreset:()=>void; positionText:(position:string)=>void; rangePercent:(value:number,min:number,max:number)=>number; removeTextLayer:(id:number)=>void; renameTextPreset:()=>void; resizeTextLayer:(size:number)=>void; saveTextPreset:()=>void; selectedTextPreset:string; setTextColor:(value:string)=>void; systemFonts:FontOption[]; textColors:string[]; textLayers:TextLayer[]; textPresetBusy:boolean; textPresetName:string; textPresets:{id:string;name:string;sourceWidth?:number;layer:TextLayer}[]; updateTextLayer:(patch:Partial<TextLayer>)=>void; updateTextPreset:()=>void }=$props();
</script>

<div class="text-workspace">
  <section class="text-presets">
    <h4 title={language==="tr"?"Seçili yazıyı, konumunu ve tüm görünüm ayarlarını kaydet. Uygula yeni bir katman ekler.":"Save the selected text, position and all appearance settings. Add from preset creates a new layer."}>{language==="tr"?"Yazı presetleri":"Text presets"}</h4>
    {#if textPresets.length}
      <div class="text-preset-row">
        <select aria-label={language==="tr"?"Kayıtlı preset":"Saved preset"} bind:value={selectedTextPreset}><option value="">{language==="tr"?"Preset seç":"Choose a preset"}</option>{#each textPresets as preset (preset.id)}<option value={preset.id}>{preset.name}</option>{/each}</select>
        <button class="ghost" disabled={!selectedTextPreset||textPresetBusy} onclick={applyTextPreset}>{language==="tr"?"Presetten ekle":"Add from preset"}</button>
        <button class="ghost preset-delete" aria-label={language==="tr"?"Preseti sil":"Delete preset"} title={language==="tr"?"Preseti sil":"Delete preset"} disabled={!selectedTextPreset||textPresetBusy} onclick={deleteTextPreset}><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7"/></svg></button>
      </div>
    {/if}
    <div class="text-preset-row">
      <input aria-label={language==="tr"?"Preset adı":"Preset name"} maxlength="80" bind:value={textPresetName} placeholder={language==="tr"?"Yeni preset adı…":"New preset name…"}>
      <button class="ghost" aria-label={language==="tr"?"Seçili yazıyı kaydet":"Save selected text"} title={language==="tr"?"Seçili yazıyı kaydet":"Save selected text"} disabled={!activeText()||!textPresetName.trim()} onclick={saveTextPreset}>{language==="tr"?"Kaydet":"Save"}</button>
    </div>
    {#if selectedTextPreset}<details class="preset-manage"><summary>{language==="tr"?"Seçili preseti düzenle":"Edit selected preset"}</summary><div class="text-preset-row"><button class="ghost" disabled={!activeText()||textPresetBusy} onclick={updateTextPreset}>{language==="tr"?"Seçili yazıyla güncelle":"Update from selected text"}</button><button class="ghost" disabled={!textPresetName.trim()||textPresetBusy} onclick={renameTextPreset}>{language==="tr"?"Adı değiştir":"Rename"}</button></div><small>{language==="tr"?"Adı değiştirmek için üstteki ad alanını kullan.":"Use the name field above to rename."}</small></details>{/if}
  </section>
  <button class="add-text" onclick={addTextLayer}>＋ {language==="tr"?"Yazı ekle":"Add text"}</button>
  {#if textLayers.length}
    <div class="text-tabs">{#each textLayers as layer,index (layer.id)}<div class="text-tab" class:active={activeTextId===layer.id}><button class="text-tab-select" onclick={()=>activeTextId=layer.id}>{index+1}. {layer.text||"—"}</button><button class="text-tab-remove" aria-label={`${language==="tr"?"Yazıyı kaldır":"Remove text"}: ${layer.text||index+1}`} title={language==="tr"?"Yazıyı kaldır":"Remove text"} onclick={()=>removeTextLayer(layer.id)}>×</button></div>{/each}</div>
    {@const layer=activeText()}
    {#if layer}
      <label class="field"><span>{language==="tr"?"Yazı":"Text"}</span><textarea class="text-content-input" rows="3" value={layer.text} oninput={(event)=>updateTextLayer({text:event.currentTarget.value})}></textarea></label>
      <label class="field"><span>{language==="tr"?"Font":"Font"}</span><select value={layer.font_path} onchange={(event)=>chooseTextFont(event.currentTarget.value)}>{#each systemFonts as font}<option value={font.path}>{font.name}</option>{/each}</select></label>
      <div class="text-color-editor">
        <span>{language==="tr"?"Renk":"Color"}</span>
        <div class="text-color-row"><i style:background={layer.color}></i><input aria-label={language==="tr"?"Hex renk kodu":"Hex color"} value={layer.color} maxlength="7" onchange={(event)=>setTextColor(event.currentTarget.value)}></div>
        <div class="text-swatches">{#each textColors as color}<button class:active={layer.color===color} style:background={color} aria-label={`Use ${color}`} onclick={()=>setTextColor(color)}></button>{/each}</div>
      </div>
      <label class="field"><span>{language==="tr"?"Yazı boyutu":"Font size"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.size,8,600)}%`} min="8" max="600" step="1" value={layer.size} oninput={(event)=>resizeTextLayer(Number(event.currentTarget.value))}><small class="hint">{Math.round(layer.size)} px</small></label>
      <label class="field"><span>{language==="tr"?"Opaklık":"Opacity"}<small>%</small></span><input type="range" style={`--range-pct:${rangePercent(layer.opacity,0,100)}%`} min="0" max="100" step="1" value={layer.opacity} oninput={(event)=>updateTextLayer({opacity:Number(event.currentTarget.value)})}><small class="hint">{Math.round(layer.opacity)}%</small></label>
      <label class="field"><span>{language==="tr"?"Hizalama":"Alignment"}</span><select value={layer.align} onchange={(event)=>updateTextLayer({align:event.currentTarget.value as "left"|"center"|"right"})}><option value="left">{language==="tr"?"Sol":"Left"}</option><option value="center">{language==="tr"?"Orta":"Center"}</option><option value="right">{language==="tr"?"Sağ":"Right"}</option></select></label>
      <div class="text-position-grid" aria-label={language==="tr"?"Yazı konumu":"Text position"}>{#each ["top-left","top-center","top-right","middle-left","middle-center","middle-right","bottom-left","bottom-center","bottom-right"] as position}<button title={position.replace("-"," ")} onclick={()=>positionText(position)}></button>{/each}</div>
      <details class="text-style-options">
        <summary>{language==="tr"?"Kontur, gölge ve arka plan":"Stroke, shadow & background"}</summary>
        <label class="color-toggle"><input type="checkbox" checked={layer.outline>0} onchange={(event)=>updateTextLayer({outline:event.currentTarget.checked?Math.max(2,layer.outline):0})}><span>{language==="tr"?"Kontur açık":"Stroke enabled"}</span></label>
        {#if layer.outline>0}<label class="field"><span>{language==="tr"?"Kontur genişliği":"Stroke width"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.outline,1,20)}%`} min="1" max="20" step="1" value={layer.outline} oninput={(event)=>updateTextLayer({outline:Number(event.currentTarget.value)})}><small class="hint">{layer.outline}px</small></label>
        <label class="field"><span>{language==="tr"?"Kontur rengi":"Stroke color"}</span><input type="text" value={layer.outline_color} maxlength="7" onchange={(event)=>/^#[0-9a-f]{6}$/i.test(event.currentTarget.value)&&updateTextLayer({outline_color:event.currentTarget.value})}></label>{/if}
        <label class="field"><span>{language==="tr"?"Gölge":"Shadow"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.shadow,0,30)}%`} min="0" max="30" step="1" value={layer.shadow} oninput={(event)=>updateTextLayer({shadow:Number(event.currentTarget.value)})}><small class="hint">{layer.shadow}px</small></label>
        <label class="color-toggle"><input type="checkbox" checked={layer.background} onchange={(event)=>updateTextLayer({background:event.currentTarget.checked})}><span>{language==="tr"?"Arka plan kutusu":"Background box"}</span></label>
        {#if layer.background}
          <label class="field"><span>{language==="tr"?"Arka plan rengi":"Background color"}</span><input type="text" value={layer.background_color} maxlength="7" onchange={(event)=>/^#[0-9a-f]{6}$/i.test(event.currentTarget.value)&&updateTextLayer({background_color:event.currentTarget.value})}></label>
          <label class="field"><span>{language==="tr"?"Arka plan opaklığı":"Background opacity"}<small>%</small></span><input type="range" style={`--range-pct:${rangePercent(layer.background_opacity,0,100)}%`} min="0" max="100" step="1" value={layer.background_opacity} oninput={(event)=>updateTextLayer({background_opacity:Number(event.currentTarget.value)})}><small class="hint">{layer.background_opacity}%</small></label>
          <label class="field"><span>{language==="tr"?"İç boşluk":"Padding"}<small>px</small></span><input type="range" style={`--range-pct:${rangePercent(layer.background_padding,0,80)}%`} min="0" max="80" step="1" value={layer.background_padding} oninput={(event)=>updateTextLayer({background_padding:Number(event.currentTarget.value)})}><small class="hint">{layer.background_padding}px</small></label>
        {/if}
      </details>
      <p class="text-help">{language==="tr"?"Yazıyı sürükle; Shift ile yatay/dikey eksene kilitle. Kenar veya köşe tutamaçlarından boyutlandır.":"Drag text; hold Shift to lock movement to one axis. Resize from side or corner handles."}</p>
    {/if}
  {:else}<p class="text-empty">{language==="tr"?"Önizlemeye ilk katmanı eklemek için Yazı ekle’ye bas.":"Choose Add text to place the first layer in the preview."}</p>{/if}
</div>
