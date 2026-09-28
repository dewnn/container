import { sourceRegion } from "./clipperLayoutGeometry.ts";

export interface SocialTagGeometryInput {
  width:number; height:number; sourceWidth:number; sourceHeight:number;
  layout:string; style:"boxed"|"plain"; position:"left"|"center"|"right"; username:string; size:number;
  textUnits?:number;
  regionAHeight:number; regionOrder:string;
  regionAWidth:number; regionARegionHeight:number;
  freecamSize:number; freecamX:number; freecamY:number;
  seamOffset?:number;
}

export interface SocialTagGeometry { x:number; y:number; side:number; fontSize:number; totalWidth:number; anchorX:number; centerY:number }

export function socialTagGeometry(input:SocialTagGeometryInput):SocialTagGeometry {
  const {width,height,layout,style,username}=input;
  const position=style==="boxed"?"center":input.position;
  const estimatedUnits=Array.from(username).reduce((sum,letter)=>sum+(/[ilI1.,:!|]/.test(letter)?.35:/[mwMW@]/.test(letter)?.9:/[A-Z]/.test(letter)?.72:.59),0);
  const measured=Number.isFinite(input.textUnits)&&input.textUnits!==undefined&&input.textUnits>0;
  const units=measured?input.textUnits!:estimatedUnits;
  const unitScale=measured?1:style==="boxed"?1.1:1;
  let cameraLeft=0,cameraTop=0,cameraWidth=width,cameraBottom=height,seam=height*.78;
  const even=(value:number)=>Math.floor(Math.round(value)/2)*2;
  if(layout==="split"){
    seam=even(height*input.regionAHeight/100);
    if(input.regionOrder==="b_first"){cameraTop=seam;cameraBottom=height}else cameraBottom=seam;
  }else if(layout==="squares"){
    seam=Math.floor(height/4)*2;cameraBottom=seam;
  }else if(layout==="freecam"){
    cameraWidth=even(width*input.freecamSize/100);
    const source=sourceRegion({x:0,y:0,width:input.regionAWidth,height:input.regionARegionHeight},input.sourceWidth,input.sourceHeight);
    const sourceWidth=source.width,sourceHeight=source.height;
    const cameraHeight=Math.max(2,even(cameraWidth*sourceHeight/Math.max(1,sourceWidth)));
    cameraLeft=Math.max(0,width-cameraWidth)*input.freecamX/100;
    cameraTop=Math.max(0,height-cameraHeight)*input.freecamY/100;
    cameraBottom=cameraTop+cameraHeight;seam=cameraBottom;
  }else if(layout==="blur"||layout==="original"){
    const bandHeight=Math.min(height,width*input.sourceHeight/Math.max(1,input.sourceWidth));
    cameraBottom=(height+bandHeight)/2;
    seam=cameraBottom;
  }else if(layout==="fill"){
    cameraBottom=height*.55;seam=cameraBottom;
  }
  const cameraRight=cameraLeft+cameraWidth;
  // Only the boxed badge needs an inset against phone-side cropping.
  const inset=style==="boxed"?width*.05:0;
  const safeLeft=style==="boxed"?Math.max(inset,cameraLeft):inset,safeRight=style==="boxed"?Math.min(width-inset,cameraRight):width-inset;
  const targetX=position==="left"?Math.max(cameraLeft,safeLeft):position==="center"?cameraLeft+cameraWidth/2:Math.min(cameraRight,safeRight);
  const available=style==="boxed"?Math.max(24,position==="left"?safeRight-targetX:position==="right"?targetX-safeLeft:2*Math.min(targetX-safeLeft,safeRight-targetX)):width*.84;
  const fontSize=Math.min(input.size,available/(units*unitScale+2.4));
  const side=Math.max(8,Math.round(fontSize*1.5));
  const gap=style==="plain"?fontSize*.22:0;
  const padding=Math.max(2,Math.round(fontSize*.34));
  const textWidth=units*fontSize*unitScale;
  const totalWidth=side+gap+textWidth+(style==="boxed"?padding*2:0);
  const rawX=position==="left"?targetX:position==="center"?targetX-totalWidth/2:targetX-totalWidth;
  const x=Math.round(Math.max(safeLeft,Math.min(rawX,Math.max(safeLeft,safeRight-totalWidth))));
  const anchorX=x+(position==="left"?0:position==="center"?totalWidth/2:totalWidth);
  const cameraBelow=layout==="split"&&input.regionOrder==="b_first";
  const room=(cameraBelow?seam:height-cameraBottom)-(style==="plain"?side/2:0);
  const maxOffset=Math.max(0,Math.min(height*.22,room));
  const offset=maxOffset*(input.seamOffset??0)/100;
  const centerY=(style==="boxed"?cameraBelow?cameraTop+side/2:Math.max(cameraBottom-side/2,cameraTop+side/2):seam)+(cameraBelow?-offset:offset);
  const y=Math.round(centerY-side/2);
  return {x,y,side,fontSize,totalWidth,anchorX,centerY};
}
