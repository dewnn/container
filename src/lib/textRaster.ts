export interface TextAppearance {text:string;x:number;y:number;size:number;wrap_width?:number;color:string;opacity:number;align:"left"|"center"|"right";fontName:string;font_path:string;outline:number;outline_color:string;shadow:number;shadow_color:string;background:boolean;background_color:string;background_opacity:number;background_padding:number}
const color=(hex:string,opacity:number)=>/^#[0-9a-f]{6}$/i.test(hex)?`${hex}${Math.round(Math.max(0,Math.min(100,opacity))*2.55).toString(16).padStart(2,"0")}`:"transparent";
export function rasterText(canvas:HTMLCanvasElement,layer:TextAppearance,width:number,height:number){
  canvas.width=width;canvas.height=height;
  const ctx=canvas.getContext("2d")!;ctx.font=`400 ${layer.size}px ${JSON.stringify(layer.fontName)}, "Segoe UI Emoji", sans-serif`;
  const x=width*layer.x/100,y=height*layer.y/100;
  const anchor=layer.align==="left"?1-layer.x/100:layer.align==="right"?layer.x/100:2*Math.min(layer.x/100,1-layer.x/100);
  const maxWidth=layer.wrap_width?Math.max(layer.size,layer.wrap_width*layer.size):Math.max(layer.size,width*Math.min(.9,Math.max(.05,anchor))-(layer.background?layer.background_padding*2:0)-layer.outline-Math.max(0,layer.shadow));
  const lines:string[]=[];
  for(const paragraph of layer.text.replace(/\r\n?/g,"\n").split("\n")){
    let line="";for(const word of paragraph.trim().split(/\s+/)){const next=line?`${line} ${word}`:word;if(line&&ctx.measureText(next).width>maxWidth){lines.push(line);line=word}else line=next}lines.push(line);
  }
  const lineHeight=layer.size*1.05,h=lineHeight*lines.length,w=Math.max(0,...lines.map(line=>ctx.measureText(line).width));
  const left=layer.align==="left"?x:layer.align==="right"?x-w:x-w/2;
  const padding=layer.background?layer.background_padding:0;
  if(layer.background){ctx.fillStyle=color(layer.background_color,layer.background_opacity);ctx.fillRect(left-padding,y-h/2-padding,w+padding*2,h+padding*2)}
  ctx.textAlign=layer.align;ctx.textBaseline="middle";ctx.fillStyle=color(layer.color,layer.opacity);ctx.strokeStyle=color(layer.outline_color,layer.opacity);ctx.lineWidth=layer.outline;ctx.lineJoin="round";ctx.miterLimit=2;
  ctx.shadowColor=color(layer.shadow_color,layer.opacity*.75);ctx.shadowOffsetX=layer.shadow;ctx.shadowOffsetY=layer.shadow;
  lines.forEach((line,i)=>{const baseline=y-h/2+lineHeight*(i+.5);if(layer.outline>0)ctx.strokeText(line,x,baseline);ctx.fillText(line,x,baseline)});
  return {left:left-padding,top:y-h/2-padding,width:w+padding*2,height:h+padding*2};
}
