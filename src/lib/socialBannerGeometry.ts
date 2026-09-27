export interface SocialBannerInput {
  width:number; height:number; sourceWidth:number; sourceHeight:number;
  layout:string; regionAHeight:number; regionOrder:string;
  regionAWidth:number; regionARegionHeight:number;
  freecamSize:number; freecamX:number; freecamY:number;
  username:string; size:number; textUnits?:number;
}

export interface SocialBannerGeometry {
  x:number; y:number; width:number; height:number; factor:number;
  barHeight:number;
  logoX:number; logoWidth:number; logoArtWidth:number; logoImageBottom:number;
  prefixX:number; prefixWidth:number; prefixArtWidth:number; prefixImageLeft:number; prefixImageBottom:number;
  textX:number; textCenterY:number; textSize:number;
}

// The supplied 1080px art provides the exact green logo and KICK.COM/ raster.
// The reference places the logo and wordmark independently, so both pieces
// are cropped from that art and scaled together around the same pivot.
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
  const designNameEnd=516+147*1.18+32*1.18*units;
  // 54 is the reference size. Fitting a long name scales every gap as well.
  const factor=Math.max(.25,Math.min(input.size/54,1,516/Math.max(1,designNameEnd-540)));
  const base=cameraWidth/1080;
  const logoScale=base*factor*1.035;
  const textScale=base*factor*1.18;
  const height=113*logoScale;
  const y=cameraBelow?seam:cameraBottom-height;
  const bottom=y+height;
  const pivot=(designX:number)=>cameraLeft+base*(540+factor*(designX-540));
  const logoX=pivot(-31.5);
  const prefixX=pivot(516);
  return {
    x:cameraLeft,y,width:cameraWidth,height,factor,
    barHeight:69*textScale,
    logoX,logoWidth:350*logoScale,logoArtWidth:1080*logoScale,logoImageBottom:12*logoScale,
    prefixX,prefixWidth:147*textScale,prefixArtWidth:1080*textScale,prefixImageLeft:-419*textScale,prefixImageBottom:5*textScale,
    textX:prefixX+147*textScale,textCenterY:bottom-35*textScale,textSize:32*textScale,
  };
}
