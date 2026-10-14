//! Text, image overlay and subtitle builders.

use crate::*;

pub(crate) async fn build_image_overlay(
    request: &OperationRequest,
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    require_filters(&["overlay"], "Image / Logo Overlay").await?;
    if info.kind != "video" && info.kind != "image" {
        return Err("Image / Logo Overlay requires a video or image file.".into());
    }
    let overlay = PathBuf::from(param(p, "image_path")?);
    if !overlay.is_file() {
        return Err("Choose a valid PNG, JPG or WebP overlay image.".into());
    }
    let overlay_extension = overlay
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["png", "jpg", "jpeg", "webp"].contains(&overlay_extension.as_str()) {
        return Err("Overlay image must be PNG, JPG or WebP.".into());
    }
    let size = check_range(parse_number(p, "size")?, 1.0, 100.0, "Overlay size")?;
    let x = check_range(parse_number(p, "x")?, 0.0, 100.0, "Overlay X")?;
    let y = check_range(parse_number(p, "y")?, 0.0, 100.0, "Overlay Y")?;
    let opacity = check_range(parse_number(p, "opacity")?, 1.0, 100.0, "Overlay opacity")? / 100.0;
    let margin = check_range(
        p.get("margin")
            .map(String::as_str)
            .unwrap_or("3")
            .parse::<f64>()
            .map_err(|_| "Invalid overlay margin.")?,
        0.0,
        25.0,
        "Overlay margin",
    )?;
    let position = p.get("position").map(String::as_str).unwrap_or("custom");
    let (overlay_x, overlay_y) = match position {
        "custom" => (
            format!("main_w*{x}/100-overlay_w/2"),
            format!("main_h*{y}/100-overlay_h/2"),
        ),
        "top_left" => (
            format!("main_w*{margin}/100"),
            format!("main_h*{margin}/100"),
        ),
        "top_right" => (
            format!("main_w-overlay_w-main_w*{margin}/100"),
            format!("main_h*{margin}/100"),
        ),
        "bottom_left" => (
            format!("main_w*{margin}/100"),
            format!("main_h-overlay_h-main_h*{margin}/100"),
        ),
        "bottom_right" => (
            format!("main_w-overlay_w-main_w*{margin}/100"),
            format!("main_h-overlay_h-main_h*{margin}/100"),
        ),
        "center" => ("(main_w-overlay_w)/2".into(), "(main_h-overlay_h)/2".into()),
        _ => return Err("Invalid overlay position.".into()),
    };
    let (display_w, _) = display_dimensions(input, info).await?;
    let target_width = ((display_w as f64 * size / 100.0).round() as u64).max(2);
    *args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        request.input.clone(),
    ];
    if info.kind == "video" {
        args.extend(["-loop".into(), "1".into()]);
    }
    args.extend(["-i".into(), overlay.to_string_lossy().to_string()]);
    let enable = if info.kind == "video" {
        let start = check_range(parse_number(p, "start")?, 0.0, 86400.0, "Start")?;
        let end = check_range(parse_number(p, "end")?, 0.0, 86400.0, "End")?;
        if end <= start {
            return Err("Overlay end time must be later than its start time.".into());
        }
        format!(":enable='between(t,{start},{end})'")
    } else {
        String::new()
    };
    args.extend([
        "-filter_complex".into(),
        format!("[1:v]format=rgba,scale={target_width}:-2,colorchannelmixer=aa={opacity:.4}[logo];[0:v][logo]overlay={overlay_x}:{overlay_y}{enable}:shortest=1[v]"),
        "-map".into(), "[v]".into(),
    ]);
    if info.kind == "image" {
        *extension = image_extension(input);
        args.extend(["-frames:v".into(), "1".into()]);
        append_image_encoder(args, extension, 100)?;
    } else {
        args.extend(["-map".into(), "0:a?".into()]);
        // Overlay edits are intermediate media: retain the composed pixels
        // instead of accumulating another lossy encode on each edit.
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-qp".into(),
            "0".into(),
            "-preset".into(),
            "veryfast".into(),
        ]);
        append_mp4_audio_codec(args, info);
        *extension = "mp4".into();
    }
    Ok(())
}

pub(crate) async fn build_subtitles(
    request: &OperationRequest,
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if info.kind != "video" {
        return Err("Subtitles requires a video file.".into());
    }
    match param(p, "action")? {
        "add" => {
            let subtitle = PathBuf::from(param(p, "subtitle_path")?);
            if !subtitle.is_file() {
                return Err("Choose a valid subtitle file.".into());
            }
            let container = param(p, "container")?;
            *args = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-y".into(),
                "-i".into(),
                request.input.clone(),
                "-i".into(),
                subtitle.to_string_lossy().to_string(),
                "-map".into(),
                "0".into(),
                "-map".into(),
                "1:0".into(),
                "-c:v".into(),
                "copy".into(),
                "-c:a".into(),
                "copy".into(),
            ];
            match container {
                "mkv" => {
                    args.extend(["-c:s".into(), "copy".into()]);
                    *extension = "mkv".into();
                }
                "mp4" => {
                    args.extend(["-c:s".into(), "mov_text".into()]);
                    *extension = "mp4".into();
                }
                _ => return Err("Invalid subtitle output container.".into()),
            }
        }
        "burn" => {
            let subtitle = PathBuf::from(param(p, "subtitle_path")?);
            if !subtitle.is_file() {
                return Err("Choose a valid subtitle file.".into());
            }
            let ext = subtitle
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let filter = if matches!(ext.as_str(), "ass" | "ssa") {
                require_filters(&["ass"], "Subtitle Burn").await?;
                format!("ass=filename='{}'", ffmpeg_filter_path(&subtitle))
            } else {
                require_filters(&["subtitles"], "Subtitle Burn").await?;
                format!("subtitles=filename='{}'", ffmpeg_filter_path(&subtitle))
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
            append_mp4_audio_codec(args, info);
            *extension = "mp4".into();
        }
        "extract" => {
            let index = param(p, "subtitle_track")?
                .parse::<u64>()
                .map_err(|_| "Choose a subtitle track.".to_string())?;
            let tracks = probe_subtitles(request.input.clone()).await?;
            let track = tracks
                .iter()
                .find(|track| track.index == index)
                .ok_or("The selected subtitle track is no longer available.")?;
            let (codec, ext) = match track.codec.as_str() {
                "subrip" | "srt" => ("copy", "srt"),
                "ass" => ("copy", "ass"),
                "ssa" => ("copy", "ssa"),
                "webvtt" => ("copy", "vtt"),
                "mov_text" => ("srt", "srt"),
                "hdmv_pgs_subtitle" => ("copy", "sup"),
                _ => {
                    return Err(format!(
                        "The {} subtitle format cannot be safely extracted as a standalone file.",
                        track.codec
                    ))
                }
            };
            args.extend([
                "-map".into(),
                format!("0:{index}"),
                "-c:s".into(),
                codec.into(),
            ]);
            *extension = ext.into();
        }
        "remove" => {
            args.extend([
                "-map".into(),
                "0".into(),
                "-map".into(),
                "-0:s".into(),
                "-c".into(),
                "copy".into(),
            ]);
            *extension = input
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .unwrap_or("mkv")
                .into();
        }
        _ => return Err("Invalid subtitle action.".into()),
    }
    Ok(())
}

pub(crate) async fn build_text(
    info: &MediaInfo,
    input: &Path,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if info.kind != "video" && info.kind != "image" {
        return Err("Text requires a video or image file.".into());
    }
    let layers: Vec<TextLayer> = serde_json::from_str(param(p, "layers")?)
        .map_err(|error| format!("Invalid text layers: {error}"))?;
    if layers.is_empty() {
        return Err("Add at least one text layer.".into());
    }
    let raster_path = p.get("text_raster_path").map(PathBuf::from);
    let mut text_filters = Vec::with_capacity(layers.len());
    for layer in layers {
        if layer.text.trim().is_empty() {
            return Err("Text layers cannot be empty.".into());
        }
        if raster_path.is_some() {
            continue;
        }
        let size = check_range(layer.size, 8.0, 600.0, "Font size")?;
        let opacity = check_range(layer.opacity, 0.0, 100.0, "Opacity")? / 100.0;
        let x = check_range(layer.x, 0.0, 100.0, "Text X")? / 100.0;
        let y = check_range(layer.y, 0.0, 100.0, "Text Y")? / 100.0;
        let x_expression = match layer.align.as_str() {
            "left" => format!("w*{x:.6}"),
            "center" => format!("w*{x:.6}-text_w/2"),
            "right" => format!("w*{x:.6}-text_w"),
            _ => return Err("Invalid text alignment.".into()),
        };
        let font = if layer.font_path.is_empty() {
            PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
                .join("Fonts")
                .join("impact.ttf")
        } else {
            PathBuf::from(&layer.font_path)
        };
        if !font.is_file() {
            return Err("The selected font is no longer available.".into());
        }
        // CSS text stroke is centered on the glyph edge. drawtext's border expands by
        // the supplied amount on every side, so half keeps preview/export in parity.
        let outline = check_range(layer.outline, 0.0, 20.0, "Outline")? / 2.0;
        let shadow = check_range(layer.shadow, 0.0, 30.0, "Shadow")?;
        let outline_color = if layer.outline_color.is_empty() {
            "#000000"
        } else {
            &layer.outline_color
        };
        let shadow_color = if layer.shadow_color.is_empty() {
            "#000000"
        } else {
            &layer.shadow_color
        };
        let line_spacing = size * 0.05;
        let mut filter = format!("drawtext=fontfile='{}':text='{}':expansion=none:fontcolor='{}':alpha={opacity:.4}:fontsize={size}:line_spacing={line_spacing:.3}:x={x_expression}:y=h*{y:.6}-text_h/2:borderw={outline:.3}:bordercolor='{}':shadowx={shadow:.0}:shadowy={shadow:.0}:shadowcolor='{}'@{:.4}",drawtext_escape(&font.to_string_lossy()),drawtext_escape(&layer.text),drawtext_escape(&layer.color),drawtext_escape(outline_color),drawtext_escape(shadow_color),opacity*0.75);
        if layer.background {
            let background_opacity =
                check_range(layer.background_opacity, 0.0, 100.0, "Background opacity")? / 100.0;
            let background_padding =
                check_range(layer.background_padding, 0.0, 80.0, "Background padding")?;
            let background_color = if layer.background_color.is_empty() {
                "#000000"
            } else {
                &layer.background_color
            };
            filter.push_str(&format!(
                ":box=1:boxcolor='{}'@{background_opacity:.4}:boxborderw={background_padding:.0}",
                drawtext_escape(background_color)
            ));
        }
        text_filters.push(filter);
    }
    if let Some(path) = raster_path {
        require_filters(&["overlay"], "Emoji Text").await?;
        if !path.is_file() {
            return Err("Text raster image is unavailable.".into());
        }
        if info.kind == "video" {
            args.extend(["-loop".into(), "1".into()]);
        }
        args.extend(["-i".into(), path.to_string_lossy().into_owned()]);
        args.extend([
            "-filter_complex".into(),
            "[1:v]format=rgba[txt];[0:v][txt]overlay=0:0:format=auto:shortest=1[v]".into(),
            "-map".into(),
            "[v]".into(),
        ]);
        if info.kind == "video" {
            args.extend(["-map".into(), "0:a?".into()]);
        }
    } else {
        args.extend(["-vf".into(), text_filters.join(",")]);
    }
    if info.kind == "image" {
        *extension = image_extension(input);
        args.extend(["-frames:v".into(), "1".into()]);
        append_image_encoder(args, extension, 100)?;
    } else {
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-qp".into(),
            "0".into(),
            "-preset".into(),
            "veryfast".into(),
        ]);
        append_mp4_audio_codec(args, info);
        *extension = "mp4".into();
    }
    Ok(())
}
