# CONTAINER — Türkçe

CONTAINER; video, ses ve görselleri yerel olarak işleyen Windows için bir FFmpeg araç kutusudur. Dosyalar sunucuya yüklenmez ve kaynak dosya değiştirilmez.

![CONTAINER Toolbox](container-toolbox.png)

## Ana özellikler

- Dosya türüne göre ayrılan Video, Audio ve Image araçları
- Kırpma, yeniden boyutlandırma, FPS, interpolasyon ve frame blending
- Kalite, bitrate ve hedef dosya boyutuna göre sıkıştırma
- Metin, renk, noise, distortion ve yaratıcı efektler
- Timeline üzerinden kesme, ekran görüntüsü ve GIF seçimi
- Ses çıkarma, değiştirme ve kaldırma
- Toplu işleme
- Donanım codec algılama ve güvenli CPU fallback
- TR/EN arayüz ve açık/koyu tema

## SmartCut

SmartCut, videodaki konuşmayı cihaz üzerinde algılar ve sessiz kısımları düzenlenebilir bölgeler halinde gösterir. Kesimleri önizleyebilir; MP4 veya FCPXML olarak dışa aktarabilirsin. Kamera ve harici ses kayıtları, varsa gömülü timecode bilgisiyle hizalanır.

## İndirme

GitHub deposundaki **Releases** bölümünde iki Windows paketi bulunur:

- `CONTAINER-Setup-<sürüm>-x64.exe`: önerilen kurulum dosyası
- `CONTAINER-Portable-<sürüm>-x64.zip`: taşınabilir uygulama ve FFmpeg dosyaları

Uygulama henüz kod imzalı değildir. Bu nedenle Windows SmartScreen “Bilinmeyen yayıncı” uyarısı gösterebilir.

## Geliştirme

Node.js 22+, pnpm 10+, Rust stable, Visual Studio C++ Build Tools ve PATH içinde FFmpeg/FFprobe gerekir.

```powershell
pnpm install --frozen-lockfile
pnpm check
pnpm build
pnpm tauri dev
```

## Çıktılar

Her işlem, seçilen dosyanın yanında `CONTAINER Output/<kategori>` klasörüne yeni bir dosya oluşturur. Kaynak dosya hiçbir zaman üzerine yazılmaz.

## Lisans durumu

SmartCut yaklaşımı [cobanov/autocut](https://github.com/cobanov/autocut) projesinden esinlenmiştir. Autocut deposunda şu anda bir lisans dosyası bulunmadığı için, CONTAINER'a OSI lisansı eklenmeden önce türetilmiş kısımlar için yazılı izin alınmalı veya bu kısımlar bağımsız biçimde yeniden uygulanmalıdır. Ayrıntı: [OPEN_SOURCE_REVIEW.md](../OPEN_SOURCE_REVIEW.md).

Geliştirici: **dewn** — Codex ile vibe-coded.
