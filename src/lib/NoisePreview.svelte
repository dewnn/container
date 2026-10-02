<script lang="ts">
  let {video,amount,language}:{video:HTMLVideoElement|null;amount:number;language:string}=$props();
  let canvas:HTMLCanvasElement|undefined=$state();
  let split=$state(50),unavailable=$state(false);
  $effect(()=>{
    const source=video,target=canvas,strength=amount,divider=split;
    if(!source||!target)return;
    const ctx=target.getContext("2d",{willReadFrequently:true});if(!ctx)return;
    let frame=0,animation=0,disposed=false;
    const draw=()=>{
      if(source.readyState<2||!source.videoWidth)return;
      target.width=Math.min(640,source.videoWidth);target.height=Math.max(1,Math.round(target.width*source.videoHeight/source.videoWidth));
      try{
        ctx.drawImage(source,0,0,target.width,target.height);
        const image=ctx.getImageData(0,0,target.width,target.height),pixels=image.data,start=Math.round(target.width*divider/100);
        for(let y=0;y<target.height;y++)for(let x=start;x<target.width;x++){
          const offset=(y*target.width+x)*4;
          for(let c=0;c<3;c++)pixels[offset+c]+=Math.round((Math.random()*2-1)*strength);
        }
        ctx.putImageData(image,0,0);unavailable=false;
      }catch{unavailable=true}
    };
    const stop=()=>{if(frame)source.cancelVideoFrameCallback?.(frame);cancelAnimationFrame(animation);frame=animation=0};
    const schedule=()=>{
      if(disposed||source.paused||source.ended||unavailable)return;
      if(source.requestVideoFrameCallback)frame=source.requestVideoFrameCallback(()=>{frame=0;draw();schedule()});
      else animation=requestAnimationFrame(()=>{animation=0;draw();schedule()});
    };
    const update=()=>{stop();draw();schedule()};
    const events=["loadeddata","seeked","play","pause","ended"];
    for(const event of events)source.addEventListener(event,update);update();
    return()=>{disposed=true;stop();for(const event of events)source.removeEventListener(event,update)};
  });
</script>
<div class="noise-preview">
  <canvas bind:this={canvas} style:visibility={unavailable?"hidden":"visible"} aria-label={language==="tr"?"Canlı gürültü karşılaştırması":"Live noise comparison"}></canvas>
  <div class="noise-preview-controls"><span>{language==="tr"?"Orijinal":"Original"}</span><input type="range" min="0" max="100" bind:value={split} aria-label={language==="tr"?"Gürültü karşılaştırma ayırıcı":"Noise comparison divider"}/><span>{language==="tr"?"Gürültü":"Noise"} · {amount}</span><small>{unavailable?(language==="tr"?"Önizleme kullanılamıyor; render ayarları korunuyor.":"Preview unavailable; render settings are unchanged."):(language==="tr"?"Yaklaşık canlı önizleme · çıktı FFmpeg ile işlenir":"Approximate live preview · export is processed by FFmpeg")}</small></div>
</div>
