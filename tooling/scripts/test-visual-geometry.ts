import { readFileSync } from "node:fs";
import { socialTagGeometry } from "../../src/lib/socialTagGeometry.ts";
import { socialBannerGeometry } from "../../src/lib/socialBannerGeometry.ts";

interface Fixture { layout:string;style:"boxed"|"plain";position:"left"|"center"|"right";username:string;x:number;y:number;side:number;fontSize:number }
const fixtures=JSON.parse(readFileSync(new URL("../tests/fixtures/social-tag-geometry.json",import.meta.url),"utf8")) as Fixture[];
for(const fixture of fixtures){
  const actual=socialTagGeometry({width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:fixture.layout,style:fixture.style,position:fixture.position,username:fixture.username,size:36,regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2});
  for(const key of ["x","y","side","fontSize"] as const){
    if(actual[key]!==fixture[key])throw new Error(`${fixture.layout}/${fixture.style} ${key}: preview=${actual[key]} expected=${fixture[key]}`);
  }
}
const oldBoxed=socialTagGeometry({width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:"split",style:"boxed",position:"right",username:"Example",size:36,regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2});
if(oldBoxed.x!==55)throw new Error("Legacy boxed side positions must render centered.");
for(const style of ["boxed","plain"] as const){
  for(const position of ["left","center","right"] as const){
    const result=socialTagGeometry({width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:"split",style,position,username:"a_very_long_kick_username_12345",size:36,regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2});
    const inset=style==="boxed"?18:0;
    if(result.x<inset-1||result.x+result.totalWidth>360-inset+1)throw new Error(`${style}/${position} long Social Tag escapes its output bounds`);
  }
}
const measured=socialTagGeometry({width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:"split",style:"plain",position:"center",username:"Example",size:36,textUnits:4.72314,regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2});
if(measured.x!==64||Math.abs(measured.anchorX-180)>1)throw new Error(`Measured Social Tag center drifted: ${JSON.stringify(measured)}`);
const bannerInput={width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:"split",regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2,username:"ADINROSS",size:54,textUnits:4};
const banner=socialBannerGeometry(bannerInput);
if(Math.abs(banner.y-153.015)>.01||Math.abs(banner.prefixX-172)>.01||Math.abs(banner.textCenterY-178.23333333333332)>.01)throw new Error(`Kick banner reference placement drifted: ${JSON.stringify(banner)}`);
const smaller=socialBannerGeometry({...bannerInput,size:36});
const gap=(g:typeof banner)=>g.prefixX-g.logoX-g.logoWidth;
if(Math.abs(banner.height/smaller.height-1.5)>.01||Math.abs(banner.textSize/smaller.textSize-1.5)>.01||Math.abs(banner.logoWidth/smaller.logoWidth-1.5)>.01||Math.abs(gap(banner)/gap(smaller)-1.5)>.01)throw new Error("Kick banner logo, font and gaps must scale proportionally.");
const long=socialBannerGeometry({...bannerInput,size:54,username:"A_VERY_LONG_KICK_USERNAME_12345",textUnits:18});
if(long.textX+long.textSize*18>long.x+long.width*.8+1)throw new Error("Long Kick banner usernames must avoid the phone action rail.");
const batuhan=socialBannerGeometry({...bannerInput,username:"BATUHANFURKAN5",textUnits:8});
if(Math.abs(batuhan.textSize-banner.textSize)>.01||batuhan.prefixX>=banner.prefixX||batuhan.textX+batuhan.textSize*8>batuhan.x+batuhan.width*.8+1)throw new Error("Longer Kick names must keep the reference font size, move left and remain safe.");
const reversed=socialBannerGeometry({...bannerInput,regionOrder:"b_first"});
if(reversed.y!==192)throw new Error("The Kick banner must remain on the camera side of a reversed split.");
const lowered=socialBannerGeometry({...bannerInput,seamOffset:100});
if(Math.abs(lowered.y-banner.y-640*.22)>.01)throw new Error("Split Kick banner must move below the camera without changing its size.");
const raised=socialBannerGeometry({...bannerInput,regionOrder:"b_first",seamOffset:100});
if(Math.abs(raised.y-reversed.y+640*.22)>.01)throw new Error("Reversed split Kick banner must move away from the camera.");
const freecam=socialBannerGeometry({...bannerInput,layout:"freecam"});
if(freecam.x!==90||freecam.width!==180)throw new Error("The Kick banner must follow the freecam camera bounds.");
if(freecam.logoX<freecam.x)throw new Error("Freecam Kick logo must not escape the camera's left edge.");
for(const layout of ["squares","freecam"]){
  const original=socialBannerGeometry({...bannerInput,layout});
  const moved=socialBannerGeometry({...bannerInput,layout,seamOffset:100});
  if(Math.abs(moved.y-original.y-640*.22)>.01||moved.y+moved.height>640+.01)throw new Error(`${layout} Kick banner must move down without escaping the output.`);
}
const bottomFreecam=socialBannerGeometry({...bannerInput,layout:"freecam",freecamY:100,seamOffset:100});
if(bottomFreecam.y+bottomFreecam.height>640+.01)throw new Error("Freecam banner offset must clamp to the output bottom.");
const blurBanner=socialBannerGeometry({...bannerInput,layout:"blur"});
const loweredBlur=socialBannerGeometry({...bannerInput,layout:"blur",seamOffset:100});
if(loweredBlur.y<=blurBanner.y+blurBanner.height)throw new Error("Blur Kick banner must move away from the sharp band.");
const originalBanner=socialBannerGeometry({...bannerInput,layout:"original"});
const loweredOriginal=socialBannerGeometry({...bannerInput,layout:"original",seamOffset:100});
if(loweredOriginal.y<=originalBanner.y+originalBanner.height)throw new Error("Original Kick banner must move away from the sharp band.");
if(Math.abs(originalBanner.y-blurBanner.y)>.01)throw new Error("Original and Blur foreground geometry must match.");
const fillBanner=socialBannerGeometry({...bannerInput,layout:"fill"});
const movedFill=socialBannerGeometry({...bannerInput,layout:"fill",seamOffset:50});
if(movedFill.y<=fillBanner.y+20)throw new Error("Fill Kick banner distance must move the preview.");
for(const layout of ["split","squares","freecam","original","blur","fill"]){
  for(const style of ["boxed","plain"] as const){
    const input={...bannerInput,layout,style,position:"center" as const};
    const base=socialTagGeometry(input);
    const moved=socialTagGeometry({...input,seamOffset:50});
    if(moved.centerY<=base.centerY+10)throw new Error(`${layout}/${style} tag distance must move below the camera.`);
    if(moved.x!==base.x||moved.side!==base.side)throw new Error(`${layout}/${style} tag distance changed horizontal placement or size.`);
  }
}
for(const style of ["boxed","plain"] as const){
  const input={...bannerInput,layout:"split",regionOrder:"b_first",style,position:"center" as const};
  const base=socialTagGeometry(input);
  const moved=socialTagGeometry({...input,seamOffset:50});
  if(moved.centerY>=base.centerY-10)throw new Error(`Reversed split ${style} tag must move above the camera.`);
}
console.log(`Social Tag preview geometry: ${fixtures.length} export fixtures and Kick banner proportions matched`);
