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
for(const style of ["boxed","plain"] as const){
  for(const position of ["left","center","right"] as const){
    const result=socialTagGeometry({width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:"split",style,position,username:"a_very_long_kick_username_12345",size:36,regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2});
    const inset=style==="boxed"?18:0;
    if(result.x<inset-1||result.x+result.totalWidth>360-inset+1)throw new Error(`${style}/${position} long Social Tag escapes its output bounds`);
  }
}
const measured=socialTagGeometry({width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:"split",style:"plain",position:"center",username:"Example",size:36,textUnits:4.72314,regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2});
if(measured.x!==64||Math.abs(measured.anchorX-180)>1)throw new Error(`Measured Social Tag center drifted: ${JSON.stringify(measured)}`);
const bannerInput={width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:"split",regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2,username:"ADINROSS",size:36,textUnits:4};
const banner=socialBannerGeometry(bannerInput);
if(Math.abs(banner.y-158.33333333333334)>.01||Math.abs(banner.textX-188.66666666666666)>.01||Math.abs(banner.textCenterY-182)>.01)throw new Error(`Kick banner source proportions drifted: ${JSON.stringify(banner)}`);
const bigger=socialBannerGeometry({...bannerInput,size:54});
if(Math.abs(bigger.height/banner.height-1.5)>.01||Math.abs(bigger.textSize/banner.textSize-1.5)>.01||Math.abs((bigger.textCenterY-bigger.y)/bigger.scale-71)>.01)throw new Error("Kick banner logo and text must scale together around the supplied art's centerline.");
const long=socialBannerGeometry({...bannerInput,size:54,username:"A_VERY_LONG_KICK_USERNAME_12345",textUnits:18});
if(long.textX+long.textSize*18>long.x+long.width-5)throw new Error("Long Kick banner usernames must fit inside the camera width.");
const reversed=socialBannerGeometry({...bannerInput,regionOrder:"b_first"});
if(reversed.y!==192)throw new Error("The Kick banner must remain on the camera side of a reversed split.");
const freecam=socialBannerGeometry({...bannerInput,layout:"freecam"});
if(freecam.x!==90||freecam.width!==180)throw new Error("The Kick banner must follow the freecam camera bounds.");
console.log(`Social Tag preview geometry: ${fixtures.length} export fixtures and Kick banner proportions matched`);
