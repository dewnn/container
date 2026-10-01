<script lang="ts">
  let {video,box,strength}:{video:HTMLVideoElement|null;box:{left:number;top:number;width:number;height:number};strength:number}=$props();
  let canvas:HTMLCanvasElement|undefined=$state();
  $effect(()=>{
    const source=video,target=canvas,bounds=box;
    if(!source||!target)return;
    // Blur needs no second decoder or full-resolution backing surface.
    target.width=Math.max(1,Math.min(480,Math.round(bounds.width)));
    target.height=Math.max(1,Math.round(target.width*bounds.height/bounds.width));
    const ctx=target.getContext("2d");if(!ctx)return;
    let frame=0,animation=0,disposed=false;
    const draw=()=>{
      if(source.readyState<2||!source.videoWidth||!source.videoHeight)return;
      const scale=Math.max(target.width/source.videoWidth,target.height/source.videoHeight);
      const width=target.width/scale,height=target.height/scale;
      ctx.drawImage(source,(source.videoWidth-width)/2,(source.videoHeight-height)/2,width,height,0,0,target.width,target.height);
    };
    const stop=()=>{if(frame)source.cancelVideoFrameCallback?.(frame);cancelAnimationFrame(animation);frame=animation=0};
    const schedule=()=>{
      if(disposed||source.paused||source.ended)return;
      if(source.requestVideoFrameCallback)frame=source.requestVideoFrameCallback(()=>{frame=0;draw();schedule()});
      else animation=requestAnimationFrame(()=>{animation=0;draw();schedule()});
    };
    const update=()=>{stop();draw();schedule()};
    const events=["loadeddata","seeked","play","pause","ended","resize"];
    for(const event of events)source.addEventListener(event,update);
    update();
    return()=>{disposed=true;stop();for(const event of events)source.removeEventListener(event,update)};
  });
</script>
<canvas bind:this={canvas} class="blur-backdrop-canvas" aria-hidden="true" style={`position:absolute;z-index:1;pointer-events:none;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;filter:blur(${strength*box.width/1080}px);clip-path:inset(0)`}></canvas>
