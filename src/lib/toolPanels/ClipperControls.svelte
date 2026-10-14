<script lang="ts">
  import type { FontOption, TextLayer } from "../editorTypes";
  import type { TextAppearance } from "../textRaster";
  import WatermarkControls from "../WatermarkControls.svelte";
  import { safeArea, setSafeAreaPlatform, type SafeAreaPlatform } from "../safeArea.svelte";
  let { language, applyWatermarkPreset, autoDetectCamera, busy, cameraDetecting, cameraDetectionMessage, centerContentRegion, changeWatermark, chooseWatermarkFont, clipperPreviewMode=$bindable(), contentTargetDimensions, freecamLayoutBox=$bindable(), freecamPlacement, prepareWatermark, setSplitHeight, setSplitOrder, setToolNumber, setToolValue, setVerticalLayout, startFreecamPlacement, systemFonts, textPresets, toolNumber, toolValue, watermarkLayer, watermarkLoading }:{ language:"tr"|"en"; applyWatermarkPreset:(id:string)=>Promise<void>; autoDetectCamera:()=>unknown; busy:boolean; cameraDetecting:boolean; cameraDetectionMessage:string; centerContentRegion:()=>void; changeWatermark:(patch:Partial<TextAppearance>)=>void; chooseWatermarkFont:(path:string)=>Promise<void>; clipperPreviewMode:"output"|"source"; contentTargetDimensions:()=>{width:number;height:number}; freecamLayoutBox:HTMLElement|null; freecamPlacement:()=>{width:number;height:number;left:number;top:number}; prepareWatermark:()=>unknown; setSplitHeight:(value:number)=>void; setSplitOrder:(value:string)=>void; setToolNumber:(key:string,value:number)=>void; setToolValue:(key:string,value:string)=>void; setVerticalLayout:(layout:"original"|"blur"|"fill"|"split"|"squares"|"freecam")=>void; startFreecamPlacement:(event:PointerEvent,mode:"move"|"resize")=>void; systemFonts:FontOption[]; textPresets:{id:string;name:string;sourceWidth?:number;layer:TextLayer}[]; toolNumber:(key:string)=>number; toolValue:(key:string)=>string; watermarkLayer:TextAppearance|null; watermarkLoading:boolean }=$props();
</script>

<section>
  <header><b>{language==="tr"?"DİKEY YERLEŞİM":"VERTICAL LAYOUT"}</b><small>1080×1920</small></header>
  <div class="transform-options three">
    <button class:active={toolValue("vertical_layout")==="split"} onclick={()=>setVerticalLayout("split")}>{language==="tr"?"BÖL":"SPLIT"}</button>
    <button class:active={toolValue("vertical_layout")==="squares"} onclick={()=>setVerticalLayout("squares")}>{language==="tr"?"KARELER":"SQUARES"}</button>
    <button class:active={toolValue("vertical_layout")==="freecam"} onclick={()=>setVerticalLayout("freecam")}>FREECAM</button>
    <button class:active={toolValue("vertical_layout")==="original"} onclick={()=>setVerticalLayout("original")}>{language==="tr"?"ORİJİNAL BOYUT":"ORIGINAL SIZE"}</button>
    <button class:active={toolValue("vertical_layout")==="blur"} onclick={()=>setVerticalLayout("blur")}>{language==="tr"?"BULANIK":"BLUR"}</button>
    <button class:active={toolValue("vertical_layout")==="fill"} onclick={()=>setVerticalLayout("fill")}>{language==="tr"?"DOLDUR":"FILL"}</button>
  </div>
  {#if ["split","squares","freecam"].includes(toolValue("vertical_layout"))}
    <button class="auto-camera" class:working={cameraDetecting} onclick={autoDetectCamera} disabled={cameraDetecting||busy}><span>{cameraDetecting?"◌":"◇"}</span>{cameraDetecting?(language==="tr"?"KAMERA ARANIYOR…":"DETECTING CAMERA…"):(language==="tr"?"KAMERAYI OTOMATİK BUL":"AUTO-DETECT CAMERA")}<em title={language==="tr"?"Deneysel özellik":"Experimental feature"}>{language==="tr"?"DENEYSEL":"EXPERIMENTAL"}</em></button>
    {#if cameraDetectionMessage}<p class="auto-camera-result">{cameraDetectionMessage}</p>{/if}
  {/if}
  {#if ["split","squares","freecam"].includes(toolValue("vertical_layout"))}
    {@const contentTarget=contentTargetDimensions()}
    <button class="center-content" onclick={centerContentRegion}>◎ {language==="tr"?"İÇERİĞİ ORTALA":"CENTER CONTENT"} · {contentTarget.width}×{contentTarget.height}</button>
  {/if}
  {#if toolValue("vertical_layout")==="original"}
    <label class="field"><span>{language==="tr"?"Tuval arka planı":"Canvas background"}</span><select value={toolValue("canvas_background")} onchange={(event)=>setToolValue("canvas_background",event.currentTarget.value)}><option value="black">{language==="tr"?"Siyah":"Black"}</option><option value="white">{language==="tr"?"Beyaz":"White"}</option><option value="custom">{language==="tr"?"Özel renk":"Custom color"}</option></select></label>
    {#if toolValue("canvas_background")==="custom"}<label class="field"><span>{language==="tr"?"Arka plan rengi":"Background color"}</span><input type="text" maxlength="7" value={toolValue("canvas_color")} oninput={(event)=>setToolValue("canvas_color",event.currentTarget.value)}></label>{/if}
  {/if}
  {#if toolValue("vertical_layout")==="fill"}<p>{language==="tr"?"Video dikey tuvali tamamen doldurur. Sonuç önizlemesini sürükleyerek yatay kadrajı ayarla.":"The video fills the vertical canvas completely. Drag the result preview to adjust the horizontal framing."}</p>{/if}
  {#if toolValue("vertical_layout")==="blur"}
    <label class="watermark-slider"><span>{language==="tr"?"Arka plan bulanıklığı":"Background blur"}<small>{toolNumber("blur_strength").toFixed(0)} px</small></span><input type="range" min="0" max="60" step="1" value={toolNumber("blur_strength")} oninput={(event)=>setToolNumber("blur_strength",Number(event.currentTarget.value))}></label>
  {/if}
  {#if ["original","blur","fill"].includes(toolValue("vertical_layout"))}
    {#if toolValue("vertical_layout")==="fill"}<label class="watermark-slider"><span>{language==="tr"?"Video yakınlaştırma":"Video zoom"}<small>{toolNumber("clipper_zoom").toFixed(0)}%</small></span><input type="range" min="100" max="300" step="1" value={toolNumber("clipper_zoom")} oninput={(event)=>setToolNumber("clipper_zoom",Number(event.currentTarget.value))}></label>{/if}
    <button class="center-content" onclick={()=>{setToolNumber("clipper_zoom",100);setToolNumber("clipper_x",50);setToolNumber("clipper_y",50)}}>{language==="tr"?"ZOOMU SIFIRLA":"RESET ZOOM"}</button>
    {#if toolValue("vertical_layout")==="blur"}<p>{language==="tr"?"Net videoyu yakınlaştırıp sürükle; bulanık arka plan değişmez.":"Zoom and drag the sharp video; the blurred background stays unchanged."}</p>{/if}
  {/if}
  {#if toolValue("vertical_layout")==="squares"}<p>{language==="tr"?"Kamera ve içerik, dikey tuvali eşit iki tam genişlikte bölüme ayırır.":"Camera and content divide the vertical canvas into two equal full-width sections."}</p>{/if}
  {#if toolValue("vertical_layout")==="freecam"}
    {@const freecam=freecamPlacement()}
    <div class="freecam-layout" bind:this={freecamLayoutBox} aria-label={language==="tr"?"Freecam çıktı yerleşimi":"Freecam output layout"}>
      <span>{language==="tr"?"İÇERİK":"CONTENT"}</span>
      <button class="freecam-camera" style:left={`${freecam.left}%`} style:top={`${freecam.top}%`} style:width={`${freecam.width}%`} style:height={`${freecam.height}%`} onpointerdown={(event)=>startFreecamPlacement(event,"move")}>
        {language==="tr"?"KAMERA":"CAMERA"}<i role="presentation" onpointerdown={(event)=>startFreecamPlacement(event,"resize")}></i>
      </button>
    </div>
    <p>{language==="tr"?"Kamerayı bu küçük yerleşimde sürükle; sağ alt köşeden boyutlandır. Kaynak kırpımını KAYNAK BÖLGELERİ görünümündeki Camera Region ile ayarla.":"Drag the camera in this layout; resize from the lower-right corner. Adjust its crop using Camera Region in SOURCE REGIONS."}</p>
     <label><span class="slider-title">{language==="tr"?"Kamera konumu X":"Camera position X"}<button type="button" class="slider-reset" aria-label={language==="tr"?"Kamera X konumunu sıfırla":"Reset camera X"} onclick={(event)=>{event.preventDefault();setToolNumber("freecam_x",50)}}>↺</button></span><input aria-label={language==="tr"?"Kamera konumu X":"Camera position X"} type="range" min="0" max="100" step="1" value={toolNumber("freecam_x")} oninput={(event)=>setToolNumber("freecam_x",Number(event.currentTarget.value))}></label>
     <label><span class="slider-title">{language==="tr"?"Kamera konumu Y":"Camera position Y"}<button type="button" class="slider-reset" aria-label={language==="tr"?"Kamera Y konumunu sıfırla":"Reset camera Y"} onclick={(event)=>{event.preventDefault();setToolNumber("freecam_y",2)}}>↺</button></span><input aria-label={language==="tr"?"Kamera konumu Y":"Camera position Y"} type="range" min="0" max="100" step="1" value={toolNumber("freecam_y")} oninput={(event)=>setToolNumber("freecam_y",Number(event.currentTarget.value))}></label>
     <label><span class="slider-title">{language==="tr"?"Kamera boyutu":"Camera size"}<button type="button" class="slider-reset" aria-label={language==="tr"?"Kamera boyutunu sıfırla":"Reset camera size"} onclick={(event)=>{event.preventDefault();setToolNumber("freecam_size",77)}}>↺</button></span><input aria-label={language==="tr"?"Kamera boyutu":"Camera size"} type="range" min="15" max="90" step="1" value={toolNumber("freecam_size")} oninput={(event)=>setToolNumber("freecam_size",Number(event.currentTarget.value))}><small>{toolNumber("freecam_size").toFixed(0)}%</small></label>
     <button class="center-content" onclick={()=>{setToolNumber("freecam_x",50);setToolNumber("freecam_y",2);setToolNumber("freecam_size",77)}}>{language==="tr"?"KAMERAYI SIFIRLA":"RESET CAMERA"}</button>
  {/if}
  <div class="safe-area-controls">
    <label class="watermark-text-field"><span>{language==="tr"?"Güvenli alan":"Safe area"}</span><select value={safeArea.platform} onchange={(event)=>{setSafeAreaPlatform(event.currentTarget.value as SafeAreaPlatform);if(event.currentTarget.value!=="off")clipperPreviewMode="output"}}><option value="off">{language==="tr"?"Kapalı":"Off"}</option><option value="tiktok">TikTok</option><option value="reels">Instagram Reels</option><option value="shorts">YouTube Shorts</option></select></label>
    {#if safeArea.platform!=="off"}<small>{language==="tr"?"Taralı bölgeleri platformun arayüzü kapatır. Yazı, filigran ve etiketleri çerçevenin içinde tut. Değerler yaklaşıktır; yalnızca önizlemede görünür.":"The platform interface covers the shaded areas. Keep text, watermark and tags inside the frame. Values are approximate and only shown in the preview."}</small>{/if}
  </div>
  <div class="clipper-watermark-controls">
    <label class="watermark-toggle"><input type="checkbox" checked={toolValue("watermark_enabled")==="true"} onchange={(event)=>{setToolValue("watermark_enabled",event.currentTarget.checked?"true":"false");if(event.currentTarget.checked)clipperPreviewMode="output"}}><span>{language==="tr"?"Filigran":"Watermark"}</span></label>
    {#if toolValue("watermark_enabled")==="true"}
      {#if watermarkLayer}<WatermarkControls layer={watermarkLayer} fonts={systemFonts} presets={textPresets} width={toolNumber("output_width")||1080} {language} onchange={changeWatermark} onfont={chooseWatermarkFont} onpreset={applyWatermarkPreset}/>{:else}<small>{language==="tr"?"Font hazırlanıyor…":"Preparing font…"}</small><button class="ghost" disabled={watermarkLoading} onclick={prepareWatermark}>{language==="tr"?"Tekrar dene":"Retry"}</button>{/if}
    {/if}
  </div>
  <div class="clipper-watermark-controls">
    <label class="watermark-toggle"><input type="checkbox" checked={toolValue("social_tag_enabled")==="true"} onchange={(event)=>setToolValue("social_tag_enabled",event.currentTarget.checked?"true":"false")}><span>{language==="tr"?"Sosyal etiket":"Social Tag"}</span></label>
    {#if toolValue("social_tag_enabled")==="true"}
      <label class="watermark-text-field"><span>{language==="tr"?"Platform":"Platform"}</span><select value={toolValue("social_tag_platform")} onchange={(event)=>setToolValue("social_tag_platform",event.currentTarget.value)}><option value="kick">Kick</option><option value="twitch" disabled={toolValue("social_tag_style")==="kick_banner"}>Twitch</option></select></label>
      <label class="watermark-text-field"><span>{language==="tr"?"Kullanıcı adı":"Username"}</span><input type="text" maxlength="32" placeholder="kanaladi" value={toolValue("social_tag_username")} oninput={(event)=>setToolValue("social_tag_username",event.currentTarget.value)}></label>
      <label class="watermark-text-field"><span>{language==="tr"?"Görünüm":"Style"}</span><select value={toolValue("social_tag_style")} onchange={(event)=>{const value=event.currentTarget.value;setToolValue("social_tag_style",value);if(value==="kick_banner"){setToolValue("social_tag_platform","kick");setToolNumber("social_tag_size",54)}}}><option value="boxed">{language==="tr"?"Kutulu etiket":"Boxed badge"}</option><option value="plain">{language==="tr"?"Düz etiket":"Plain tag"}</option><option value="kick_banner">{language==="tr"?"Kick.com şeridi":"Kick.com banner"}</option></select></label>
      {#if toolValue("social_tag_style")==="plain"}
        <label class="watermark-text-field"><span>{language==="tr"?"Konum":"Position"}</span><select value={toolValue("social_tag_plain_position")} onchange={(event)=>setToolValue("social_tag_plain_position",event.currentTarget.value)}><option value="left">{language==="tr"?"Sol":"Left"}</option><option value="center">{language==="tr"?"Orta":"Center"}</option><option value="right">{language==="tr"?"Sağ":"Right"}</option></select></label>
      {/if}
       <label class="watermark-slider"><span>{language==="tr"?"Boyut":"Size"}<small>{toolNumber("social_tag_size").toFixed(0)} px</small></span><input type="range" min="20" max={toolValue("social_tag_style")==="kick_banner"?54:96} step="1" value={toolNumber("social_tag_size")} oninput={(event)=>setToolNumber("social_tag_size",Number(event.currentTarget.value))}></label>
       <label class="watermark-slider"><span class="slider-title"><span>{toolValue("social_tag_style")==="kick_banner"?(language==="tr"?"Şeridi kameradan uzaklaştır":"Move banner away from camera"):(language==="tr"?"Etiketi kameradan uzaklaştır":"Move tag away from camera")}</span><span class="slider-end"><small>{toolNumber("social_tag_seam_offset").toFixed(0)}%</small><button type="button" class="slider-reset" aria-label={language==="tr"?"Etiket mesafesini sıfırla":"Reset tag distance"} title={language==="tr"?"Varsayılan: 0%":"Default: 0%"} onclick={()=>setToolNumber("social_tag_seam_offset",0)}>↺</button></span></span><input aria-label={toolValue("social_tag_style")==="kick_banner"?(language==="tr"?"Şeridi kameradan uzaklaştır":"Move banner away from camera"):(language==="tr"?"Etiketi kameradan uzaklaştır":"Move tag away from camera")} type="range" min="0" max="100" step="1" value={toolNumber("social_tag_seam_offset")} oninput={(event)=>setToolNumber("social_tag_seam_offset",Number(event.currentTarget.value))}></label>
    {/if}
  </div>
  {#if toolValue("vertical_layout")==="split"}
    <details class="text-style-options clipper-advanced">
      <summary>{language==="tr"?"GELİŞMİŞ":"ADVANCED"}</summary>
      <label><span>{language==="tr"?"Sıralama":"Order"}</span><select value={toolValue("region_order")} onchange={(event)=>setSplitOrder(event.currentTarget.value)}><option value="a_first">{language==="tr"?"Kamera üstte":"Camera above content"}</option><option value="b_first">{language==="tr"?"İçerik üstte":"Content above camera"}</option></select></label>
      <label><span>{language==="tr"?"Üst bölüm yüksekliği":"Top region height"}</span><input type="range" min="20" max="80" step="1" value={toolNumber("region_a_height")} oninput={(event)=>setSplitHeight(Number(event.currentTarget.value))}><small>{toolNumber("region_a_height").toFixed(0)}%</small></label>
      <p>{language==="tr"?"Kamera ve içerik kutularını kaynak önizleme üzerinde sürükleyip kenarlarından boyutlandır.":"Drag Camera and Content on the source preview and resize them from their edges."}</p>
    </details>
  {/if}
</section>
