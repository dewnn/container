import { readFileSync } from "node:fs";
import { socialTagGeometry } from "../src/lib/socialTagGeometry.ts";

interface Fixture { layout:string;style:"boxed"|"plain";username:string;x:number;y:number;side:number;fontSize:number }
const fixtures=JSON.parse(readFileSync(new URL("../tests/fixtures/social-tag-geometry.json",import.meta.url),"utf8")) as Fixture[];
for(const fixture of fixtures){
  const actual=socialTagGeometry({width:360,height:640,sourceWidth:1920,sourceHeight:1080,layout:fixture.layout,style:fixture.style,username:fixture.username,size:36,regionAHeight:30,regionOrder:"a_first",regionAWidth:50,regionARegionHeight:50,freecamSize:50,freecamX:50,freecamY:2});
  for(const key of ["x","y","side","fontSize"] as const){
    if(actual[key]!==fixture[key])throw new Error(`${fixture.layout}/${fixture.style} ${key}: preview=${actual[key]} expected=${fixture[key]}`);
  }
}
console.log(`Social Tag preview geometry: ${fixtures.length} export fixtures matched`);
