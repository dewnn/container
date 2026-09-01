export type MediaKind = "video" | "audio" | "image";
export type FieldType = "number" | "text" | "select" | "range" | "file";

export interface Option { label: string; value: string }
export interface Field {
  key: string;
  label: string;
  type: FieldType;
  value: string | number;
  min?: number;
  max?: number;
  step?: number;
  unit?: string;
  options?: Option[];
  hint?: string;
  accept?: string[];
}

export interface Tool {
  id: string;
  title: string;
  category: string;
  kind: MediaKind[];
  description: string;
  detail: string;
  accent?: "blue" | "green" | "purple" | "red" | "yellow";
  fields: Field[];
}

const select = (key: string, label: string, value: string, options: [string, string][], hint?: string): Field => ({
  key, label, type: "select", value, options: options.map(([v, l]) => ({ value: v, label: l })), hint,
});
const number = (key: string, label: string, value: number, min: number, max: number, step = 1, unit = "", hint = ""): Field => ({
  key, label, type: "number", value, min, max, step, unit, hint,
});

export const tools: Tool[] = [
  {
    id: "ratio", title: "Ratio / Crop", category: "Transform", kind: ["video"], accent: "blue",
    description: "Videoyu seçilen en-boy oranına ortadan kırpar.",
    detail: "Görüntü esnetilmez. Kenarlardaki fazla alan merkezden kesilir.",
    fields: [select("ratio", "Target ratio", "9:16", [["1:1","1:1 square"],["4:5","4:5 portrait"],["9:16","9:16 reels"],["16:9","16:9 landscape"],["4:3","4:3 classic"]])],
  },
  {
    id: "resize", title: "Resize", category: "Transform", kind: ["video"], accent: "green",
    description: "Çözünürlüğü oranı koruyarak değiştirir.", detail: "Tek bir kenar seçilir; diğer kenar otomatik ve çift sayı hesaplanır.",
    fields: [select("axis", "Dimension", "height", [["height","Target height"],["width","Target width"]]), number("size", "Pixels", 1080, 2, 7680, 2, "px"), number("crf", "Quality / CRF", 16, 0, 30, 1, "", "Düşük değer daha kaliteli ve daha büyüktür.")],
  },
  {
    id: "fps", title: "Change FPS", category: "Motion", kind: ["video"], accent: "blue",
    description: "Kare hızını doğrudan değiştirir.", detail: "Yeni hareket üretmez; kare atar veya aynı kareyi tekrarlar.",
    fields: [number("fps", "Target FPS", 60, 1, 2400, 1, "fps"), number("crf", "Quality / CRF", 16, 0, 30)],
  },
  {
    id: "interpolation", title: "FPS Interpolation", category: "Motion", kind: ["video"], accent: "green",
    description: "Aralara harmanlanmış kareler koyarak FPS’i yükseltir.", detail: "Hedef giriş FPS’inden yüksek, 60’ın katı ve en fazla 2400 olmalıdır.",
    fields: [number("fps", "Target FPS", 60, 60, 2400, 60, "fps", "Dosya açıldığında uygun ilk 60 katına ayarlanır.")],
  },
  {
    id: "frame_blend", title: "Frame Blending", category: "Motion", kind: ["video"], accent: "purple",
    description: "Kareleri karıştırarak FPS’i düşürür.", detail: "Hareketlerde uzun pozlama benzeri izler oluşabilir.",
    fields: [number("fps", "Target FPS", 24, 1, 2399, 1, "fps")],
  },
  {
    id: "dedupe", title: "Duplicate Frame Removal", category: "Motion", kind: ["video"], accent: "yellow",
    description: "Birbirinin aynısı olan gereksiz kareleri kaldırır.",
    detail: "Takılan ekran kaydı, animasyon ve tekrar eden kareli videolarda işe yarar. Zaman damgaları korunur; ses hızlanmaz ve senkron bozulmaz. Çıktı değişken FPS olabilir.",
    fields: [select("profile", "Detection profile", "safe", [["safe","Safe / recommended"],["strong","Strong / more frames"]])],
  },
  {
    id: "speed", title: "Video Speed", category: "Motion", kind: ["video"], accent: "purple",
    description: "Görüntü ve sesi birlikte hızlandırır veya yavaşlatır.", detail: "0.5× iki kat uzun, 2× yaklaşık yarı süre demektir. Lossless Video yalnızca zaman damgalarını değiştirir; görüntüyü yeniden kodlamaz fakat sesi bilinçli olarak kaldırır.",
    fields: [number("speed", "Multiplier", 2, 0.05, 100, 0.05, "×"), select("speed_mode","Speed mode","synced",[["synced","Video + audio / synced"],["lossless_video","Lossless video only / no audio"]]), number("crf", "Quality / CRF", 16, 0, 30)],
  },
  {
    id: "compression", title: "Quality / Compression", category: "Quality", kind: ["video"], accent: "green",
    description: "CRF ile kalite ve dosya boyutunu dengeler.", detail: "16 yüksek kalite, 20 dengeli, 24 küçük dosya için uygundur.",
    fields: [number("crf", "CRF", 20, 0, 30, 1), select("preset", "CPU preset", "veryfast", [["ultrafast","Ultra fast"],["veryfast","Very fast"],["medium","Medium"],["slow","Slow / smaller"]])],
  },
  {
    id: "smart_quality", title: "Smart Quality Analysis", category: "Quality", kind: ["video"], accent: "blue",
    description: "Videoya uygun CRF değerini kısa örneklerle ölçer.",
    detail: "Videonun başından, ortasından ve sonundan kısa parçalar alır; 16, 20, 24 ve 28 CRF değerlerini dener. Her denemeyi VMAF ile kaynak görüntüye karşı ölçer. Final video oluşturmaz ve kaynak dosyaya dokunmaz.",
    fields: [
      select("goal", "Quality goal", "balanced", [["high","High quality"],["balanced","Balanced"],["small","Smaller file"]]),
      select("sample_duration", "Sample duration", "2", [["1","1 second / faster"],["2","2 seconds / recommended"],["3","3 seconds / more precise"]]),
    ],
  },
  {
    id: "bitrate", title: "Bitrate Control", category: "Quality", kind: ["video"], accent: "yellow",
    description: "Hedef video bitrate’i ile dosya büyüklüğünü kontrol eder.", detail: "Gerçek bitrate sahne karmaşıklığına göre biraz değişebilir.",
    fields: [number("mbps", "Target bitrate", 5, 0.05, 500, 0.05, "Mbps")],
  },
  {
    id: "discord_compressor", title: "Discord Compressor", category: "Quality", kind: ["video"], accent: "blue",
    description: "Videoyu Discord yükleme sınırına sığacak dosya boyutuna sıkıştırır.",
    detail: "Dosya boyutu sabit bir bütçedir: süre ve ses büyüdükçe görüntüye daha az alan kalır. Akıllı mod ses, çözünürlük ve FPS’i bu bütçeye birlikte uyarlar; iki geçişli kodlama da kalan alanı sahnelere daha verimli dağıtır.",
    fields: [
      number("target_mb", "Discord size limit", 20, 2, 2000, 1, "MB"),
      select("codec", "Video codec", "h264", [["h264","H.264 / safest"],["hevc","H.265 / better quality"]]),
      select("resolution", "Maximum resolution", "auto", [["auto","Smart for available bitrate"],["source","Source"],["1080","Up to 1080p"],["720","Up to 720p"],["480","Up to 480p"],["360","Up to 360p"],["240","Up to 240p"]]),
      select("fps_limit", "Frame rate limit", "auto", [["auto","Smart for available bitrate"],["source","Source"],["60","Up to 60 FPS"],["30","Up to 30 FPS"],["24","Up to 24 FPS"]]),
      select("audio_kbps", "Audio bitrate", "auto", [["auto","Smart audio budget"],["64","64 kbps / compact"],["96","96 kbps / balanced"],["128","128 kbps / high"],["192","192 kbps / very high"]]),
      select("preset", "Compression speed", "fast", [["veryfast","Faster"],["fast","Recommended"],["medium","Higher quality / slow"],["slow","Maximum quality / very slow"]]),
    ],
  },
  {
    id: "potatoify", title: "Potatoify", category: "Quality", kind: ["video"], accent: "purple",
    description: "FPS, çözünürlük ve bitrate’i aynı anda bilinçli olarak bozar.", detail: "Düşük kaliteli internet videosu / meme görünümü üretir.",
    fields: [select("profile","Quality profile","decent",[["decent","Decent"],["bad","Bad"],["terrible","Terrible"],["unbearable","Unbearable"],["custom","Custom"],["random","Random"]]), number("fps", "FPS", 12, 1, 120, 1), number("video_badness", "Video badness", 5, 1, 20, 1), number("audio_badness", "Audio badness", 5, 1, 20, 1), number("shrink", "Scale divisor", 4, 1, 20, 1)],
  },
  {
    id: "text", title: "Text", category: "Overlay", kind: ["video"], accent: "blue",
    description: "Videonun üzerine Impact yazı işler.", detail: "Opacity hem yazıya hem kontura uygulanır; 0 görünmez, 100 tam görünürdür.",
    fields: [
      { key:"text", label:"Text", type:"text", value:"CONTAINER" },
      select("position","Position","center",[["top-left","Top left"],["top","Top center"],["top-right","Top right"],["left","Middle left"],["center","Center"],["right","Middle right"],["bottom-left","Bottom left"],["bottom","Bottom center"],["bottom-right","Bottom right"]]),
      { key:"color", label:"Color", type:"text", value:"white", hint:"FFmpeg color name or 0xRRGGBB" },
      number("size","Font size",64,8,600,1,"px"), number("opacity","Opacity",100,0,100,1,"%"),
    ],
  },
  {
    id: "color", title: "Color Adjustment", category: "Effects", kind: ["video"], accent: "blue",
    description: "Kontrast, doygunluk ve parlaklığı birlikte ayarlar.", detail: "1/1/0 görüntüyü değiştirmez. Küçük adımlarla ilerlemek daha güvenlidir.",
    fields: [number("contrast","Contrast",1.08,-2,3,0.01), number("saturation","Saturation",1.10,0,3,0.01), number("brightness","Brightness",0,-1,1,0.01)],
  },
  {
    id: "noise", title: "Visual Noise", category: "Effects", kind: ["video"], accent: "purple",
    description: "Hareketli analog gren/noise ekler.", detail: "720p için 3 güvenli, 6 dengeli bir başlangıçtır.",
    fields: [number("amount","Noise amount",6,1,100,1)],
  },
  { id:"negate", title:"Negate", category:"Effects", kind:["video"], accent:"green", description:"Bütün renkleri negatifine çevirir.", detail:"Fotoğraf negatifi benzeri tek tıklamalı efekttir.", fields:[] },
  {
    id:"deep_fry", title:"Deep Fry", category:"Effects", kind:["video"], accent:"red", description:"Aşırı kontrast, renk, keskinlik, noise ve renk kayması uygular.", detail:"1–2 güvenli; 4 belirgin; 8–10 çok ağırdır.",
    fields:[number("level","Fry level",4,1,10,1)],
  },
  {
    id:"corruption", title:"Video Corruption", category:"Effects", kind:["video"], accent:"red", description:"Kodlanmış video paketlerine kontrollü glitch uygular.", detail:"Çıktı kısmen okunamaz olabilir; kaynak dosyaya dokunulmaz.",
    fields:[number("level","Severity",2,1,10,1)],
  },
  {
    id:"encode", title:"Encoding Engine", category:"Export", kind:["video"], accent:"green", description:"Videoyu seçilen CPU veya donanım codec’iyle yeniden kodlar.", detail:"Donanım encoder’ı sistemde yoksa FFmpeg açık hata döndürür.",
    fields:[
      select("encoder","Encoder","libx264",[["libx264","CPU H.264"],["h264_amf","AMD H.264"],["h264_nvenc","NVIDIA H.264"],["h264_qsv","Intel H.264"],["libx265","CPU HEVC"],["hevc_amf","AMD HEVC"],["hevc_nvenc","NVIDIA HEVC"],["libvpx-vp9","CPU VP9"],["libsvtav1","CPU AV1"],["av1_nvenc","NVIDIA AV1"],["av1_amf","AMD AV1"],["av1_qsv","Intel AV1"]]),
      number("crf","Quality",18,0,40,1,"","Lower means cleaner and larger. The selected encoder uses its own equivalent quality mode."),
      select("pixel_format","Pixel format","auto",[["auto","Auto / preserve when compatible"],["yuv420p","Compatible 8-bit 4:2:0"],["yuv420p10le","10-bit 4:2:0"],["source","Source exactly"]]),
      select("audio_mode","Audio tracks","main",[["main","Main track"],["all","All tracks"],["selected","Selected track"],["merge","Merge all to one"],["none","No audio"]]),
      select("audio_track","Selected audio track","0",[["0","Track 1"]]),
    ],
  },
  {
    id:"proxy", title:"Proxy Creator", category:"Export", kind:["video"], accent:"blue",
    description:"Kurgu sırasında rahat oynatılan hafif bir çalışma kopyası üretir.",
    detail:"Kaynak dosyaya dokunmaz. H.264, hızlı çözme ayarları ve kısa GOP kullanır; bütün ses parçalarını yeniden kodlamadan korur. Auto, kaynak çözünürlüğe göre uygun yüksekliği seçer.",
    fields:[select("resolution","Proxy resolution","auto",[["auto","Auto / recommended"],["540","540p"],["720","720p"],["1080","1080p"]]),select("quality","Proxy quality","edit",[["edit","Editing / high quality"],["compact","Compact / smaller"]])],
  },
  {
    id:"fix_timestamps", title:"Fix Timestamps", category:"Export", kind:["video","audio"], accent:"yellow",
    description:"Bozuk veya negatif zaman damgalarını onarmayı dener.",
    detail:"Fast Repair kaliteyi değiştirmeden akışları kopyalar. Deep Repair daha uyumludur fakat videoyu yeniden kodlar; yalnızca hızlı yöntem yetmezse kullanın.",
    fields:[select("method","Repair method","fast",[["fast","Fast / lossless remux"],["deep","Deep / re-encode"]])],
  },
  {
    id:"file_hash", title:"File Hash", category:"Export", kind:["video","audio","image"], accent:"blue",
    description:"Dosyanın SHA-256 dijital parmak izini hesaplar.",
    detail:"Dosyayı değiştirmez ve yeni dosya üretmez. Aynı içeriğe sahip iki dosyanın SHA-256 değeri de aynıdır; indirme veya kopyanın bozulup bozulmadığını kontrol etmek için kullanılır.",
    fields:[],
  },
  {
    id:"cut", title:"Cut Video", category:"Export", kind:["video"], accent:"green", description:"Yalnızca seçilen zaman aralığını dışa aktarır.", detail:"Başlangıç ve bitiş saniye cinsindendir; orijinal değişmez.",
    fields:[number("start","Start",0,0,86400,0.01,"s"),number("end","End",10,0.01,86400,0.01,"s"),select("cut_mode","Cut mode","exact",[["exact","Exact / re-encode"],["lossless","Lossless / nearest keyframe"]]),number("crf","Quality / CRF",18,0,30,1),select("audio_mode","Audio tracks","main",[["main","Main track"],["all","All tracks"],["selected","Selected track"],["merge","Merge all to one"],["none","No audio"]]),select("audio_track","Selected audio track","0",[["0","Track 1"]])],
  },
  {
    id:"remux", title:"Remux", category:"Export", kind:["video"], accent:"yellow", description:"Yeniden kodlamadan yalnızca kapsayıcıyı değiştirir.", detail:"Çok hızlı ve kayıpsızdır; codec hedef kapsayıcıyla uyumlu olmalıdır.",
    fields:[select("format","Container","mkv",[["mp4","MP4"],["mkv","MKV"],["mov","MOV"]]),select("audio_mode","Audio tracks","all",[["main","Main track"],["all","All tracks"],["selected","Selected track"],["merge","Merge all to one"],["none","No audio"]]),select("audio_track","Selected audio track","0",[["0","Track 1"]])],
  },
  {
    id:"screenshot", title:"Screenshot", category:"Export", kind:["video"], accent:"blue", description:"Seçilen saniyedeki tek kareyi resim olarak kaydeder.", detail:"PNG kayıpsız; JPG küçük; WebP dengelidir.",
    fields:[number("timestamp","Timestamp",0,0,86400,0.01,"s"),select("format","Format","png",[["png","PNG"],["jpg","JPG"],["webp","WebP"]])],
  },
  {
    id:"gif", title:"GIF Maker", category:"Export", kind:["video"], accent:"purple", description:"Videonun bir bölümünü paletli GIF’e dönüştürür.", detail:"Yüksek çözünürlük ve FPS dosyayı çok büyütür.",
    fields:[number("start","Start",0,0,86400,0.01,"s"),number("duration","Duration",5,0.01,86400,0.01,"s"),number("height","Height",480,2,2160,2,"px"),number("fps","FPS",15,1,60,1),select("max_colors","Maximum colors","256",[["32","32 / smallest"],["64","64 / compact"],["128","128 / balanced"],["256","256 / best"]]),select("palette_mode","Palette mode","auto",[["auto","Auto / scene aware"],["single","Single palette"],["multi","Per-frame palette"]]),select("dither","Dithering","balanced",[["balanced","Balanced"],["sharp","Sharp detail"],["small","Smaller file"],["off","Off"]]),select("transparency","Transparency","off",[["off","Off / opaque"],["preserve","Preserve alpha"]]),select("loop","Loop","0",[["0","Infinite"],["-1","Play once"],["2","2 times"],["3","3 times"]])],
  },
  {
    id:"cfr", title:"Convert to CFR", category:"Export", kind:["video"], accent:"blue", description:"Değişken kare hızını sabit kare hızına çevirir.", detail:"Kurgu programlarındaki timeline ve ses senkron sorunlarını azaltır.", fields:[number("fps","Target FPS",30,1,2400,1,"fps")],
  },
  { id:"remove_audio", title:"Remove Audio", category:"Audio", kind:["video"], accent:"red", description:"Videodaki ses akışını kaldırır.", detail:"Görüntü yeniden kodlanmadan kopyalanır.", fields:[] },
  {
    id:"extract_audio", title:"Extract Audio", category:"Audio", kind:["video"], accent:"green", description:"Videonun sesini ayrı dosyaya çıkarır.", detail:"AAC, MP3, WAV, FLAC ve OPUS desteklenir.",
    fields:[select("format","Audio format","mp3",[["copy","Copy original / lossless"],["aac","AAC / M4A"],["mp3","MP3"],["wav","WAV"],["flac","FLAC"],["opus","OPUS"]]),select("audio_mode","Audio tracks","main",[["main","Main track"],["all","All tracks in one MKA"],["selected","Selected track"]]),select("audio_track","Selected audio track","0",[["0","Track 1"]])],
  },
  {
    id:"replace_audio", title:"Replace Audio", category:"Audio", kind:["video"], accent:"yellow", description:"Videonun sesini seçilen başka ses dosyasıyla değiştirir.", detail:"Görüntü kopyalanır; yeni ses AAC olarak kodlanır.",
    fields:[{key:"audio_path",label:"Replacement audio",type:"file",value:"",accept:["mp3","wav","m4a","aac","flac","opus"]}],
  },
  {
    id:"distortion", title:"Simple Distortion", category:"Audio", kind:["video","audio"], accent:"red", description:"Ses frekanslarını aşırı yükselterek clipping/distortion üretir.", detail:"1 güvenli başlangıç, 3 belirgin, 10 çok ağırdır.", fields:[number("level","Severity",3,1,10,1)],
  },
  {
    id:"audio_convert", title:"Convert Audio", category:"Audio", kind:["audio"], accent:"blue", description:"Ses dosyasını başka formata dönüştürür.", detail:"AAC, MP3, WAV, FLAC ve OPUS desteklenir.", fields:[select("format","Output format","mp3",[["aac","AAC / M4A"],["mp3","MP3"],["wav","WAV"],["flac","FLAC"],["opus","OPUS"]])],
  },
  {
    id:"image_ratio", title:"Social Ratio / Crop", category:"Image", kind:["image"], accent:"blue",
    description:"Görseli sosyal medya oranlarına merkezden kırpar.",
    detail:"Görsel esnetilmez veya büyütülmez. PNG piksel verisini kayıpsız korur; JPEG yüksek kaliteli fakat doğası gereği kayıplıdır.",
    fields:[
      select("ratio","Target ratio","1:1",[["1:1","1:1 square · posts"],["4:5","4:5 portrait · Instagram"],["9:16","9:16 · Stories / Reels / TikTok"],["16:9","16:9 landscape · YouTube"],["191:100","1.91:1 landscape · Facebook / X"],["2:3","2:3 portrait · Pinterest"],["3:2","3:2 photo"],["4:3","4:3 classic"]]),
      select("format","Output format","png",[["png","PNG · lossless"],["jpg","JPEG · high quality / smaller"]]),
    ],
  },
  {
    id:"image_potatoify", title:"Image Potatoify", category:"Image", kind:["image"], accent:"purple", description:"Resmi tekrar tekrar JPEG sıkıştırarak bozar.", detail:"Times to compress arttıkça JPEG blokları ve renk kaybı büyür.", fields:[number("quality","Badness",5,1,10,1),number("times","Times to compress",5,1,100,1),number("scale","Scale divisor",2,1,10,1)],
  },
];

export const forKind = (kind: MediaKind) => tools.filter((tool) => tool.kind.includes(kind));
export const cloneTool = (tool: Tool): Tool => ({ ...tool, fields: tool.fields.map((field) => ({ ...field, options: field.options?.map((option) => ({...option})) })) });

const enText: Record<string, [string, string]> = {
  ratio:["Crops the video to the selected aspect ratio from the center.","The image is not stretched. Excess edges are removed evenly from the center."],
  resize:["Changes resolution while preserving aspect ratio.","Choose one edge; the other is calculated automatically as an even number."],
  fps:["Changes the frame rate directly.","It does not create new motion; frames are dropped or repeated."],
  interpolation:["Raises FPS by generating blended intermediate frames.","The target must be above the source FPS, a multiple of 60, and no more than 2400."],
  frame_blend:["Lowers FPS while blending frames.","Motion may gain trails similar to a long-exposure look."],
  dedupe:["Removes frames that are duplicates of nearby frames.","Useful for stuck screen recordings, animation, and repeated frames. Timestamps are preserved so audio does not speed up or lose sync. The output may use variable frame rate."],
  speed:["Speeds up or slows down picture and sound together.","0.5× makes it twice as long; 2× makes it roughly half as long."],
  compression:["Balances quality and file size with CRF.","16 is high quality, 20 balanced, and 24 suits smaller files."],
  smart_quality:["Measures a suitable CRF for this video using short samples.","It samples the beginning, middle, and end; tests CRF 16, 20, 24, and 28; then compares every result with the source using VMAF. It does not render a final video or modify the source."],
  bitrate:["Controls file size using a target video bitrate.","Actual bitrate may vary slightly with scene complexity."],
  discord_compressor:["Compresses a video to fit a Discord upload-size limit.","File size is a fixed budget: longer duration and larger audio leave less room for picture. Smart mode adjusts audio, resolution, and FPS together; two-pass encoding distributes the remaining video budget more efficiently across scenes."],
  potatoify:["Deliberately damages FPS, resolution, and bitrate together.","Creates a low-quality internet video or meme look."],
  text:["Burns Impact text onto the video.","Opacity applies to both text and outline; 0 is invisible and 100 fully visible."],
  color:["Adjusts contrast, saturation, and brightness together.","1/1/0 leaves the image unchanged. Small steps are safer."],
  noise:["Adds animated analogue grain/noise.","For 720p, 3 is safe and 6 is a balanced starting point."],
  negate:["Inverts all colors.","A one-click photographic negative effect."],
  deep_fry:["Applies extreme contrast, color, sharpening, noise, and color shift.","1–2 is safe, 4 is obvious, and 8–10 is very heavy."],
  corruption:["Applies controlled glitches to encoded video packets.","The output may become partly unreadable; the source is never changed."],
  encode:["Re-encodes using the selected CPU or hardware codec.","FFmpeg reports a clear error if the hardware encoder is unavailable."],
  proxy:["Creates a lightweight working copy that plays smoothly while editing.","The source is untouched. It uses H.264, fast decoding settings, and a short GOP, while copying every audio track without re-encoding. Auto chooses a suitable height for the source."],
  fix_timestamps:["Attempts to repair broken or negative timestamps.","Fast Repair copies streams without changing quality. Deep Repair is more compatible but re-encodes video; use it only when the fast method is not enough."],
  file_hash:["Calculates the file's SHA-256 digital fingerprint.","It changes nothing and creates no output file. Identical files have the same SHA-256 value, which is useful for checking whether a download or copy was damaged."],
  cut:["Exports only the selected time range.","Start and end are in seconds; the original remains untouched."],
  remux:["Changes only the container without re-encoding.","Very fast and lossless; codecs must be compatible with the target container."],
  screenshot:["Saves one frame at the selected timestamp.","PNG is lossless, JPG is small, and WebP is balanced."],
  gif:["Turns part of the video into a palette GIF.","High resolution and FPS can make the file very large."],
  cfr:["Converts variable frame rate to constant frame rate.","Reduces timeline and audio-sync problems in editing software."],
  remove_audio:["Removes the audio stream from the video.","The picture is copied without re-encoding."],
  extract_audio:["Extracts video audio to a separate file.","AAC, MP3, WAV, FLAC, and OPUS are supported."],
  replace_audio:["Replaces the video audio with another audio file.","The picture is copied and the new audio is encoded as AAC."],
  distortion:["Boosts audio frequencies to create clipping/distortion.","1 is a safe start, 3 is obvious, and 10 is very heavy."],
  audio_convert:["Converts an audio file to another format.","AAC, MP3, WAV, FLAC, and OPUS are supported."],
  image_ratio:["Crops an image to common social-media aspect ratios.","The image is never stretched or enlarged. PNG preserves pixel data losslessly; JPEG is high quality but inherently lossy."],
  image_potatoify:["Damages an image through repeated JPEG compression.","More compression passes create stronger JPEG blocks and color loss."],
};

const trTitles: Record<string,string> = {ratio:"Oran / Kırp",resize:"Boyutlandır",fps:"FPS Değiştir",interpolation:"FPS İnterpolasyonu",frame_blend:"Kare Harmanlama",dedupe:"Tekrar Eden Kareleri Kaldır",speed:"Video Hızı",compression:"Kalite / Sıkıştırma",smart_quality:"Akıllı Kalite Analizi",bitrate:"Bitrate Kontrolü",discord_compressor:"Discord Sıkıştırıcı",text:"Yazı",color:"Renk Ayarı",noise:"Görsel Gürültü",negate:"Negatif",corruption:"Video Bozma",encode:"Kodlama Motoru",proxy:"Proxy Oluşturucu",fix_timestamps:"Zaman Damgalarını Onar",file_hash:"Dosya Özeti",cut:"Video Kes",screenshot:"Ekran Görüntüsü",gif:"GIF Oluştur",cfr:"CFR'ye Dönüştür",remove_audio:"Sesi Kaldır",extract_audio:"Sesi Çıkar",replace_audio:"Sesi Değiştir",distortion:"Basit Distortion",audio_convert:"Sesi Dönüştür",image_ratio:"Sosyal Medya Oranı / Kırp",image_potatoify:"Görsel Potatoify"};
const trCategories: Record<string,string> = {Transform:"Dönüştürme",Motion:"Hareket",Quality:"Kalite",Overlay:"Kaplama",Effects:"Efektler",Export:"Dışa Aktarma",Audio:"Ses",Image:"Görsel"};
const trFields: Record<string,string> = {"Target ratio":"Hedef oran",Dimension:"Boyut yönü",Pixels:"Piksel","Quality / CRF":"Kalite / CRF","Quality goal":"Kalite hedefi","Sample duration":"Örnek süresi","Target FPS":"Hedef FPS",Multiplier:"Hız çarpanı","Speed mode":"Hız yöntemi",CRF:"CRF","CPU preset":"CPU ön ayarı","Target bitrate":"Hedef bitrate","Discord size limit":"Discord boyut sınırı","Video codec":"Video codec'i","Maximum resolution":"En yüksek çözünürlük","Frame rate limit":"Kare hızı sınırı","Audio bitrate":"Ses bitrate'i","Compression speed":"Sıkıştırma hızı","Video badness":"Video bozulması","Audio badness":"Ses bozulması","Scale divisor":"Ölçek böleni",Text:"Yazı",Position:"Konum",Color:"Renk","Font size":"Yazı boyutu",Opacity:"Opaklık",Contrast:"Kontrast",Saturation:"Doygunluk",Brightness:"Parlaklık","Noise amount":"Gürültü miktarı","Fry level":"Fry seviyesi",Severity:"Şiddet",Encoder:"Kodlayıcı",Quality:"Kalite","Pixel format":"Piksel formatı","Audio tracks":"Ses parçaları","Selected audio track":"Seçili ses parçası",Start:"Başlangıç",End:"Bitiş","Cut mode":"Kesim yöntemi",Container:"Kapsayıcı",Timestamp:"Zaman",Format:"Format",Duration:"Süre",Height:"Yükseklik","Maximum colors":"En fazla renk","Palette mode":"Palet yöntemi",Dithering:"Renk geçişi",Transparency:"Şeffaflık",Loop:"Tekrar","Audio format":"Ses formatı","Replacement audio":"Yeni ses dosyası","Output format":"Çıktı formatı",Badness:"Bozulma","Times to compress":"Sıkıştırma sayısı","Detection profile":"Algılama profili","Proxy resolution":"Proxy çözünürlüğü","Proxy quality":"Proxy kalitesi","Repair method":"Onarım yöntemi"};

export function localizedTool(tool: Tool, language: "tr"|"en"): Tool {
  const copy=cloneTool(tool);
  if(language==="en") {
    const text=enText[copy.id]; if(text){copy.description=text[0];copy.detail=text[1]}
    return copy;
  }
  copy.title=trTitles[copy.id]??copy.title;
  copy.category=trCategories[copy.category]??copy.category;
  const optionNames:Record<string,string>={"1:1 square":"1:1 kare","4:5 portrait":"4:5 dikey","9:16 reels":"9:16 reels","16:9 landscape":"16:9 yatay","4:3 classic":"4:3 klasik","Target height":"Hedef yükseklik","Target width":"Hedef genişlik","Ultra fast":"Çok hızlı","Very fast":"Hızlı",Medium:"Orta","Slow / smaller":"Yavaş / daha küçük","High quality":"Yüksek kalite",Balanced:"Dengeli","Smaller file":"Daha küçük dosya","1 second / faster":"1 saniye / daha hızlı","2 seconds / recommended":"2 saniye / önerilen","3 seconds / more precise":"3 saniye / daha hassas","H.264 / safest":"H.264 / en uyumlu","H.265 / better quality":"H.265 / daha kaliteli","Smart for available bitrate":"Kalan bitrate'e göre akıllı","Smart audio budget":"Akıllı ses bütçesi","64 kbps / compact":"64 kbps / küçük","96 kbps / balanced":"96 kbps / dengeli","128 kbps / high":"128 kbps / yüksek","192 kbps / very high":"192 kbps / çok yüksek",Source:"Kaynak","Up to 1080p":"En fazla 1080p","Up to 720p":"En fazla 720p","Up to 480p":"En fazla 480p","Up to 360p":"En fazla 360p","Up to 240p":"En fazla 240p","Up to 60 FPS":"En fazla 60 FPS","Up to 30 FPS":"En fazla 30 FPS","Up to 24 FPS":"En fazla 24 FPS",Faster:"Daha hızlı",Recommended:"Önerilen","Higher quality / slow":"Daha kaliteli / yavaş","Maximum quality / very slow":"En yüksek kalite / çok yavaş",Fast:"Hızlı","Smallest / slow":"En küçük / yavaş","Top left":"Sol üst","Top center":"Üst orta","Top right":"Sağ üst","Middle left":"Sol orta",Center:"Orta","Middle right":"Sağ orta","Bottom left":"Sol alt","Bottom center":"Alt orta","Bottom right":"Sağ alt"};
  Object.assign(optionNames,{"Safe / recommended":"Güvenli / önerilen","Strong / more frames":"Güçlü / daha fazla kare","Video + audio / synced":"Video + ses / senkron","Lossless video only / no audio":"Kayıpsız yalnız video / sessiz","Auto / recommended":"Otomatik / önerilen","Editing / high quality":"Kurgu / yüksek kalite","Compact / smaller":"Kompakt / daha küçük","Fast / lossless remux":"Hızlı / kayıpsız remux","Deep / re-encode":"Derin / yeniden kodlama","Auto / preserve when compatible":"Otomatik / uyumluysa koru","Compatible 8-bit 4:2:0":"Uyumlu 8-bit 4:2:0","Source exactly":"Kaynağı aynen koru","Main track":"Ana parça","All tracks":"Tüm parçalar","Selected track":"Seçili parça","Merge all to one":"Tümünü tek parçada birleştir","No audio":"Ses yok","Exact / re-encode":"Tam / yeniden kodlama","Lossless / nearest keyframe":"Kayıpsız / en yakın ana kare","Copy original / lossless":"Orijinali kopyala / kayıpsız","All tracks in one MKA":"Tüm parçalar tek MKA","Auto / scene aware":"Otomatik / sahneye duyarlı","Single palette":"Tek palet","Per-frame palette":"Her kareye ayrı palet","Sharp detail":"Keskin detay","Smaller file":"Daha küçük dosya","Off / opaque":"Kapalı / opak","Preserve alpha":"Alfayı koru",Infinite:"Sonsuz","Play once":"Bir kez oynat"});
  Object.assign(optionNames,{"1:1 square · posts":"1:1 kare · gönderiler","4:5 portrait · Instagram":"4:5 dikey · Instagram","9:16 · Stories / Reels / TikTok":"9:16 · Hikâyeler / Reels / TikTok","16:9 landscape · YouTube":"16:9 yatay · YouTube","1.91:1 landscape · Facebook / X":"1.91:1 yatay · Facebook / X","2:3 portrait · Pinterest":"2:3 dikey · Pinterest","3:2 photo":"3:2 fotoğraf","PNG · lossless":"PNG · kayıpsız","JPEG · high quality / smaller":"JPEG · yüksek kalite / daha küçük"});
  Object.assign(optionNames,{Custom:"Özel",Random:"Rastgele"});
  Object.assign(trFields,{"Quality profile":"Kalite profili"});
  copy.fields=copy.fields.map(field=>({...field,label:trFields[field.label]??field.label,options:field.options?.map(option=>({...option,label:optionNames[option.label]??option.label}))}));
  return copy;
}
export const localizedForKind = (kind: MediaKind, language:"tr"|"en") => tools.filter(tool=>tool.kind.includes(kind)).map(tool=>localizedTool(tool,language));
export function localizedForSection(section:MediaKind, source:MediaKind, language:"tr"|"en") {
  return tools.filter(tool=>{
    if(section==="image") return source==="image"&&tool.kind.includes("image");
    if(section==="video") return source==="video"&&tool.kind.includes("video")&&tool.category!=="Audio";
    if(source==="video") return tool.category==="Audio"&&tool.kind.includes("video");
    return source==="audio"&&tool.kind.includes("audio");
  }).map(tool=>localizedTool(tool,language));
}
