// Turns raw errors (backend messages, FFmpeg and yt-dlp output) into short,
// actionable text in the current interface language.
import { renderProblem } from "./renderFeedback";
import { turkishBackendError } from "./backendErrors";

export function friendlyProblem(reason:unknown,language:"tr"|"en"):string{
  const raw=String(reason??"").trim();
  const renderHint=renderProblem(raw,language);if(renderHint)return renderHint;
  if(language==="tr"){
    if(/valid HTTPS|video link/i.test(raw))return "Geçerli bir HTTPS video bağlantısı girip tekrar dene.";
    if(/yt-dlp.*not ready|yt-dlp gerekli/i.test(raw))return "İndirme bileşeni hazır değil. Önce resmî yt-dlp.exe dosyasını seç.";
    if(/ffmpeg.*(?:not found|missing|could not start)/i.test(raw))return "FFmpeg bileşeni bulunamadı. Uygulamayı yeniden kurup tekrar dene.";
    if(/font|yazı tipi/i.test(raw))return `Yazı tipi önizlemesi yüklenemedi: ${raw}`;
    if(/network|connect|timed? out|internet/i.test(raw))return "Bağlantı kurulamadı. İnternet bağlantını kontrol edip tekrar dene.";
    if(/permission|access denied/i.test(raw))return "Dosyaya erişilemedi. Klasör izinlerini kontrol edip tekrar dene.";
    if(/download failed/i.test(raw))return "İndirme tamamlanamadı. Bağlantıyı veya seçilen formatı kontrol et.";
    if(/sign in|login required|confirm you.?re not a bot/i.test(raw))return "Bu video giriş doğrulaması istiyor ve şu anda indirilemiyor.";
    if(/HTTP Error 40[134]|forbidden/i.test(raw))return "Kaynak indirmeye izin vermedi. Biraz sonra veya başka bir formatla tekrar dene.";
    if(/video unavailable|private video/i.test(raw))return "Bu video kullanılamıyor veya özel olarak ayarlanmış.";
    if(/format.*(?:unavailable|not available|unsupported)/i.test(raw))return "Seçilen format bu video için kullanılamıyor. Başka bir kalite seç.";
    return turkishBackendError(raw)??(raw||"Beklenmeyen bir sorun oluştu. Lütfen tekrar dene.");
  }
  if(/valid HTTPS|video link/i.test(raw))return "Enter a valid HTTPS video link and try again.";
  if(/yt-dlp.*not ready/i.test(raw))return "The download component is not ready. Select the official yt-dlp executable first.";
  if(/ffmpeg.*(?:not found|missing|could not start)/i.test(raw))return "FFmpeg is unavailable. Reinstall the application and try again.";
  if(/font/i.test(raw))return `The font preview could not be loaded: ${raw}`;
  if(/network|connect|timed? out|internet/i.test(raw))return "Could not connect. Check your internet connection and try again.";
  if(/permission|access denied/i.test(raw))return "The file could not be accessed. Check the folder permissions and try again.";
  if(/download failed/i.test(raw))return "The download could not be completed. Check the link or selected format.";
  if(/sign in|login required|confirm you.?re not a bot/i.test(raw))return "This video requires sign-in verification and cannot currently be downloaded.";
  if(/HTTP Error 40[134]|forbidden/i.test(raw))return "The source refused the download. Try again later or choose another format.";
  if(/video unavailable|private video/i.test(raw))return "This video is unavailable or private.";
  if(/format.*(?:unavailable|not available|unsupported)/i.test(raw))return "That format is unavailable for this video. Choose another quality.";
  return raw||"An unexpected problem occurred. Please try again.";
}
