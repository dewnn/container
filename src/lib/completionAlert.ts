import {isTauri} from "@tauri-apps/api/core";
import {getCurrentWindow,UserAttentionType} from "@tauri-apps/api/window";

export async function completionAlert(message:string){
  try{
    if(localStorage.getItem("container-completion-alert")!=="true")return;
    window.dispatchEvent(new CustomEvent("container-toast",{detail:{message,kind:"success"}}));
    if(isTauri())await getCurrentWindow().requestUserAttention(UserAttentionType.Informational);
  }catch{/* Optional alerts never change a completed job's result. */}
}
