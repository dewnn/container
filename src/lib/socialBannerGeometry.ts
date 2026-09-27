export interface SocialBannerInput {
  width:number; height:number; sourceWidth:number; sourceHeight:number;
  layout:string; regionAHeight:number; regionOrder:string;
  regionAWidth:number; regionARegionHeight:number;
  freecamSize:number; freecamX:number; freecamY:number;
  username:string; size:number; textUnits?:number;
}

export interface SocialBannerGeometry {
  x:number; y:number; width:number; height:number; scale:number;
  barY:number; barHeight:number; artWidth:number;
  textX:number; textCenterY:number; textSize:number;
}

// Source art is 1080 square. Its only visible strip is y=979..1079:
// green KICK logo at x=62..327, KICK.COM/ at x=419..565, bar at y=1016..1079.
export function socialBannerGeometry(input:SocialBannerInput):SocialBannerGeometry {
  const even=(value:number)=>Math.floor(Math.round(value)/2)*2;
  let cameraLeft=0,cameraTop=0,cameraWidth=input.width,cameraBottom=input.height;
  let seam=input.height;
  let cameraBelow=false;
  if(input.layout==="split"){
    seam=even(input.height*input.regionAHeight/100);
    cameraBelow=input.regionOrder==="b_first";
    if(cameraBelow)cameraTop=seam;else cameraBottom=seam;
  }else if(input.layout==="squares"){
    seam=Math.floor(input.height/4)*2;
    cameraBottom=seam;
  }else if(input.layout==="freecam"){
    cameraWidth=even(input.width*input.freecamSize/100);
    const sourceWidth=input.sourceWidth*input.regionAWidth/100;
    const sourceHeight=input.sourceHeight*input.regionARegionHeight/100;
    const cameraHeight=Math.max(2,even(cameraWidth*sourceHeight/Math.max(1,sourceWidth)));
    cameraLeft=Math.max(0,input.width-cameraWidth)*input.freecamX/100;
    cameraTop=Math.max(0,input.height-cameraHeight)*input.freecamY/100;
    cameraBottom=cameraTop+cameraHeight;
  }
  const text=input.username.toUpperCase();
  const estimatedUnits=Array.from(text).reduce((sum,letter)=>sum+(/[I1.,:!|]/.test(letter)?.29:/[MW@]/.test(letter)?.68:.5),0);
  const units=Number.isFinite(input.textUnits)&&input.textUnits!==undefined&&input.textUnits>0?input.textUnits:estimatedUnits;
  // Keep the supplied logo/prefix and the typed username at one scale. Long
  // names shrink the entire strip rather than squeezing only its text.
  const factor=Math.max(.25,Math.min(input.size/36,1.5,1056/(566+32*units)));
  const scale=cameraWidth/1080*factor;
  const stripHeight=101*scale;
  const y=cameraBelow?seam:cameraBottom-stripHeight;
  return {
    x:cameraLeft,y,width:cameraWidth,height:stripHeight,scale,
    barY:y+37*scale,barHeight:64*scale,artWidth:1080*scale,
    textX:cameraLeft+566*scale,textCenterY:y+71*scale,textSize:32*scale,
  };
}
