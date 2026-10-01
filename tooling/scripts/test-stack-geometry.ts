import assert from "node:assert/strict";
import {stackOutputDimensions,scaleStackText} from "../../src/lib/stackGeometry.ts";

const source={width:1920,height:1080};
const base={rotate:"0",crop_mode:"off",crop_w:"100",crop_h:"100",size_mode:"source"};
assert.deepEqual(stackOutputDimensions("transform",{...base,rotate:"90"},source),{width:1080,height:1920});
assert.deepEqual(stackOutputDimensions("transform",{...base,crop_mode:"free",crop_w:"50",crop_h:"50",size_mode:"height",size:"720"},source),{width:1280,height:720});
assert.deepEqual(stackOutputDimensions("transform",{...base,rotate:"90",crop_mode:"free",crop_w:"50",crop_h:"50",size_mode:"width",size:"720"},source),{width:720,height:1280});
assert.deepEqual(stackOutputDimensions("transform",{...base,size_mode:"exact",output_width:"1081",output_height:"1921"},source),{width:1080,height:1920});
assert.deepEqual(stackOutputDimensions("clipper",{...base,crop_mode:"9:16",vertical_layout:"split",size_mode:"exact",output_width:"1080",output_height:"1920"},source),{width:1080,height:1920});
assert.deepEqual(stackOutputDimensions("color",{},source),source);
assert.throws(()=>stackOutputDimensions("transform",{...base,size_mode:"exact",output_width:"oops",output_height:"1920"},source));
const layer={size:50,outline:4,shadow:2,background_padding:10,x:50,y:80,wrap_width:12};
assert.deepEqual(scaleStackText(layer,1920,960),{...layer,size:25,outline:2,shadow:1,background_padding:5});
assert.deepEqual(layer,{size:50,outline:4,shadow:2,background_padding:10,x:50,y:80,wrap_width:12});
assert.deepEqual(scaleStackText(layer,NaN,960),layer);
console.log("Stack geometry: rotation, crop, resize, Clipper, text scaling and legacy layers passed");
