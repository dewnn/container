import { readFileSync } from "node:fs";
import { socialTagGeometry } from "../src/lib/socialTagGeometry.ts";

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
    if(result.x<0||result.x+result.totalWidth>361)throw new Error(`${style}/${position} long Social Tag escapes the output`);
  }
}
const measured=socialTagGeometry({width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:"split",style:"plain",position:"center",username:"Example",size:36,textUnits:4.72314,regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2});
if(measured.x!==64||Math.abs(measured.anchorX-180)>1)throw new Error(`Measured Social Tag center drifted: ${JSON.stringify(measured)}`);
console.log(`Social Tag preview geometry: ${fixtures.length} export fixtures matched`);
