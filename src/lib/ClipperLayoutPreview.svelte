<script lang="ts">
  import { drawClipperLayout, type ClipperLayoutInput } from "./clipperLayoutGeometry";

  let { video,input,box }:{video:HTMLVideoElement|null;input:ClipperLayoutInput;box:{left:number;top:number;width:number;height:number}}=$props();
  let canvas:HTMLCanvasElement|undefined=$state();

  $effect(()=>{
    const target=canvas,source=video,settings=input,bounds=box;
    if(!target||!source)return;
    const context=target.getContext("2d");if(!context)return;
    const ratio=Math.min(2,window.devicePixelRatio||1);
    target.width=Math.max(1,Math.round(bounds.width*ratio));
    target.height=Math.max(1,Math.round(bounds.height*ratio));
    let animation=0;
    const draw=()=>{
      context.setTransform(target.width/settings.width,0,0,target.height/settings.height,0,0);
      if(source.readyState>=2)drawClipperLayout(context,source,settings);
    };
    const playing=()=>{
      cancelAnimationFrame(animation);draw();
      if(!source.paused&&!source.ended)animation=requestAnimationFrame(playing);
    };
    const events=["loadeddata","seeked","timeupdate","play","pause","ended"];
    for(const event of events)source.addEventListener(event,playing);
    playing();
    return ()=>{cancelAnimationFrame(animation);for(const event of events)source.removeEventListener(event,playing)};
  });
</script>

<canvas bind:this={canvas} class="clipper-output-canvas" aria-label="Clipper output preview" style={`position:absolute;z-index:2;left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px;background:#000;pointer-events:none`}></canvas>
