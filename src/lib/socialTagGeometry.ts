export interface SocialTagGeometryInput {
  width:number; height:number; sourceWidth:number; sourceHeight:number;
  layout:string; style:"boxed"|"plain"; username:string; size:number;
  regionAHeight:number; regionOrder:string;
  regionAWidth:number; regionARegionHeight:number;
  freecamSize:number; freecamX:number; freecamY:number;
}

export interface SocialTagGeometry { x:number; y:number; side:number; fontSize:number; totalWidth:number; anchorX:number; centerY:number }

export function socialTagGeometry(input:SocialTagGeometryInput):SocialTagGeometry {
  const {width,height,layout,style,username}=input;
  const units=Array.from(username).reduce((sum,letter)=>sum+(/[ilI1.,:!|]/.test(letter)?.35:/[mwMW@]/.test(letter)?.9:/[A-Z]/.test(letter)?.72:.59),0);
  let cameraLeft=0,cameraTop=0,cameraWidth=width,cameraBottom=height,seam=height*.78;
  const even=(value:number)=>Math.floor(Math.round(value)/2)*2;
  if(layout==="split"){
    seam=even(height*input.regionAHeight/100);
    if(input.regionOrder==="b_first"){cameraTop=seam;cameraBottom=height}else cameraBottom=seam;
  }else if(layout==="squares"){
    seam=Math.floor(height/4)*2;cameraBottom=seam;
  }else if(layout==="freecam"){
    cameraWidth=even(width*input.freecamSize/100);
    const sourceWidth=input.sourceWidth*input.regionAWidth/100;
    const sourceHeight=input.sourceHeight*input.regionARegionHeight/100;
    const cameraHeight=Math.max(2,even(cameraWidth*sourceHeight/Math.max(1,sourceWidth)));
    cameraLeft=Math.max(0,width-cameraWidth)*input.freecamX/100;
    cameraTop=Math.max(0,height-cameraHeight)*input.freecamY/100;
    cameraBottom=cameraTop+cameraHeight;seam=cameraBottom;
  }
  const anchorX=style==="boxed"?cameraLeft:layout==="freecam"?cameraLeft+cameraWidth/2:width/2;
  const available=style==="boxed"?Math.max(width-anchorX,24):width*.84;
  const fontSize=Math.min(input.size,available/(units*(style==="boxed"?1.1:1)+2.4));
  const side=Math.max(8,Math.round(fontSize*1.5));
  const gap=style==="plain"?fontSize*.22:0;
  const padding=Math.max(2,Math.round(fontSize*.34));
  const textWidth=units*fontSize*(style==="boxed"?1.1:1);
  const totalWidth=side+gap+textWidth+(style==="boxed"?padding*2:0);
  const x=Math.round(Math.max(0,style==="boxed"?Math.min(anchorX,Math.max(0,width-totalWidth)):anchorX-totalWidth/2));
  const centerY=style==="boxed"?Math.max(cameraBottom-side/2,cameraTop+side/2):seam;
  const y=Math.round(centerY-side/2);
  return {x,y,side,fontSize,totalWidth,anchorX,centerY};
}
