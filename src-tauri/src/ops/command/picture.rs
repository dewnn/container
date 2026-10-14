//! Picture, motion and quality tool builders.

use crate::*;

pub(crate) fn build_transform(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    op: &str,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if info.kind != "video" && info.kind != "image" {
        return Err("Transform requires a video or image file.".into());
    }
    let crop_mode = param(p, "crop_mode")?;
    let vertical_layout = p
        .get("vertical_layout")
        .map(String::as_str)
        .unwrap_or("fill");
    if !["original", "blur", "fill", "split", "squares", "freecam"].contains(&vertical_layout) {
        return Err("Invalid vertical layout.".into());
    }
    let vertical_layout_active = info.kind == "video" && crop_mode == "9:16";
    let fit_mode = p.get("fit_mode").map(String::as_str).unwrap_or("crop");
    if !["crop", "contain"].contains(&fit_mode) {
        return Err("Invalid fit mode.".into());
    }
    let mut filters = Vec::new();
    match param(p, "rotate")? {
        "0" => {}
        "90" => filters.push("transpose=clock".into()),
        "180" => filters.push("hflip,vflip".into()),
        "270" => filters.push("transpose=cclock".into()),
        _ => return Err("Invalid rotation.".into()),
    }
    // Flip after rotation so Horizontal and Vertical always describe
    // the axes visible to the user in the preview.
    if param(p, "flip_h")? == "true" {
        filters.push("hflip".into());
    }
    if param(p, "flip_v")? == "true" {
        filters.push("vflip".into());
    }
    if crop_mode != "off"
        && (info.kind != "image" || fit_mode == "crop")
        && (!vertical_layout_active || vertical_layout == "fill")
    {
        let x = check_range(parse_number(p, "crop_x")?, 0.0, 99.0, "Crop X")?;
        let y = check_range(parse_number(p, "crop_y")?, 0.0, 99.0, "Crop Y")?;
        let w = check_range(parse_number(p, "crop_w")?, 1.0, 100.0, "Crop width")?;
        let h = check_range(parse_number(p, "crop_h")?, 1.0, 100.0, "Crop height")?;
        if x + w > 100.001 || y + h > 100.001 {
            return Err("Crop rectangle must stay inside the video.".into());
        }
        filters.push(format!(
            "crop=trunc(iw*{w}/100/2)*2:trunc(ih*{h}/100/2)*2:trunc(iw*{x}/100/2)*2:trunc(ih*{y}/100/2)*2"
        ));
    }
    if op == "clipper" && vertical_layout_active && vertical_layout == "fill" {
        let (zoom, pan_x, pan_y) = clipper_framing(p)?;
        if zoom > 100.0 {
            let scale = zoom / 100.0;
            filters.push(format!(
                "crop=trunc(iw/{scale:.4}/2)*2:trunc(ih/{scale:.4}/2)*2:trunc((iw-ow)*{pan_x:.2}/100/2)*2:trunc((ih-oh)*{pan_y:.2}/100/2)*2"
            ));
        }
    }
    if info.kind == "image" && fit_mode == "contain" && crop_mode != "off" {
        let (rw, rh) = match crop_mode {
            "1:1" => (1.0, 1.0),
            "4:5" => (4.0, 5.0),
            "5:4" => (5.0, 4.0),
            "3:2" => (3.0, 2.0),
            "2:3" => (2.0, 3.0),
            "16:9" => (16.0, 9.0),
            "9:16" => (9.0, 16.0),
            "4:3" => (4.0, 3.0),
            "3:4" => (3.0, 4.0),
            "191:100" => (191.0, 100.0),
            "free" => return Err("Fit / contain requires a fixed aspect ratio.".into()),
            _ => return Err("Invalid canvas ratio.".into()),
        };
        let rotated = matches!(param(p, "rotate")?, "90" | "270");
        let source_w = if rotated { info.height } else { info.width }.unwrap_or(1);
        let source_h = if rotated { info.width } else { info.height }.unwrap_or(1);
        let target_ratio = rw / rh;
        let (target_w, target_h) = if source_w as f64 / source_h as f64 > target_ratio {
            (source_w, (source_w as f64 / target_ratio).ceil() as u64)
        } else {
            ((source_h as f64 * target_ratio).ceil() as u64, source_h)
        };
        let background = match p
            .get("canvas_background")
            .map(String::as_str)
            .unwrap_or("transparent")
        {
            "transparent" => "0x00000000".into(),
            "black" => "black".into(),
            "white" => "white".into(),
            "custom" => {
                let value = p
                    .get("canvas_color")
                    .map(String::as_str)
                    .unwrap_or("#202020");
                parse_hex_color(value, "Canvas background")?;
                format!("0x{}", value.trim_start_matches('#'))
            }
            _ => return Err("Invalid canvas background.".into()),
        };
        filters.push(format!(
            "format=rgba,pad={target_w}:{target_h}:(ow-iw)/2:(oh-ih)/2:color={background}"
        ));
    }
    match param(p, "size_mode")? {
        "source" => {}
        "height" => {
            let size = check_range(parse_number(p, "size")?, 2.0, 7680.0, "Size")? as u64;
            filters.push(format!("scale=-2:trunc({size}/2)*2:flags=lanczos"));
        }
        "width" => {
            let size = check_range(parse_number(p, "size")?, 2.0, 7680.0, "Size")? as u64;
            filters.push(format!("scale=trunc({size}/2)*2:-2:flags=lanczos"));
        }
        "exact" => {
            let width = check_range(parse_number(p, "output_width")?, 2.0, 7680.0, "Width")? as u64;
            let height =
                check_range(parse_number(p, "output_height")?, 2.0, 7680.0, "Height")? as u64;
            if vertical_layout_active && ["split", "squares", "freecam"].contains(&vertical_layout)
            {
                filters.push(multi_region_layout_filter(
                    p,
                    vertical_layout,
                    width,
                    height,
                    info,
                )?);
            } else if vertical_layout_active && vertical_layout == "original" {
                let (zoom, pan_x, pan_y) = clipper_framing(p)?;
                let scale = zoom / 100.0;
                let band_height = clipper_band_height(width, height, info);
                let background = match p
                    .get("canvas_background")
                    .map(String::as_str)
                    .unwrap_or("black")
                {
                    "black" | "transparent" => "black".into(),
                    "white" => "white".into(),
                    "custom" => {
                        let value = p
                            .get("canvas_color")
                            .map(String::as_str)
                            .unwrap_or("#202020");
                        parse_hex_color(value, "Canvas background")?;
                        format!("0x{}", value.trim_start_matches('#'))
                    }
                    _ => return Err("Invalid canvas background.".into()),
                };
                filters.push(format!(
                    "scale=trunc({width}*{scale:.4}/2)*2:-2:flags=lanczos,crop={width}:min(ih\\,{band_height}):trunc((iw-ow)*{pan_x:.2}/100/2)*2:trunc((ih-oh)*{pan_y:.2}/100/2)*2,pad={width}:{height}:0:(oh-ih)/2:color={background},setsar=1"
                ));
            } else if vertical_layout_active && vertical_layout == "blur" {
                let (zoom, pan_x, pan_y) = clipper_framing(p)?;
                let strength = if p.contains_key("blur_strength") {
                    check_range(
                        parse_number(p, "blur_strength")?,
                        0.0,
                        60.0,
                        "Blur strength",
                    )?
                } else {
                    35.0
                };
                let scale = zoom / 100.0;
                let band_height = clipper_band_height(width, height, info);
                filters.push(format!(
                    "split=2[bg][fg];[bg]scale={width}:{height}:force_original_aspect_ratio=increase:flags=lanczos,crop={width}:{height},gblur=sigma={strength:.2}[bg];[fg]scale=trunc({width}*{scale:.4}/2)*2:-2:flags=lanczos,crop={width}:min(ih\\,{band_height}):trunc((iw-ow)*{pan_x:.2}/100/2)*2:trunc((ih-oh)*{pan_y:.2}/100/2)*2[fg];[bg][fg]overlay=(W-w)/2:(H-h)/2,setsar=1"
                ));
            } else if info.kind == "image" {
                filters.push(format!(
                    "scale={width}:{height}:force_original_aspect_ratio=decrease:flags=lanczos,format=rgba,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=0x00000000"
                ));
            } else {
                filters.push(format!(
                    "scale=trunc({width}/2)*2:trunc({height}/2)*2:flags=lanczos"
                ));
            }
        }
        _ => return Err("Invalid output size mode.".into()),
    }
    if op == "clipper" && vertical_layout_active {
        let width = check_range(parse_number(p, "output_width")?, 2.0, 7680.0, "Width")? as u64;
        let height = check_range(parse_number(p, "output_height")?, 2.0, 7680.0, "Height")? as u64;
        let styled_watermark = enabled(p, "watermark_enabled")
            && p.get("watermark_layer")
                .is_some_and(|value| !value.is_empty());
        if !styled_watermark {
            if let Some(watermark) =
                clipper_watermark_filter(p, vertical_layout, width, height, info)?
            {
                filters.push(watermark);
            }
        }
        if let Some(social_tag) =
            clipper_social_tag_filter(p, vertical_layout, width, height, info)?
        {
            filters.push(social_tag);
        }
        if styled_watermark {
            let path = p
                .get("text_raster_path")
                .map(PathBuf::from)
                .filter(|path| path.is_file())
                .ok_or(
                    "Watermark raster is missing. Open the project in the editor and render again.",
                )?;
            filters.push(format!("null[wm_base];movie=filename={}:dec_threads=1,format=rgba[wm_text];[wm_base][wm_text]overlay=0:0:format=auto:eof_action=repeat:repeatlast=1", movie_filter_path(&path)));
        }
    }
    if op == "clipper" {
        // Filtered social exports must leave with a regular frame cadence.
        // Smart-cut inputs can contain mixed packet time bases around the
        // re-encoded boundary; allowing FFmpeg's implicit vsync here caused
        // dropped frames and hardware-decoder-only green playback on some
        // H.264 files.
        if let Some(fps) = info.fps.filter(|fps| fps.is_finite() && *fps > 0.0) {
            filters.push(format!("fps={fps:.6}"));
        }
    }
    if !filters.is_empty() {
        args.extend(["-vf".into(), filters.join(",")]);
    }
    if info.kind == "image" {
        let output_extension = match param(p, "format")? {
            "png" => "png",
            "jpg" | "jpeg" => "jpg",
            "webp" => "webp",
            "bmp" => "bmp",
            "tif" | "tiff" => "tiff",
            "avif" => "avif",
            _ => return Err("Invalid image output format.".into()),
        };
        if output_extension == "avif" && image_has_alpha(info) {
            return Err("This FFmpeg build cannot safely preserve image transparency in AVIF. Choose PNG or WebP.".into());
        }
        if output_extension == "jpg" {
            filters.push(alpha_flatten_filter(
                p.get("jpeg_background")
                    .map(String::as_str)
                    .unwrap_or("#ffffff"),
            )?);
        }
        if !filters.is_empty() {
            if let Some(position) = args.iter().position(|value| value == "-vf") {
                args[position + 1] = filters.join(",");
            } else {
                args.extend(["-vf".into(), filters.join(",")]);
            }
        }
        args.extend(["-frames:v".into(), "1".into()]);
        append_image_encoder(args, output_extension, 100)?;
        *extension = output_extension.into();
    } else {
        if op == "clipper" {
            // Lossless H.264 is disproportionately large for social-layout exports.
            // CRF 18 remains visually transparent while ultrafast keeps the CPU-heavy
            // crop/scale/composite pipeline moving substantially faster.
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-profile:v".into(),
                "high".into(),
                "-crf".into(),
                "18".into(),
                "-preset".into(),
                "ultrafast".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
        } else {
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-qp".into(),
                "0".into(),
                "-preset".into(),
                "veryfast".into(),
            ]);
        }
        append_mp4_audio_codec(args, info);
        *extension = "mp4".into();
    }
    Ok(())
}

pub(crate) async fn build_upscale(
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if info.kind != "video" {
        return Err("Upscale requires a video file.".into());
    }
    let (width, height) = display_dimensions(input, info).await?;
    let target_edge =
        check_range(parse_number(p, "target_edge")?, 720.0, 4320.0, "Resolution")? as u64;
    if target_edge <= width.min(height) {
        return Err("Upscale target must be larger than the source resolution.".into());
    }
    let target_long =
        (width.max(height) as f64 / width.min(height) as f64 * target_edge as f64).ceil();
    if target_long > 7680.0 {
        return Err("Upscale output would exceed the supported 7680-pixel limit.".into());
    }
    let filter = if width >= height {
        format!("scale=-2:trunc({target_edge}/2)*2:flags=lanczos,setsar=1")
    } else {
        format!("scale=trunc({target_edge}/2)*2:-2:flags=lanczos,setsar=1")
    };
    let (_, encoder_args) = fastest_h264_encoder().await;
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "0:a?".into(),
        "-vf".into(),
        filter,
    ]);
    args.extend(encoder_args);
    let audio_copy_safe = info
        .audio_tracks
        .iter()
        .all(|track| ["aac", "mp3", "ac3", "eac3", "alac"].contains(&track.codec.as_str()));
    if info.audio_codec.is_some() {
        if audio_copy_safe {
            args.extend(["-c:a".into(), "copy".into()]);
        } else {
            args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
        }
    }
    args.extend(["-movflags".into(), "+faststart".into()]);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_resize(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let size = check_range(parse_number(p, "size")?, 2.0, 7680.0, "Size")? as u64;
    let crf = check_range(parse_number(p, "crf")?, 0.0, 30.0, "CRF")?;
    let filter = if param(p, "axis")? == "width" {
        format!("scale=trunc({size}/2)*2:-2")
    } else {
        format!("scale=-2:trunc({size}/2)*2")
    };
    args.extend([
        "-vf".into(),
        filter,
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        crf.to_string(),
        "-preset".into(),
        "veryfast".into(),
    ]);
    append_mp4_audio_codec(args, info);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_fps(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let fps = check_range(parse_number(p, "fps")?, 1.0, 2400.0, "FPS")?;
    let crf = p
        .get("crf")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(16.0);
    args.extend([
        "-vf".into(),
        format!("fps={fps}"),
        "-fps_mode".into(),
        "cfr".into(),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        crf.to_string(),
        "-preset".into(),
        "veryfast".into(),
    ]);
    append_mp4_audio_codec(args, info);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_interpolation(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let fps = check_range(parse_number(p, "fps")?, 60.0, 2400.0, "FPS")?;
    if info.fps.is_some_and(|current| fps <= current)
        || (fps / 60.0 - (fps / 60.0).round()).abs() > 1e-8
    {
        return Err("Interpolation FPS must be above input FPS and a multiple of 60.".into());
    }
    args.extend([
        "-vf".into(),
        format!("minterpolate=fps={fps}:mi_mode=blend"),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        "16".into(),
        "-preset".into(),
        "ultrafast".into(),
    ]);
    append_mp4_audio_codec(args, info);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_frame_blend(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let fps = check_range(parse_number(p, "fps")?, 1.0, 2399.0, "FPS")?;
    let current = info.fps.ok_or("Input FPS is unknown.")?;
    if fps >= current {
        return Err("Frame Blending target must be below input FPS.".into());
    }
    let frames = ((current / fps).round() as u64).clamp(2, 128);
    args.extend([
        "-vf".into(),
        format!("tmix=frames={frames},fps={fps}"),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        "16".into(),
        "-preset".into(),
        "veryfast".into(),
    ]);
    append_mp4_audio_codec(args, info);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_speed(
    request: &OperationRequest,
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let speed = check_range(parse_number(p, "speed")?, 0.05, 100.0, "Speed")?;
    let mode = p.get("speed_mode").map(String::as_str).unwrap_or("synced");
    if mode == "lossless_video" {
        *args = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-y".into(),
            "-itsscale".into(),
            format!("{:.10}", 1.0 / speed),
            "-i".into(),
            request.input.clone(),
            "-map".into(),
            "0:v:0".into(),
            "-c:v".into(),
            "copy".into(),
            "-an".into(),
        ];
        *extension = "mkv".into();
    } else if mode == "synced" {
        let crf = check_range(parse_number(p, "crf")?, 0.0, 30.0, "CRF")?;
        args.extend(["-filter:v".into(), format!("setpts=PTS/{speed}")]);
        if info.audio_codec.is_some() {
            args.extend([
                "-filter:a".into(),
                atempo(speed),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
        }
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            crf.to_string(),
            "-preset".into(),
            "veryfast".into(),
        ]);
        *extension = "mp4".into();
    } else {
        return Err("Invalid speed mode.".into());
    }
    Ok(())
}

pub(crate) fn build_compression(
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let mode = p.get("mode").map(String::as_str).unwrap_or("crf");
    if mode == "bitrate" {
        let mbps = check_range(parse_number(p, "mbps")?, 0.05, 500.0, "Bitrate")?;
        let kbps = (mbps * 1000.0).round() as u64;
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "veryfast".into(),
            "-b:v".into(),
            format!("{kbps}k"),
            "-maxrate".into(),
            format!("{}k", (kbps as f64 * 1.35) as u64),
            "-bufsize".into(),
            format!("{}k", kbps * 2),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
        ]);
    } else if mode == "crf" {
        let crf = check_range(parse_number(p, "crf")?, 0.0, 30.0, "CRF")?;
        let preset = param(p, "preset")?;
        if !["ultrafast", "veryfast", "medium", "slow"].contains(&preset) {
            return Err("Invalid preset.".into());
        }
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            crf.to_string(),
            "-preset".into(),
            preset.into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
        ]);
    } else {
        return Err("Invalid compression mode.".into());
    }
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_potatoify(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let profile = p.get("profile").map(String::as_str).unwrap_or("custom");
    let (fps, vb, ab, shrink) = match profile {
        "decent" => (24.0, 3.0, 2.0, 2.0),
        "bad" => (18.0, 6.0, 5.0, 3.0),
        "terrible" => (12.0, 11.0, 10.0, 5.0),
        "unbearable" => (6.0, 18.0, 18.0, 10.0),
        "random" => {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as u64;
            (
                (5 + seed % 26) as f64,
                (3 + (seed / 29) % 18) as f64,
                (3 + (seed / 53) % 18) as f64,
                (2 + (seed / 97) % 11) as f64,
            )
        }
        "custom" => (
            check_range(parse_number(p, "fps")?, 1.0, 120.0, "FPS")?,
            check_range(
                parse_number(p, "video_badness")?,
                1.0,
                20.0,
                "Video badness",
            )?,
            check_range(
                parse_number(p, "audio_badness")?,
                1.0,
                20.0,
                "Audio badness",
            )?,
            check_range(parse_number(p, "shrink")?, 1.0, 20.0, "Scale")?,
        ),
        _ => return Err("Invalid Potatoify quality profile.".into()),
    };
    let w = ((info.width.unwrap_or(1280) as f64 / shrink) as u64 / 2 * 2).max(2);
    let h = ((info.height.unwrap_or(720) as f64 / shrink) as u64 / 2 * 2).max(2);
    let bitrate = ((w * h) as f64 * fps / vb).max(1000.0) as u64;
    args.extend([
        "-vf".into(),
        format!("scale={w}:{h}:flags=neighbor,fps={fps}"),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "ultrafast".into(),
        "-b:v".into(),
        bitrate.to_string(),
        "-maxrate".into(),
        bitrate.to_string(),
        "-bufsize".into(),
        (bitrate * 2).to_string(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        format!("{}k", (256.0 / ab).clamp(16.0, 192.0) as u64),
    ]);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) async fn build_blur_pixelate(
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    require_filters(&["overlay", "boxblur"], "Blur / Pixelate").await?;
    if info.kind != "video" && info.kind != "image" {
        return Err("Blur / Pixelate requires a video or image file.".into());
    }
    let x = check_range(parse_number(p, "region_x")?, 0.0, 99.0, "Region X")?;
    let y = check_range(parse_number(p, "region_y")?, 0.0, 99.0, "Region Y")?;
    let w = check_range(parse_number(p, "region_w")?, 1.0, 100.0, "Region width")?;
    let h = check_range(parse_number(p, "region_h")?, 1.0, 100.0, "Region height")?;
    if x + w > 100.001 || y + h > 100.001 {
        return Err("Effect rectangle must stay inside the video.".into());
    }
    let strength = check_range(parse_number(p, "strength")?, 2.0, 60.0, "Strength")?;
    let (source_w, source_h) = display_dimensions(input, info).await?;
    let region_w = (((source_w as f64 * w / 100.0).floor() as u64) / 2 * 2).max(2);
    let region_h = (((source_h as f64 * h / 100.0).floor() as u64) / 2 * 2).max(2);
    let region_x = ((((source_w as f64 * x / 100.0).floor() as u64) / 2 * 2)
        .min(source_w.saturating_sub(region_w)))
        / 2
        * 2;
    let region_y = ((((source_h as f64 * y / 100.0).floor() as u64) / 2 * 2)
        .min(source_h.saturating_sub(region_h)))
        / 2
        * 2;
    let crop = format!("crop={region_w}:{region_h}:{region_x}:{region_y}");
    let effect = match param(p, "effect")? {
        "blur" => format!(
            "boxblur=luma_radius={}:luma_power=2",
            (strength / 2.0)
                .max(1.0)
                .min(region_w.min(region_h) as f64 / 2.0)
        ),
        "pixelate" => {
            let divisor = (strength / 2.0).round().max(2.0);
            let pixel_w = ((region_w as f64 / divisor).floor() as u64).max(1);
            let pixel_h = ((region_h as f64 / divisor).floor() as u64).max(1);
            format!("scale={pixel_w}:{pixel_h}:flags=neighbor,scale={region_w}:{region_h}:flags=neighbor")
        }
        _ => return Err("Invalid effect mode.".into()),
    };
    args.extend([
        "-filter_complex".into(),
        format!("[0:v]split[base][area];[area]{crop},{effect}[fx];[base][fx]overlay={region_x}:{region_y}[v]"),
        "-map".into(), "[v]".into(), "-map".into(), "0:a?".into(),
    ]);
    if info.kind == "image" {
        *extension = image_extension(input);
        args.extend(["-frames:v".into(), "1".into()]);
        append_image_encoder(args, extension, 100)?;
    } else {
        let (_, encoder_args) = fastest_h264_encoder().await;
        args.extend(encoder_args);
        append_mp4_audio_codec(args, info);
        *extension = "mp4".into();
    }
    Ok(())
}

pub(crate) fn build_color(
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if info.kind != "video" && info.kind != "image" {
        return Err("Color Adjustment requires a video or image file.".into());
    }
    let mut filters = Vec::new();
    let mut eq = Vec::new();
    if enabled(p, "brightness_enabled") {
        eq.push(format!(
            "brightness={}",
            check_range(parse_number(p, "brightness")?, -100.0, 100.0, "Brightness")? / 100.0
        ));
    }
    if enabled(p, "contrast_enabled") {
        eq.push(format!(
            "contrast={}",
            check_range(parse_number(p, "contrast")?, 0.0, 200.0, "Contrast")? / 100.0
        ));
    }
    if enabled(p, "saturation_enabled") {
        eq.push(format!(
            "saturation={}",
            check_range(parse_number(p, "saturation")?, 0.0, 200.0, "Saturation")? / 100.0
        ));
    }
    if enabled(p, "gamma_enabled") {
        eq.push(format!(
            "gamma={}",
            check_range(parse_number(p, "gamma")?, 10.0, 300.0, "Gamma")? / 100.0
        ));
    }
    if !eq.is_empty() {
        filters.push(format!("eq={}", eq.join(":")));
    }
    if enabled(p, "hue_enabled") {
        filters.push(format!(
            "hue=h={}",
            check_range(parse_number(p, "hue")?, -180.0, 180.0, "Hue")?
        ));
    }
    if enabled(p, "temperature_enabled") {
        filters.push(format!(
            "colortemperature=temperature={}",
            check_range(
                parse_number(p, "temperature")?,
                1000.0,
                40000.0,
                "Temperature"
            )?
        ));
    }
    if enabled(p, "sharpen_enabled") {
        filters.push(format!(
            "unsharp=5:5:{:.3}:5:5:0",
            check_range(parse_number(p, "sharpen")?, 0.0, 100.0, "Sharpen")? * 0.02
        ));
    }
    if enabled(p, "blur_enabled") {
        filters.push(format!(
            "gblur=sigma={:.3}",
            check_range(parse_number(p, "blur")?, 0.0, 100.0, "Blur")? / 25.0
        ));
    }
    match p.get("denoise").map(String::as_str).unwrap_or("off") {
        "off" => {}
        "low" => filters.push("hqdn3d=1.5:1.5:6:6".into()),
        "medium" => filters.push("hqdn3d=3:3:8:8".into()),
        "high" => filters.push("hqdn3d=6:6:12:12".into()),
        _ => return Err("Invalid denoise mode.".into()),
    }
    if enabled(p, "deband_enabled") {
        let value = check_range(parse_number(p, "deband")?, 0.0, 100.0, "Deband")? / 1250.0;
        filters.push(format!(
            "deband=1thr={value:.5}:2thr={value:.5}:3thr={value:.5}:4thr={value:.5}"
        ));
    }
    if enabled(p, "vignette_enabled") {
        let value = check_range(parse_number(p, "vignette")?, 0.0, 100.0, "Vignette")? / 100.0;
        filters.push(format!("vignette=angle=PI/4*{value:.4}"));
    }
    if p.get("grayscale").is_some_and(|value| value == "on") {
        filters.push("hue=s=0".into());
    }
    match p.get("deinterlace").map(String::as_str).unwrap_or("off") {
        "off" => {}
        "auto" => filters.push("yadif=deint=interlaced".into()),
        "on" => filters.push("yadif=deint=all".into()),
        _ => return Err("Invalid interlace mode.".into()),
    }
    if filters.is_empty() {
        return Err("Enable at least one video filter.".into());
    }
    args.extend(["-vf".into(), filters.join(",")]);
    if info.kind == "image" {
        *extension = image_extension(input);
        args.extend(["-frames:v".into(), "1".into()]);
        append_image_encoder(args, extension, 100)?;
    } else {
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            "16".into(),
            "-preset".into(),
            "veryfast".into(),
        ]);
        append_mp4_audio_codec(args, info);
        *extension = "mp4".into();
    }
    Ok(())
}

pub(crate) fn build_noise(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let value = check_range(parse_number(p, "amount")?, 1.0, 100.0, "Noise")?;
    args.extend([
        "-vf".into(),
        format!("noise=alls={value}:allf=t+u"),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        "16".into(),
        "-preset".into(),
        "veryfast".into(),
    ]);
    append_mp4_audio_codec(args, info);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_negate(
    info: &MediaInfo,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    args.extend([
        "-vf".into(),
        "negate".into(),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        "16".into(),
        "-preset".into(),
        "veryfast".into(),
    ]);
    append_mp4_audio_codec(args, info);
    *extension = "mp4".into();
    Ok(())
}
