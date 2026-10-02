import {invoke,isTauri} from "@tauri-apps/api/core";
export interface PremiereStatus {supported:boolean;installed:boolean;connected:boolean;project:string;sequence_id:string;sequence_name:string}
const disconnected:PremiereStatus={supported:false,installed:false,connected:false,project:"",sequence_id:"",sequence_name:""};
export const premiere=$state({status:{...disconnected},sending:false,installing:false});
let refreshing=false;
export async function refreshPremiere(){
  if(!isTauri()||refreshing||premiere.sending)return;
  refreshing=true;
  try{const value=await invoke<PremiereStatus>("premiere_status");if(!premiere.sending)premiere.status=value&&typeof value.connected==="boolean"?value:{...disconnected}}catch{if(!premiere.sending)premiere.status={...disconnected}}finally{refreshing=false}
}
export function watchPremiere(){void refreshPremiere();const timer=setInterval(()=>void refreshPremiere(),2000);return()=>clearInterval(timer)}
