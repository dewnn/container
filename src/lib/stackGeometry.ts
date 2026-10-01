export interface StackDimensions { width:number; height:number }

// Mirrors Transform's rotation -> even crop -> scale order in the FFmpeg path.
export function stackOutputDimensions(operation:string,p:Record<string,string>,input:StackDimensions):StackDimensions{
  if(!["transform","clipper"].includes(operation))return {...input};
  let {width,height}=input;
  if(["90","270"].includes(p.rotate))[width,height]=[height,width];
  const even=(value:number)=>Math.max(2,Math.floor(value/2)*2);
  const nearestEven=(value:number)=>Math.max(2,Math.round(value/2)*2);
  const vertical=p.crop_mode==="9:16";
  if(p.crop_mode!=="off"&&(!vertical||(p.vertical_layout??"fill")==="fill")){
    width=even(width*Number(p.crop_w)/100);height=even(height*Number(p.crop_h)/100);
  }
  if(operation==="clipper"&&vertical&&p.vertical_layout==="fill"&&Number(p.clipper_zoom)>100){
    width=even(width*100/Number(p.clipper_zoom));height=even(height*100/Number(p.clipper_zoom));
  }
  if(p.size_mode==="exact"){width=even(Number(p.output_width));height=even(Number(p.output_height));}
  else if(p.size_mode==="height"){const next=even(Number(p.size));width=nearestEven(width*next/height);height=next;}
  else if(p.size_mode==="width"){const next=even(Number(p.size));height=nearestEven(height*next/width);width=next;}
  if(!Number.isFinite(width)||!Number.isFinite(height)||width<2||height<2)throw new Error("Invalid Stack dimensions.");
  return {width,height};
}

export function scaleStackText<T extends {size:number;outline:number;shadow:number;background_padding:number}>(layer:T,fromWidth:number,toWidth:number):T{
  const scale=Number.isFinite(fromWidth)&&fromWidth>0?toWidth/fromWidth:1;
  return {...layer,size:layer.size*scale,outline:layer.outline*scale,shadow:layer.shadow*scale,background_padding:layer.background_padding*scale};
}
