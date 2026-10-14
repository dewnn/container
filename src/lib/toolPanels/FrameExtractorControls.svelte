<script lang="ts">
  let { language, frameAdvanced=$bindable(), playerTime, setToolValue, toolNumber, toolValue, toolboxCurrent }:{ language:"tr"|"en"; frameAdvanced:boolean; playerTime:(value:number)=>string; setToolValue:(key:string,value:string)=>void; toolNumber:(key:string)=>number; toolValue:(key:string)=>string; toolboxCurrent:number }=$props();
</script>

<div class="frame-extractor-simple">
  {#if toolValue("mode")==="burst"}
    <p>{language==="tr"?"Çıkarma başlayacak:":"Extraction starts at:"} <b class="mono">{playerTime(toolboxCurrent)}</b><br/><small>{language==="tr"?"Başlangıcı oynatıcıdan değiştir.":"Move the playhead to choose the start."}</small></p>
    <label class="field"><span>{language==="tr"?"Çıkarılacak kare sayısı":"Number of frames to extract"}</span><input type="number" min="1" max="1000" step="1" value={toolValue("count")} oninput={event=>setToolValue("count",event.currentTarget.value)}/></label>
    <label class="field"><span>{language==="tr"?"Kareler arası süre":"Distance between frames"}<small>ms</small></span><input type="number" min="1" max="3600000" step="1" value={Math.round(toolNumber("interval")*1000)} oninput={event=>setToolValue("interval",String(Number(event.currentTarget.value)/1000))}/></label>
    <p>{language==="tr"?`${toolValue("count")} kare, ${Math.round(toolNumber("interval")*1000)} ms aralıkla. Örnek: 10 kare × 100 ms, ilk kareden son kareye yaklaşık 0,9 saniyeyi kapsar.`:`${toolValue("count")} frames, ${Math.round(toolNumber("interval")*1000)} ms apart. Example: 10 frames × 100 ms covers about 0.9 seconds from first to last.`}</p>
    <small>{language==="tr"?"PNG, videonun kendi çözünürlüğünde. Video biterse daha az kare çıkar. Aralıklar mevcut video karelerine yuvarlanır; aynı kare tekrarlanmaz.":"PNG at the video's original resolution. Fewer frames are extracted if the video ends. Intervals align to available video frames; frames are not duplicated."}</small>
  {/if}
  <button class="ghost" onclick={()=>{frameAdvanced=!frameAdvanced;if(!frameAdvanced)setToolValue("mode","burst")}}>{frameAdvanced?(language==="tr"?"Basit moda dön":"Back to simple mode"):(language==="tr"?"Gelişmiş seçenekler":"Advanced options")}</button>
</div>
