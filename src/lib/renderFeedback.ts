export function remainingSeconds(percent:number,elapsed:number):number|null {
  if(!Number.isFinite(percent)||!Number.isFinite(elapsed)||percent<2||percent>=100||elapsed<5)return null;
  return Math.max(1,Math.ceil(elapsed*(100-percent)/percent));
}

export function renderProblem(reason:unknown,language:"tr"|"en"):string|null {
  const raw=String(reason),tr=language==="tr";
  if(/no audio|requires an audio stream/i.test(raw))return tr?"Bu videoda ses yok. Ses içeren bir video açın veya Kaynak geçmişinden önceki videoya dönün.":"This video has no audio. Open a video with audio or return to the previous video in Source history.";
  if(/no space left|disk full|not enough space|disk.*full|os error 112/i.test(raw))return tr?"Diskte yer kalmadı. Çıktı diskinde yer açıp yeniden dene.":"The output disk is full. Free up space and retry.";
  if(/sharing violation|used by another process|being used by|os error 32/i.test(raw))return tr?"Dosya başka bir uygulamada açık. Oynatıcıyı veya dosyayı kullanan uygulamayı kapatıp yeniden dene.":"The file is in use. Close the player or application using it and retry.";
  if(/no such file|file not found|cannot find the file|os error 2\b/i.test(raw))return tr?"Gerekli dosya bulunamadı. Kaynak video, görsel ve fontların taşınmadığını kontrol et; eksik kaynağı yeniden seç.":"A required file is missing. Check that source media, images and fonts have not moved; select the missing source again.";
  if(/permission denied|access.*denied|os error 5\b/i.test(raw))return tr?"Dosyaya erişim engellendi. Klasörün yazma iznini ve dosyanın salt okunur olmadığını kontrol edip yeniden dene.":"File access was denied. Check folder write permissions and read-only files, then retry.";
  if(/invalid data found|error.*decod|corrupt/i.test(raw))return tr?"Medya okunamadı veya bozuk olabilir. Kaynağın oynatıcıda açıldığını kontrol et; gerekirse yeniden indir.":"The media may be damaged or unreadable. Check it in a player and download a fresh copy if needed.";
  return null;
}
