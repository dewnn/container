<script lang="ts">
  import { applyAccent, readAccent, DEFAULT_ACCENT } from "./accentTheme";
  import PremiereSettings from "./PremiereSettings.svelte";
  import {premiere} from "./premiereBridge.svelte";
  let {language,theme,onlanguage,ontheme,onresetlayout,layoutBusy=false}:{language:"tr"|"en";theme:"dark"|"light";onlanguage:(value:"tr"|"en")=>void;ontheme:(value:"dark"|"light")=>void;onresetlayout?:()=>void;layoutBusy?:boolean}=$props();
  let dialog:HTMLDialogElement;
  let alerts=$state(false);
  let accent=$state(readAccent());
  function show(){accent=readAccent();try{alerts=localStorage.getItem("container-completion-alert")==="true"}catch{}dialog.showModal()}
  function update(hue:number,saturation=accent.saturation){accent={hue:((hue%360)+360)%360,saturation};applyAccent(accent.hue,accent.saturation)}
  function pick(event:PointerEvent){
    const wheel=event.currentTarget as HTMLElement;
    if(event.type==="pointermove"&&!wheel.hasPointerCapture(event.pointerId))return;
    if(event.type==="pointerdown"){if(event.button!==0)return;wheel.setPointerCapture(event.pointerId);wheel.focus()}
    const rect=wheel.getBoundingClientRect();
    update(Math.atan2(event.clientY-rect.top-rect.height/2,event.clientX-rect.left-rect.width/2)*180/Math.PI+90);
  }
  function wheelKey(event:KeyboardEvent){
    const delta=event.shiftKey?10:1;
    if(["ArrowRight","ArrowUp","ArrowLeft","ArrowDown","Home","End"].includes(event.key)){
      event.preventDefault();update(event.key==="Home"?0:event.key==="End"?359:accent.hue+(["ArrowRight","ArrowUp"].includes(event.key)?delta:-delta));
    }
  }
  function saveAlert(){try{localStorage.setItem("container-completion-alert",String(alerts));window.dispatchEvent(new Event("container-alert-preference"))}catch{alerts=false}}
</script>
<button class="general-settings-trigger ghost" class:premiere-ready={premiere.status.connected} onclick={show} aria-label={language==="tr"?"Genel ayarlar":"General settings"} title={premiere.status.connected?(language==="tr"?"Genel ayarlar · Premiere bağlı":"General settings · Premiere connected"):(language==="tr"?"Genel ayarlar":"General settings")}><svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M3 5h14M3 10h14M3 15h14"/><circle cx="7" cy="5" r="2" fill="var(--panel)"/><circle cx="13" cy="10" r="2" fill="var(--panel)"/><circle cx="8" cy="15" r="2" fill="var(--panel)"/></svg></button>
<dialog bind:this={dialog} class="general-settings-dialog panel" aria-labelledby="general-settings-title">
  <header><h2 id="general-settings-title">{language==="tr"?"Genel ayarlar":"General settings"}</h2><button class="ghost" onclick={()=>dialog.close()} aria-label={language==="tr"?"Kapat":"Close"}>×</button></header>
  <label><span>{language==="tr"?"Dil":"Language"}</span><select value={language} onchange={event=>onlanguage(event.currentTarget.value as "tr"|"en")}><option value="tr">Türkçe</option><option value="en">English</option></select></label>
  <label><span>{language==="tr"?"Tema":"Theme"}</span><select value={theme} onchange={event=>ontheme(event.currentTarget.value as "dark"|"light")}><option value="dark">{language==="tr"?"Koyu":"Dark"}</option><option value="light">{language==="tr"?"Açık":"Light"}</option></select></label>
  <section class="accent-picker" aria-labelledby="accent-title">
    <div class="accent-heading"><h3 id="accent-title">{language==="tr"?"Vurgu rengi":"Accent color"}</h3><button class="ghost" onclick={()=>update(DEFAULT_ACCENT.hue,DEFAULT_ACCENT.saturation)}>{language==="tr"?"Sıfırla":"Reset"}</button></div>
    <div class="accent-picker-body">
      <div class="accent-wheel" role="slider" tabindex="0" aria-label={language==="tr"?"Renk çarkı":"Color wheel"} aria-valuemin={0} aria-valuemax={359} aria-valuenow={Math.round(accent.hue)%360} onpointerdown={pick} onpointermove={pick} onkeydown={wheelKey}>
        <div class="accent-wheel-core"><span></span></div>
        <i style:transform={`rotate(${accent.hue}deg)`}><b></b></i>
      </div>
      <div class="accent-options">
        <div class="accent-presets">{#each [213,270,330,15,45,145,180] as hue}<button class:active={Math.abs(accent.hue-hue)<1} style:background={`hsl(${hue} 85% 60%)`} aria-label={`${language==="tr"?"Renk":"Color"} ${hue}°`} aria-pressed={Math.abs(accent.hue-hue)<1} onclick={()=>update(hue)}></button>{/each}</div>
        <label class="accent-saturation"><span>{language==="tr"?"Canlılık":"Saturation"}<b>{Math.round(accent.saturation)}%</b></span><input type="range" min="55" max="100" value={accent.saturation} oninput={event=>update(accent.hue,Number(event.currentTarget.value))}></label>
        <small>{language==="tr"?"Tüm uygulamada anında uygulanır.":"Applies instantly across the app."}</small>
      </div>
    </div>
  </section>
  {#if onresetlayout}<section class="workspace-layout"><h3>{language==="tr"?"Çalışma alanı düzeni":"Workspace layout"}</h3><p>{language==="tr"?"Yalnızca açık çalışma alanının panel boyutlarını sıfırlar.":"Reset panel sizes for the current workspace only."}</p><button class="ghost" disabled={layoutBusy} onclick={()=>{dialog.close();onresetlayout?.()}}>{language==="tr"?"Panel düzenini sıfırla":"Reset panel layout"}</button></section>{/if}
  <label class="general-alert"><input type="checkbox" bind:checked={alerts} onchange={saveAlert}><span>{language==="tr"?"İşlem bitince bildir":"Notify when finished"}<small>{language==="tr"?"Uygulama içi bildirim ve görev çubuğu uyarısı":"In-app message and taskbar alert"}</small></span></label>
  <PremiereSettings {language}/>
  <footer><button class="ghost" onclick={()=>dialog.close()}>{language==="tr"?"Tamam":"Done"}</button></footer>
</dialog>
