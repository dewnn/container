import assert from "node:assert/strict";
import { startStages, checkpointStage, continueStage, validStages } from "../src/lib/stageHistory.ts";
import { projectResources, replaceProjectResource } from "../src/lib/projectResources.ts";
const a={mediaPath:"a.mp4",savedAt:1,toolbox:{output:"b.mp4",mediaUrl:"old",text:"hello"}};
const b={mediaPath:"b.mp4",savedAt:2,toolbox:{output:"c.mp4",mediaUrl:"new",text:"world"}};
let h=continueStage(startStages(a,"Text"),b,"SmartCut");
assert.equal(h.entries.length,2);
h.current=1;
const unchanged=checkpointStage(h,{...a,savedAt:9,toolbox:{...a.toolbox,mediaUrl:"refreshed"}},"Text");
assert.equal(unchanged.entries.length,2,"URL refresh must not fork");
const fork=checkpointStage(h,{...a,toolbox:{...a.toolbox,text:"changed"}},"Text");
assert.equal(fork.entries.length,3);
assert.equal(fork.entries[1].session.toolbox.text,"world","Forward branch is retained");
assert.equal(fork.entries[0].session.toolbox.text,"hello","Ancestor is immutable");
assert.equal(fork.entries[2].parent,1);
assert.equal(h.entries.length,2,"Helpers must not mutate input");
const resumed=continueStage(fork,b,"SmartCut");
assert.equal(resumed.entries[3].parent,3);
const edited=checkpointStage(resumed,{...b,savedAt:4,toolbox:{...b.toolbox,text:"draft"}},"SmartCut");
assert.equal(edited.entries.length,4,"Leaf drafts update in place");
let long=edited;
for(let i=0;i<50;i++)long=continueStage(long,b,"Stage");
assert.equal(long.entries.length,30);assert.equal(long.trimmed,true);
const isSession=(v:unknown):v is typeof a=>!!v&&typeof (v as typeof a).mediaPath==="string";
assert(validStages(long,isSession));
const oldest=long.entries[0].id;
long.current=long.entries[1].id;
const protectedHistory=checkpointStage(long,{...a,savedAt:50},"Edited",oldest);
assert(protectedHistory.entries.some(entry=>entry.id===oldest),"The requested destination must survive history trimming");
assert(!validStages({...long,current:-1},isSession));
assert(!validStages({...long,entries:[{...long.entries[0],session:{...a,stageHistory:h}}]},isSession));
const project={...a,stageHistory:h};
assert(!projectResources(project).some(r=>r.path==="b.mp4"),"Current validation does not require historical files");
assert(projectResources(project,true).some(r=>r.path==="c.mp4"),"Project relocation includes outputs");
assert.equal(replaceProjectResource(project,"b.mp4","moved.mp4").stageHistory.entries[1].session.mediaPath,"moved.mp4");
console.log("stage history: continuation, branches, drafts, limits, validation and relocation passed");

// Repeated branch/navigation operations exercise retention without losing the destination.
let stress=startStages(a,"Transform");
for(let i=0;i<240;i++){
  const before=JSON.stringify(stress);
  const destination=stress.entries[(i*7)%stress.entries.length].id;
  const updated=checkpointStage(stress,{...a,toolbox:{...a.toolbox,text:`edit-${i}`}},"Text",destination);
  assert.equal(JSON.stringify(stress),before,"Checkpoint must not mutate the existing graph");
  assert(updated.entries.some(e=>e.id===destination),"Navigation target survives trimming");
  updated.current=destination;
  stress=continueStage(updated,{...b,mediaPath:`output-${i}.mp4`},"Transform");
  assert(validStages(stress,isSession),`Graph remains valid after operation ${i}`);
  assert(stress.entries.length<=30);
  assert(stress.entries.some(e=>e.id===stress.current));
}
let large=startStages({...a,payload:"x".repeat(200_000)},"Text");
for(let i=0;i<10;i++)large=continueStage(large,{...a,payload:"x".repeat(200_000)},"Text");
assert(large.trimmed);assert(JSON.stringify(large).length<=750_000);
assert(validStages(large,isSession));
const localized={...a,toolbox:{selected:{id:"transform",title:"Transform",fields:[{key:"rotate",value:"0",label:"Rotate"}]}}};
const localizedHistory=continueStage(startStages(localized,"Transform"),localized,"Transform");
localizedHistory.current=1;
const translated=structuredClone(localized);translated.toolbox.selected.title="Dönüştür";translated.toolbox.selected.fields[0].label="Döndür";
assert.equal(checkpointStage(localizedHistory,translated,"Dönüştür").entries.length,2,"Translation alone must not create a branch");
for(const invalid of [
  {...h,next:1},
  {...h,entries:[h.entries[0],h.entries[0]]},
  {...h,entries:h.entries.map(e=>({...e,parent:e.id}))},
  {...h,entries:h.entries.map(e=>({...e,parent:999}))},
  {...h,entries:h.entries.map(e=>({...e,session:null}))},
])assert(!validStages(invalid,isSession));
console.log("stage history stress: 240 branch cycles, byte budget, localization and invalid graphs passed");
