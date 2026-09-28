export interface FrameRect { x:number; y:number; width:number; height:number }
export interface ClipperLayoutInput {
  width:number; height:number; sourceWidth:number; sourceHeight:number;
  layout:string; regionAHeight:number; regionOrder:string;
  camera:FrameRect; content:FrameRect;
  freecamSize:number; freecamX:number; freecamY:number;
}

const evenRound=(value:number)=>Math.max(2,Math.floor(Math.round(value)/2)*2);

// FFmpeg crops subsampled input to even source pixels before scaling it.
export function sourceRegion(region:FrameRect,width:number,height:number):FrameRect {
  const cropWidth=Math.min(width,Math.max(2,Math.floor(width*region.width/200)*2));
  const cropHeight=Math.min(height,Math.max(2,Math.floor(height*region.height/200)*2));
  return {
    x:Math.max(0,Math.min(width-cropWidth,Math.floor(width*region.x/200)*2)),
    y:Math.max(0,Math.min(height-cropHeight,Math.floor(height*region.y/200)*2)),
    width:cropWidth,height:cropHeight,
  };
}

export function clipperLayoutGeometry(input:ClipperLayoutInput) {
  const {width,height}=input;
  const cameraSource=sourceRegion(input.camera,input.sourceWidth,input.sourceHeight);
  const contentSource=sourceRegion(input.content,input.sourceWidth,input.sourceHeight);
  let camera:FrameRect,content:FrameRect;
  if(input.layout==="freecam"){
    const cameraWidth=evenRound(width*input.freecamSize/100);
    const cameraHeight=evenRound(cameraWidth*cameraSource.height/cameraSource.width);
    camera={x:Math.max(0,width-cameraWidth)*input.freecamX/100,y:Math.max(0,height-cameraHeight)*input.freecamY/100,width:cameraWidth,height:cameraHeight};
    content={x:0,y:0,width,height};
  }else{
    const seam=input.layout==="squares"?Math.floor(height/4)*2:Math.min(height-2,evenRound(height*input.regionAHeight/100));
    const top={x:0,y:0,width,height:seam},bottom={x:0,y:seam,width,height:height-seam};
    const reversed=input.layout==="split"&&input.regionOrder==="b_first";
    camera=reversed?bottom:top;content=reversed?top:bottom;
  }
  return {camera,content,cameraSource,contentSource,radius:input.layout==="freecam"?Math.max(2,Math.round(camera.width*.025)):0};
}

export function drawClipperLayout(context:CanvasRenderingContext2D,source:CanvasImageSource,input:ClipperLayoutInput) {
  const geometry=clipperLayoutGeometry(input);
  const draw=(crop:FrameRect,target:FrameRect,radius=0)=>{
    const scale=Math.max(target.width/crop.width,target.height/crop.height);
    const visibleWidth=target.width/scale,visibleHeight=target.height/scale;
    context.save();
    context.beginPath();context.roundRect(target.x,target.y,target.width,target.height,radius);context.clip();
    context.drawImage(source,crop.x+(crop.width-visibleWidth)/2,crop.y+(crop.height-visibleHeight)/2,visibleWidth,visibleHeight,target.x,target.y,target.width,target.height);
    context.restore();
  };
  context.fillStyle="#000";context.fillRect(0,0,input.width,input.height);
  draw(geometry.contentSource,geometry.content);
  draw(geometry.cameraSource,geometry.camera,geometry.radius);
}
