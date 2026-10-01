export function imageOutputFormat(path:string,format:string):string{
  if(format!=="source")return format==="jpeg"?"jpg":format;
  const extension=path.split(/[\\/]/).pop()?.split(".").pop()?.toLowerCase();
  if(extension==="jpeg")return "jpg";
  if(extension==="tif")return "tiff";
  return extension&&["jpg","png","webp","bmp","tiff","avif"].includes(extension)?extension:"png";
}
export function imageQualityAdjustable(format:string){return ["jpg","webp","avif"].includes(format)}
export function imageTargetSupported(format:string){return ["jpg","webp","png"].includes(format)}
