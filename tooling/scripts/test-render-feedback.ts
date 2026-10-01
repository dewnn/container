import assert from "node:assert/strict";
import {remainingSeconds,renderProblem} from "../../src/lib/renderFeedback.ts";
assert.equal(remainingSeconds(50,10),10);
assert.equal(remainingSeconds(25,10),30);
for(const [percent,elapsed] of [[0,10],[50,2],[100,10],[NaN,10],[50,Infinity]])assert.equal(remainingSeconds(percent,elapsed),null);
for(const message of ["No space left on device","Disk full (os error 112)","Sharing violation (os error 32)","No such file or directory","Permission denied","Invalid data found when processing input"]){
  for(const language of ["en","tr"] as const)assert.ok(renderProblem(message,language));
}
assert.equal(renderProblem("Unknown failure","en"),null);
console.log("Render feedback: ETA boundaries and actionable errors passed");
