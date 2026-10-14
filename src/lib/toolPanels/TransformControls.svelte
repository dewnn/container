<script lang="ts">
  import type { MediaInfo } from "../editorTypes";
  let { language, media, rotateTransform, setCropPreset, setToolNumber, setToolValue, setTransformRotation, toolNumber, toolValue, transformPresets }:{ language:"tr"|"en"; media:MediaInfo; rotateTransform:(delta:number)=>void; setCropPreset:(mode:string,applyOutputDefault?:boolean)=>void; setToolNumber:(key:string,value:number)=>void; setToolValue:(key:string,value:string)=>void; setTransformRotation:(value:number)=>void; toolNumber:(key:string)=>number; toolValue:(key:string)=>string; transformPresets:string[] }=$props();
</script>

<section>
  <header><b>{language==="tr"?"KIRP":"CROP"}</b><small>{toolValue("crop_mode")==="off" ? (language==="tr"?"kapalı":"off") : `${toolNumber("crop_w").toFixed(1)}% × ${toolNumber("crop_h").toFixed(1)}%`}</small></header>
  <div class="transform-options crop-options">
    {#each transformPresets.filter(preset=>(media?.kind==="image"||!["5:4","3:4"].includes(preset))&&!(media?.kind==="image"&&toolValue("fit_mode")==="contain"&&preset==="free")) as preset}<button class:active={toolValue("crop_mode")===preset} onclick={()=>setCropPreset(preset)}>{preset==="off"?(media?.kind==="image"?(language==="tr"?"ORİJİNAL":"ORIGINAL"):(language==="tr"?"KAPALI":"OFF")):preset==="free"?(language==="tr"?"SERBEST":"FREE"):preset==="191:100"?"1.91:1":preset.toUpperCase()}</button>{/each}
  </div>
  {#if toolValue("crop_mode")!=="off" && toolValue("fit_mode")!=="contain"}<p>{language==="tr"?"Kadrajı önizlemede sürükle; kenar ve köşelerden serbestçe boyutlandır.":"Drag the frame in the preview; resize freely from its edges and corners."}</p>{/if}
  {#if media.kind === "image"}
    <details class="text-style-options image-fit-options"><summary>{language==="tr"?"Sığdır / Tuval":"Fit / Canvas"}</summary>
      <div class="transform-options two"><button class:active={toolValue("fit_mode")==="crop"} onclick={()=>setToolValue("fit_mode","crop")}>{language==="tr"?"Kırp / doldur":"Crop / fill"}</button><button class:active={toolValue("fit_mode")==="contain"} onclick={()=>{setToolValue("fit_mode","contain");if(toolValue("crop_mode")==="free")setCropPreset("off")}}>{language==="tr"?"Sığdır / tuval":"Fit / contain"}</button></div>
      {#if toolValue("fit_mode")==="contain" && toolValue("crop_mode")!=="off"}<label class="field"><span>{language==="tr"?"Tuval arka planı":"Canvas background"}</span><select value={toolValue("canvas_background")} onchange={(event)=>setToolValue("canvas_background",event.currentTarget.value)}><option value="transparent">{language==="tr"?"Şeffaf":"Transparent"}</option><option value="black">{language==="tr"?"Siyah":"Black"}</option><option value="white">{language==="tr"?"Beyaz":"White"}</option><option value="custom">{language==="tr"?"Özel renk":"Custom color"}</option></select></label>{/if}
      {#if toolValue("fit_mode")==="contain" && toolValue("canvas_background")==="custom"}<label class="field"><span>{language==="tr"?"Arka plan rengi":"Background color"}</span><input type="text" maxlength="7" value={toolValue("canvas_color")} oninput={(event)=>setToolValue("canvas_color",event.currentTarget.value)}></label>{/if}
    </details>
  {/if}
</section>
<section>
  <header><b>{language==="tr"?"DÖNDÜR":"ROTATE"}</b><small>{toolValue("rotate")}°</small></header>
  <div class="transform-options four"><button class:active={toolValue("rotate")==="0"} onclick={()=>setTransformRotation(0)}>0°</button><button onclick={()=>rotateTransform(-90)}>↶ 90°</button><button onclick={()=>rotateTransform(90)}>↷ 90°</button><button onclick={()=>rotateTransform(180)}>180°</button></div>
</section>
<section>
  <header><b>{language==="tr"?"ÇEVİR":"FLIP"}</b></header>
  <div class="transform-options two"><button class:active={toolValue("flip_h")==="true"} onclick={()=>setToolValue("flip_h",toolValue("flip_h")==="true"?"false":"true")}>↔ {language==="tr"?"Yatay":"Horizontal"}</button><button class:active={toolValue("flip_v")==="true"} onclick={()=>setToolValue("flip_v",toolValue("flip_v")==="true"?"false":"true")}>↕ {language==="tr"?"Dikey":"Vertical"}</button></div>
</section>
<section>
  <header><b>{language==="tr"?"ÇIKTI BOYUTU":"OUTPUT SIZE"}</b></header>
  <div class="transform-options two"><button class:active={toolValue("size_mode")==="source"} onclick={()=>setToolValue("size_mode","source")}>{language==="tr"?"Kırpılan boyutu koru":"Keep crop size"}</button><button class:active={toolValue("size_mode")==="height"} onclick={()=>setToolValue("size_mode","height")}>{language==="tr"?"Yükseklik":"Height"}</button><button class:active={toolValue("size_mode")==="width"} onclick={()=>setToolValue("size_mode","width")}>{language==="tr"?"Genişlik":"Width"}</button><button class:active={toolValue("size_mode")==="exact"} onclick={()=>setToolValue("size_mode","exact")}>{language==="tr"?"Tam boyut":"Exact"}</button></div>
  {#if ["height","width"].includes(toolValue("size_mode"))}
    <label><span>{toolValue("size_mode")==="height"?(language==="tr"?"Hedef yükseklik":"Target height"):(language==="tr"?"Hedef genişlik":"Target width")}</span><div class="size-entry"><select value={String(toolNumber("size"))} onchange={(event)=>setToolNumber("size",Number(event.currentTarget.value))}>{#each [480,720,1080,1440,2160,4320] as size}<option value={size}>{size}px</option>{/each}</select><input aria-label={language==="tr"?"Özel çıktı boyutu":"Custom output size"} type="number" min="2" max="7680" step="2" value={toolNumber("size")} oninput={(event)=>setToolNumber("size",Number(event.currentTarget.value))}></div></label>
  {:else if toolValue("size_mode")==="exact"}
    <div class="exact-size"><label><span>{language==="tr"?"Genişlik":"Width"}</span><input type="number" min="2" max="7680" step="2" value={toolNumber("output_width")} oninput={(event)=>setToolNumber("output_width",Number(event.currentTarget.value))}></label><b>×</b><label><span>{language==="tr"?"Yükseklik":"Height"}</span><input type="number" min="2" max="7680" step="2" value={toolNumber("output_height")} oninput={(event)=>setToolNumber("output_height",Number(event.currentTarget.value))}></label></div>
    <p>{media.kind==="image"?(language==="tr"?"Görsel esnetilmeden bu tuvale sığdırılır; boş alanlar şeffaf kalır.":"The image is fitted into this canvas without stretching; unused space remains transparent."):(language==="tr"?"Tam boyut, seçtiğin kadrajı bu ölçülere ölçekler; oranlar farklıysa görüntü esneyebilir.":"Exact size scales the crop to these dimensions; mismatched ratios may stretch the image.")}</p>
  {/if}
</section>
{#if media.kind === "image"}
  <section>
    <header><b>{language==="tr"?"ÇIKTI FORMATI":"OUTPUT FORMAT"}</b></header>
    <div class="transform-options three"><button class:active={toolValue("format")==="png"} onclick={()=>setToolValue("format","png")}>{language==="tr"?"PNG · KAYIPSIZ":"PNG · LOSSLESS"}</button><button class:active={toolValue("format")==="webp"} onclick={()=>setToolValue("format","webp")}>WEBP</button><button class:active={toolValue("format")==="jpg"} onclick={()=>setToolValue("format","jpg")}>JPEG</button><button class:active={toolValue("format")==="bmp"} onclick={()=>setToolValue("format","bmp")}>BMP</button><button class:active={toolValue("format")==="tiff"} onclick={()=>setToolValue("format","tiff")}>TIFF</button><button class:active={toolValue("format")==="avif"} onclick={()=>setToolValue("format","avif")}>AVIF</button></div>
    {#if toolValue("format")==="jpg"}<label class="field"><span>{language==="tr"?"Şeffaf alan rengi":"Transparent area color"}</span><input type="text" maxlength="7" value={toolValue("jpeg_background")} oninput={(event)=>setToolValue("jpeg_background",event.currentTarget.value)}></label>{/if}
  </section>
{/if}
