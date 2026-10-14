//! Export and container tool builders.

use crate::*;

pub(crate) fn build_encode(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let encoder = param(p, "encoder")?;
    if ![
        "libx264",
        "h264_amf",
        "h264_nvenc",
        "h264_qsv",
        "libx265",
        "hevc_amf",
        "hevc_nvenc",
        "libvpx-vp9",
        "libsvtav1",
        "av1_nvenc",
        "av1_amf",
        "av1_qsv",
    ]
    .contains(&encoder)
    {
        return Err("Unsupported encoder.".into());
    }
    let quality = check_range(parse_number(p, "crf")?, 0.0, 40.0, "Quality")?;
    let requested_pixel_format = p.get("pixel_format").map(String::as_str).unwrap_or("auto");
    let source_pixel_format = info.pixel_format.as_deref().unwrap_or("yuv420p");
    let ten_bit_source =
        source_pixel_format.contains("10") || info.bits_per_raw_sample.is_some_and(|bits| bits > 8);
    let pixel_format = match requested_pixel_format {
        "yuv420p" | "yuv420p10le" => requested_pixel_format,
        "source" => source_pixel_format,
        "auto"
            if ten_bit_source
                && (encoder.contains("265")
                    || encoder.contains("hevc")
                    || encoder.contains("vp9")
                    || encoder.contains("av1")) =>
        {
            "yuv420p10le"
        }
        "auto" => "yuv420p",
        _ => return Err("Invalid pixel format.".into()),
    };
    let hardware_h264 = ["h264_amf", "h264_nvenc", "h264_qsv"].contains(&encoder);
    if hardware_h264 && pixel_format != "yuv420p" && pixel_format != "nv12" {
        return Err("The selected H.264 hardware encoder cannot safely produce this pixel format. Choose Compatible 8-bit 4:2:0 or use CPU/HEVC/AV1.".into());
    }
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-c:v".into(),
        encoder.into(),
        "-pix_fmt".into(),
        pixel_format.into(),
    ]);
    match encoder {
        "libx264" | "libx265" => args.extend([
            "-preset".into(),
            "veryfast".into(),
            "-crf".into(),
            quality.to_string(),
        ]),
        "libvpx-vp9" => args.extend([
            "-crf".into(),
            quality.to_string(),
            "-b:v".into(),
            "0".into(),
            "-row-mt".into(),
            "1".into(),
            "-deadline".into(),
            "good".into(),
            "-cpu-used".into(),
            "4".into(),
        ]),
        "libsvtav1" => args.extend([
            "-crf".into(),
            quality.to_string(),
            "-preset".into(),
            "8".into(),
        ]),
        "h264_nvenc" | "hevc_nvenc" | "av1_nvenc" => args.extend([
            "-rc".into(),
            "vbr".into(),
            "-cq".into(),
            quality.to_string(),
            "-b:v".into(),
            "0".into(),
        ]),
        "h264_amf" | "hevc_amf" | "av1_amf" => args.extend([
            "-rc".into(),
            "cqp".into(),
            "-qp_i".into(),
            quality.to_string(),
            "-qp_p".into(),
            quality.to_string(),
        ]),
        "h264_qsv" | "av1_qsv" => args.extend(["-global_quality".into(), quality.to_string()]),
        _ => return Err("Unsupported encoder quality mode.".into()),
    }
    append_audio_routing(args, info, p, true, "main")?;
    *extension = if encoder.contains("vp9") || encoder.contains("av1") {
        "mkv"
    } else {
        "mp4"
    }
    .into();
    Ok(())
}

pub(crate) async fn build_proxy(
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let requested_height = match param(p, "resolution")? {
        "auto" => match info.height.unwrap_or(1080) {
            0..=719 => 540,
            720..=1439 => 720,
            _ => 1080,
        },
        "540" => 540,
        "720" => 720,
        "1080" => 1080,
        _ => return Err("Invalid proxy resolution.".into()),
    };
    let crf = match param(p, "quality")? {
        "edit" => "16",
        "compact" => "22",
        _ => return Err("Invalid proxy quality.".into()),
    };
    let (source_width, source_height) = display_dimensions(input, info).await?;
    let height = requested_height.min(source_height).max(2) / 2 * 2;
    let width = (((source_width as f64 * height as f64 / source_height as f64).round() as u64) / 2
        * 2)
    .max(2);
    let scale_flags = if discord_is_low_complexity(info) {
        "neighbor"
    } else {
        "lanczos"
    };
    let fps = info.fps.unwrap_or(30.0).clamp(1.0, 120.0);
    let gop = (fps * 2.0).round().max(12.0) as u64;
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "0:a?".into(),
        "-vf".into(),
        format!("scale={width}:{height}:flags={scale_flags}"),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-tune".into(),
        "fastdecode".into(),
        "-crf".into(),
        crf.into(),
        "-g".into(),
        gop.to_string(),
        "-keyint_min".into(),
        gop.to_string(),
        "-sc_threshold".into(),
        "0".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "copy".into(),
    ]);
    // Matroska accepts copied AAC, Opus, AC-3, FLAC and other common source
    // tracks, so keeping the original audio does not depend on MP4 support.
    *extension = "mkv".into();
    Ok(())
}

pub(crate) fn build_fix_timestamps(
    request: &OperationRequest,
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let method = param(p, "method")?;
    *args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-fflags".into(),
        "+genpts+discardcorrupt".into(),
        "-i".into(),
        request.input.clone(),
        "-avoid_negative_ts".into(),
        "make_zero".into(),
    ];
    match method {
        "fast" => {
            args.extend(["-map".into(), "0".into(), "-c".into(), "copy".into()]);
            *extension = input
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .unwrap_or(if info.kind == "audio" { "mka" } else { "mkv" })
                .to_ascii_lowercase();
        }
        "deep" if info.kind == "video" => {
            args.extend([
                "-map".into(),
                "0:v:0".into(),
                "-map".into(),
                "0:a?".into(),
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "veryfast".into(),
                "-crf".into(),
                "18".into(),
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
            ]);
            *extension = "mp4".into();
        }
        "deep" => {
            args.extend(["-map".into(), "0:a?".into(), "-c:a".into(), "flac".into()]);
            *extension = "mka".into();
        }
        _ => return Err("Invalid timestamp repair method.".into()),
    }
    Ok(())
}

pub(crate) fn build_cut(
    request: &OperationRequest,
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let start = check_range(
        parse_number(p, "start")?,
        0.0,
        info.duration.unwrap_or(86400.0),
        "Start",
    )?;
    let end = check_cut_end(parse_number(p, "end")?, info.duration.unwrap_or(86400.0))?;
    if end <= start {
        return Err("End must be greater than start.".into());
    }
    let cut_mode = p.get("cut_mode").map(String::as_str).unwrap_or("lossless");
    *args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
    ];
    match cut_mode {
        "exact" => args.extend([
            "-i".into(),
            request.input.clone(),
            "-ss".into(),
            format!("{start:.6}"),
        ]),
        "lossless" | "smart" => args.extend([
            "-ss".into(),
            format!("{start:.6}"),
            "-i".into(),
            request.input.clone(),
        ]),
        _ => return Err("Invalid cut mode.".into()),
    }
    args.extend([
        "-t".into(),
        format!("{:.6}", end - start),
        "-map".into(),
        "0:v:0".into(),
    ]);
    match cut_mode {
        "exact" => {
            let crf = p
                .get("crf")
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(18.0);
            check_range(crf, 0.0, 30.0, "CRF")?;
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "veryfast".into(),
                "-crf".into(),
                crf.to_string(),
            ]);
            append_audio_routing(args, info, p, true, "main")?;
            *extension = "mp4".into();
        }
        "lossless" | "smart" => {
            args.extend(["-c:v".into(), "copy".into()]);
            append_audio_routing(args, info, p, false, "main")?;
            args.extend(["-avoid_negative_ts".into(), "make_zero".into()]);
            *extension = input
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .unwrap_or("mkv")
                .to_ascii_lowercase();
        }
        _ => unreachable!("cut mode was validated before building arguments"),
    }
    Ok(())
}

pub(crate) fn build_remux(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let format = param(p, "format")?;
    if !["mp4", "mkv", "mov"].contains(&format) {
        return Err("Invalid container.".into());
    }
    args.extend(["-map".into(), "0:v?".into(), "-c:v".into(), "copy".into()]);
    if format == "mkv" {
        args.extend([
            "-map".into(),
            "0:s?".into(),
            "-map".into(),
            "0:t?".into(),
            "-map".into(),
            "0:d?".into(),
            "-c:s".into(),
            "copy".into(),
            "-c:d".into(),
            "copy".into(),
        ]);
    } else {
        // MP4/MOV cannot carry arbitrary attachments/data. Text subtitles
        // are converted to the container-native mov_text format.
        args.extend([
            "-map".into(),
            "0:s?".into(),
            "-c:s".into(),
            "mov_text".into(),
        ]);
    }
    args.extend([
        "-map_metadata".into(),
        "0".into(),
        "-map_chapters".into(),
        "0".into(),
    ]);
    append_audio_routing(args, info, p, false, "all")?;
    *extension = format.into();
    Ok(())
}

pub(crate) fn build_screenshot(
    request: &OperationRequest,
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let ts = check_range(
        parse_number(p, "timestamp")?,
        0.0,
        info.duration.unwrap_or(86400.0),
        "Timestamp",
    )?;
    let format = param(p, "format")?;
    if !["png", "jpg", "webp"].contains(&format) {
        return Err("Invalid image format.".into());
    }
    *args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-ss".into(),
        format!("{ts:.6}"),
        "-i".into(),
        request.input.clone(),
        "-frames:v".into(),
        "1".into(),
    ];
    if format == "jpg" {
        args.extend(["-q:v".into(), "2".into()]);
    }
    if format == "webp" {
        args.extend([
            "-c:v".into(),
            "libwebp".into(),
            "-quality".into(),
            "95".into(),
        ]);
    }
    *extension = format.into();
    Ok(())
}

pub(crate) async fn build_gif(
    request: &OperationRequest,
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let start = check_range(
        parse_number(p, "start")?,
        0.0,
        info.duration.unwrap_or(86400.0),
        "Start",
    )?;
    let end = check_range(
        parse_number(p, "end")?,
        0.01,
        info.duration.unwrap_or(86400.0),
        "End",
    )?;
    if end <= start {
        return Err("GIF end time must be later than its start time.".into());
    }
    let duration = end - start;
    if info.duration.is_some_and(|total| end > total + 0.01) {
        return Err("GIF range exceeds video duration.".into());
    }
    let height = check_range(parse_number(p, "height")?, 2.0, 2160.0, "Height")? as u64;
    let fps = check_range(parse_number(p, "fps")?, 1.0, 60.0, "FPS")?;
    let (display_width, display_height) = display_dimensions(input, info).await?;
    let width = (height as f64 * display_width as f64 / display_height as f64).round() as u64;
    let width = width.div_ceil(2) * 2;
    let max_colors = p
        .get("max_colors")
        .map(String::as_str)
        .unwrap_or("256")
        .parse::<u64>()
        .map_err(|_| "Invalid GIF color count.")?;
    if ![32, 64, 128, 256].contains(&max_colors) {
        return Err("GIF colors must be 32, 64, 128, or 256.".into());
    }
    let palette_mode = p.get("palette_mode").map(String::as_str).unwrap_or("auto");
    let stats_mode = match palette_mode {
        "auto" => "diff",
        "single" => "full",
        "multi" => "single",
        _ => return Err("Invalid GIF palette mode.".into()),
    };
    let dither = match p.get("dither").map(String::as_str).unwrap_or("balanced") {
        "balanced" => "sierra2_4a",
        "sharp" => "floyd_steinberg",
        "small" => "bayer:bayer_scale=5",
        "off" => "none",
        _ => return Err("Invalid GIF dithering mode.".into()),
    };
    let reserve_transparent = match p.get("transparency").map(String::as_str).unwrap_or("off") {
        "off" => 0,
        "preserve" => 1,
        _ => return Err("Invalid GIF transparency mode.".into()),
    };
    let palette_new = if palette_mode == "multi" {
        ":new=1"
    } else {
        ""
    };
    let filter = format!(
        "fps={fps},scale={width}:{height}:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode={stats_mode}:max_colors={max_colors}:reserve_transparent={reserve_transparent}[p];[s1][p]paletteuse=dither={dither}{palette_new}"
    );
    let loop_count = p.get("loop").map(String::as_str).unwrap_or("0");
    if !["-1", "0", "2", "3"].contains(&loop_count) {
        return Err("Invalid GIF loop count.".into());
    }
    *args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-ss".into(),
        format!("{start:.6}"),
        "-i".into(),
        request.input.clone(),
        "-t".into(),
        format!("{duration:.6}"),
        "-filter_complex".into(),
        filter,
        "-loop".into(),
        loop_count.into(),
    ];
    *extension = "gif".into();
    Ok(())
}
