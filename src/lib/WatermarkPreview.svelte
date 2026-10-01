<script lang="ts">
  import {rasterText,type TextAppearance} from "./textRaster";
  let {layer,width,height,box,onchange}:{layer:TextAppearance;width:number;height:number;box:{left:number;top:number;width:number;height:number}|null;onchange:(patch:Partial<TextAppearance>)=>void}=$props();
  let canvas:HTMLCanvasElement|undefined=$state(),bounds=$state({left:0,top:0,width:0,height:0});
  $effect(()=>{if(canvas)bounds=rasterText(canvas,layer,width,height)});
  export function raster(){if(!canvas)throw new Error("Watermark preview is not ready.");rasterText(canvas,layer,width,height);return canvas.toDataURL("image/png")}
  function drag(event:PointerEvent,resize=false){
    if(event.button!==0||!box)return;event.preventDefault();event.stopPropagation();const target=event.currentTarget as HTMLElement;target.setPointerCapture(event.pointerId);
    const origin={...layer},startX=event.clientX,startY=event.clientY,initialWidth=Math.max(1,bounds.width*box.width/width),display={...box},wrapWidth=origin.wrap_width??Math.max(1,bounds.width/origin.size);
    let axis:"x"|"y"|null=null;
    const move=(e:PointerEvent)=>{
      if(e.pointerId!==event.pointerId)return;
      let dx=e.clientX-startX,dy=e.clientY-startY;
      if(resize){onchange({size:Math.max(8,Math.min(600,origin.size*(1+dx/initialWidth))),wrap_width:wrapWidth});return}
      if(e.shiftKey){
        if(!axis&&Math.max(Math.abs(dx),Math.abs(dy))>=3)axis=Math.abs(dx)>Math.abs(dy)?"x":"y";
        if(axis!=="x")dx=0;
        if(axis!=="y")dy=0;
      }else axis=null;
      onchange({x:Math.max(0,Math.min(100,origin.x+dx/display.width*100)),y:Math.max(0,Math.min(100,origin.y+dy/display.height*100))});
    };
    const stop=()=>{target.removeEventListener("pointermove",move);target.removeEventListener("pointerup",stop);target.removeEventListener("pointercancel",stop);target.removeEventListener("lostpointercapture",stop)};
    target.addEventListener("pointermove",move);target.addEventListener("pointerup",stop);target.addEventListener("pointercancel",stop);target.addEventListener("lostpointercapture",stop);
  }
</script>
<div class="watermark-canvas-layer" style={box?`left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px`:"display:none"}>
  <canvas bind:this={canvas}></canvas>
  {#if layer.text.trim()}<div role="group" aria-label="Move watermark" class="watermark-drag" style={`left:${bounds.left/width*100}%;top:${bounds.top/height*100}%;width:${bounds.width/width*100}%;height:${bounds.height/height*100}%`} onpointerdown={event=>drag(event)}><button aria-label="Resize watermark" onpointerdown={event=>drag(event,true)}></button></div>{/if}
</div>
<style>.watermark-canvas-layer{position:absolute;z-index:6;pointer-events:none;font-feature-settings:normal;font-variant:normal;letter-spacing:normal;text-transform:none}.watermark-canvas-layer canvas{width:100%;height:100%;display:block}.watermark-drag{position:absolute;pointer-events:auto;cursor:move;touch-action:none;outline:1px dashed var(--blue)}.watermark-drag button{position:absolute;right:-5px;bottom:-5px;width:10px;height:10px;padding:0;border:2px solid white;background:var(--blue);cursor:nwse-resize;touch-action:none}</style>
