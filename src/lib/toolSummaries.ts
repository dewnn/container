import { tools } from "./tools";

// Use the canonical category so colors do not change with the interface language.
export function toolCategory(id: string) {
  return tools.find(tool => tool.id === id)?.category ?? "Utilities";
}

// Sidebar-only copy; full descriptions remain available in the settings panel.
const summaries: Record<string, [string, string]> = {
  transform: ["Crop, rotate and resize", "Kırp, döndür ve boyutlandır"],
  clipper: ["Create vertical clips", "Dikey klipler oluştur"],
  upscale: ["Increase resolution", "Çözünürlüğü artır"],
  fps: ["Change frame rate", "Kare hızını değiştir"],
  interpolation: ["Add frames for smoother motion", "Ara karelerle akıcılığı artır"],
  frame_blend: ["Blend frames at lower FPS", "FPS düşürürken kareleri harmanla"],
  speed: ["Speed up or slow down", "Hızlandır veya yavaşlat"],
  stabilizer: ["Reduce camera shake", "Kamera sarsıntısını azalt"],
  compression: ["Reduce video file size", "Video dosyasını küçült"],
  discord_compressor: ["Fit a file-size limit", "Dosya boyutu sınırına sığdır"],
  potatoify: ["Add low-quality effects", "Düşük kalite efekti ekle"],
  text: ["Add text layers", "Yazı katmanları ekle"],
  image_overlay: ["Add an image or logo", "Görsel veya logo ekle"],
  color: ["Adjust colors and lighting", "Renk ve ışığı ayarla"],
  noise: ["Add grain and noise", "Gren ve gürültü ekle"],
  blur_pixelate: ["Blur or pixelate a region", "Bir bölgeyi bulanıklaştır"],
  encode: ["Choose a CPU or GPU encoder", "CPU veya GPU ile kodla"],
  proxy: ["Create a lightweight editing copy", "Hafif bir kurgu kopyası oluştur"],
  merge_videos: ["Join clips into one video", "Klipleri tek videoda birleştir"],
  subtitles: ["Add or extract subtitles", "Altyazı ekle veya çıkar"],
  fix_timestamps: ["Repair media timestamps", "Medya zaman damgalarını düzelt"],
  file_hash: ["Calculate a SHA-256 checksum", "SHA-256 dosya özeti hesapla"],
  cut: ["Export a selected time range", "Seçili zaman aralığını çıkar"],
  remux: ["Change container without encoding", "Kodlamadan kapsayıcı değiştir"],
  screenshot: ["Save a video frame", "Bir video karesini kaydet"],
  gif: ["Turn a clip into a GIF", "Klipten GIF oluştur"],
  remove_audio: ["Remove the audio track", "Ses parçasını kaldır"],
  extract_audio: ["Save audio separately", "Sesi ayrı kaydet"],
  replace_audio: ["Use a different audio track", "Ses parçasını değiştir"],
  distortion: ["Add audio distortion", "Sese distortion ekle"],
  audio_convert: ["Change audio format", "Ses formatını değiştir"],
  image_compressor: ["Reduce image file size", "Görsel dosyasını küçült"],
  metadata_cleaner: ["Remove embedded metadata", "Gömülü metaverileri temizle"],
  image_potatoify: ["Add JPEG degradation", "JPEG bozulma efekti ekle"],
};

export function toolSummary(tool: {id: string; description: string}, language: "tr" | "en") {
  return summaries[tool.id]?.[language === "tr" ? 1 : 0] ?? tool.description;
}
