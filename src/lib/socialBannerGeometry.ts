import { sourceRegion } from "./clipperLayoutGeometry.ts";

export interface SocialBannerInput {
  width:number; height:number; sourceWidth:number; sourceHeight:number;
  seamOffset?:number;
  layout:string; regionAHeight:number; regionOrder:string;
  regionAWidth:number; regionARegionHeight:number;
  freecamSize:number; freecamX:number; freecamY:number;
  username:string; size:number; textUnits?:number;
}

export interface SocialBannerGeometry {
  x:number; y:number; width:number; height:number; factor:number;
  offset:number;
  barHeight:number;
  logoX:number; logoWidth:number; logoArtWidth:number; logoImageBottom:number;
  prefixX:number; prefixWidth:number; prefixArtWidth:number; prefixImageLeft:number; prefixImageBottom:number;
  textX:number; textCenterY:number; textSize:number;
}

// The supplied 1080px art provides the exact green logo and KICK.COM/ raster.
// Crop the logo and prefix separately; fit the username beside them while
// keeping the full wordmark inside the right-side safe area.
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
    const source=sourceRegion({x:0,y:0,width:input.regionAWidth,height:input.regionARegionHeight},input.sourceWidth,input.sourceHeight);
    const sourceWidth=source.width,sourceHeight=source.height;
    const cameraHeight=Math.max(2,even(cameraWidth*sourceHeight/Math.max(1,sourceWidth)));
    cameraLeft=Math.max(0,input.width-cameraWidth)*input.freecamX/100;
    cameraTop=Math.max(0,input.height-cameraHeight)*input.freecamY/100;
    cameraBottom=cameraTop+cameraHeight;
  }else if(input.layout==="blur"||input.layout==="original"){
    const bandHeight=Math.min(input.height,input.width*input.sourceHeight/Math.max(1,input.sourceWidth));
    cameraBottom=(input.height+bandHeight)/2;
  }else if(input.layout==="fill"){
    cameraBottom=input.height*.55;
  }
  const text=input.username.toUpperCase();
  const estimatedUnits=Array.from(text).reduce((sum,letter)=>sum+(/[I1.,:!|]/.test(letter)?.29:/[MW@]/.test(letter)?.68:.5),0);
  const units=Number.isFinite(input.textUnits)&&input.textUnits!==undefined&&input.textUnits>0?input.textUnits:estimatedUnits;
  // Keep the reference font size for ordinary names. For a longer name, move
  // the whole wordmark left first; shrink only once it reaches the logo.
  const factor=Math.min(input.size/54,1);
  const base=cameraWidth/1080;
  const logoScale=base*factor*1.035;
  const logoX=input.layout==="freecam"?Math.ceil(cameraLeft):cameraLeft-31.5*base*factor;
  const logoWidth=350*logoScale;
  const minPrefixX=logoX+logoWidth+18*base*factor;
  const referencePrefixX=cameraLeft+516*base*factor;
  const safeRight=cameraLeft+cameraWidth*.78;
  const fullTextWidth=base*factor*1.18*(147+32*units);
  const textFit=Math.min(1,Math.max(.1,(safeRight-minPrefixX)/fullTextWidth));
  const textScale=base*factor*1.18*textFit;
  const prefixX=Math.max(minPrefixX,Math.min(referencePrefixX,safeRight-fullTextWidth*textFit));
  const height=113*logoScale;
  const maxOffset=["split","squares","freecam","blur","original","fill"].includes(input.layout)?Math.max(0,Math.min(input.height*.22,cameraBelow?seam:input.height-cameraBottom)):0;
  const offset=maxOffset*(input.seamOffset??0)/100;
  const y=cameraBelow?seam-offset:cameraBottom-height+offset;
  const bottom=y+height;
  return {
    x:cameraLeft,y,width:cameraWidth,height,factor,offset,
    barHeight:69*base*factor*1.18,
    logoX,logoWidth,logoArtWidth:1080*logoScale,logoImageBottom:12*logoScale,
    prefixX,prefixWidth:147*textScale,prefixArtWidth:1080*textScale,prefixImageLeft:-419*textScale,prefixImageBottom:5*textScale,
    textX:prefixX+147*textScale,textCenterY:bottom-35*base*factor*1.18,textSize:32*textScale,
  };
}
