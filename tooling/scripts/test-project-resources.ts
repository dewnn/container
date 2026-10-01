import { projectResources, replaceProjectResource } from "../../src/lib/projectResources.ts";

function assert(condition:boolean,message:string){if(!condition)throw new Error(message)}

const session={mediaPath:"C:\\video.mp4",toolbox:{selected:{fields:[{type:"file",key:"image_path",label:"Overlay image",value:"C:\\logo.png"}]},mergeInputs:["C:\\video.mp4","C:\\second.mp4"]},autocut:{linkedTracks:[{path:"C:\\mic.wav"}]},batch:{items:[{path:"C:\\second.mp4"}]}};
const references=projectResources(session);
assert(references.length===4,"Project resource inventory missed or duplicated a file.");
const replaced=replaceProjectResource(session,"C:\\video.mp4","D:\\moved\\video.mp4");
assert(replaced.mediaPath==="D:\\moved\\video.mp4"&&replaced.toolbox.mergeInputs[0]==="D:\\moved\\video.mp4","Relinking did not update every saved reference.");
assert(session.mediaPath==="C:\\video.mp4","Relinking mutated the original session.");
const stacked={mediaPath:"C:\\video.mp4",toolbox:{processingStack:[{tool:{fields:[{type:"file",key:"image_path",label:"Stack overlay",value:"C:\\old-logo.png"}]},params:{image_path:"C:\\new-logo.png"}},{tool:{fields:[{type:"file",key:"image_path",label:"Stack overlay",value:"C:\\new-logo.png"}]},params:{image_path:"C:\\new-logo.png"}}]}};
const stackReferences=projectResources(stacked);
assert(stackReferences.length===2&&stackReferences[1].path==="C:\\new-logo.png","Processing Stack resources were missed or duplicated.");
const relinked=replaceProjectResource(stacked,"C:\\new-logo.png","D:\\moved\\logo.png");
assert(relinked.toolbox.processingStack[0].params.image_path==="D:\\moved\\logo.png","Stack resource relinking failed.");
const smartcutStack={mediaPath:"C:\\video.mp4",toolbox:{processingStack:[{tool:{id:"smartcut",fields:[]},params:{source_path:"C:\\video.mp4"},smartcutSession:{analysisInput:"C:\\mic.wav",linkedTracks:[{path:"C:\\mic.wav"},{path:"C:\\camera.mp4"}]}}]}};
assert(projectResources(smartcutStack).length===3,"Saved SmartCut Stack lost or duplicated its analysis resources.");
const movedSmartcut=replaceProjectResource(smartcutStack,"C:\\video.mp4","D:\\video.mp4");
assert(movedSmartcut.toolbox.processingStack[0].params.source_path==="D:\\video.mp4","Relinking lost SmartCut's source identity.");
console.log("project resource scenarios: 7 passed");
