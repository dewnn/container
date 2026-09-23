import { projectResources, replaceProjectResource } from "../src/lib/projectResources.ts";

function assert(condition:boolean,message:string){if(!condition)throw new Error(message)}

const session={mediaPath:"C:\\video.mp4",toolbox:{selected:{fields:[{type:"file",key:"image_path",label:"Overlay image",value:"C:\\logo.png"}]},mergeInputs:["C:\\video.mp4","C:\\second.mp4"]},autocut:{linkedTracks:[{path:"C:\\mic.wav"}]},batch:{items:[{path:"C:\\second.mp4"}]}};
const references=projectResources(session);
assert(references.length===4,"Project resource inventory missed or duplicated a file.");
const replaced=replaceProjectResource(session,"C:\\video.mp4","D:\\moved\\video.mp4");
assert(replaced.mediaPath==="D:\\moved\\video.mp4"&&replaced.toolbox.mergeInputs[0]==="D:\\moved\\video.mp4","Relinking did not update every saved reference.");
assert(session.mediaPath==="C:\\video.mp4","Relinking mutated the original session.");
console.log("project resource scenarios: 3 passed");
