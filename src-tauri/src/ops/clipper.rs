//! Clipper layouts, watermark, Social Tag filters and camera detection.

use crate::*;

pub(crate) fn transform_region(
    params: &HashMap<String, String>,
    prefix: &str,
) -> Result<(f64, f64, f64, f64), String> {
    let x = check_range(
        parse_number(params, &format!("{prefix}_x"))?,
        0.0,
        99.0,
        "Region X",
    )?;
    let y = check_range(
        parse_number(params, &format!("{prefix}_y"))?,
        0.0,
        99.0,
        "Region Y",
    )?;
    let w = check_range(
        parse_number(params, &format!("{prefix}_w"))?,
        1.0,
        100.0,
        "Region width",
    )?;
    let h = check_range(
        parse_number(params, &format!("{prefix}_h"))?,
        1.0,
        100.0,
        "Region height",
    )?;
    if x + w > 100.001 || y + h > 100.001 {
        return Err("Transform regions must stay inside the video.".into());
    }
    Ok((x, y, w, h))
}

pub(crate) fn transform_region_crop(
    input: &str,
    output: &str,
    region: (f64, f64, f64, f64),
    width: u64,
    height: u64,
) -> String {
    let (x, y, w, h) = region;
    format!("[{input}]crop=trunc(iw*{w}/100/2)*2:trunc(ih*{h}/100/2)*2:trunc(iw*{x}/100/2)*2:trunc(ih*{y}/100/2)*2,scale={width}:{height}:force_original_aspect_ratio=increase:flags=lanczos,crop={width}:{height}[{output}]")
}

pub(crate) fn clipper_framing(params: &HashMap<String, String>) -> Result<(f64, f64, f64), String> {
    let read = |key, default, low, high, label| -> Result<f64, String> {
        if params.contains_key(key) {
            check_range(parse_number(params, key)?, low, high, label)
        } else {
            Ok(default)
        }
    };
    Ok((
        read("clipper_zoom", 100.0, 100.0, 300.0, "Clipper zoom")?,
        read("clipper_x", 50.0, 0.0, 100.0, "Clipper X")?,
        read("clipper_y", 50.0, 0.0, 100.0, "Clipper Y")?,
    ))
}

pub(crate) fn clipper_band_height(width: u64, height: u64, info: &MediaInfo) -> u64 {
    let source_width = info.width.unwrap_or(16).max(1) as f64;
    let source_height = info.height.unwrap_or(9) as f64;
    ((width as f64 * source_height / source_width).floor() as u64 / 2 * 2).clamp(2, height)
}

pub(crate) fn multi_region_layout_filter(
    params: &HashMap<String, String>,
    layout: &str,
    width: u64,
    height: u64,
    info: &MediaInfo,
) -> Result<String, String> {
    let camera = transform_region(params, "region_a")?;
    let content = transform_region(params, "region_b")?;
    match layout {
        "split" => {
            if height < 4 {
                // Two even-height bands need at least 2 px each; smaller values
                // would make the clamp below panic.
                return Err("Output height is too small for the Split layout.".into());
            }
            let top_percent = check_range(
                parse_number(params, "region_a_height")?,
                20.0,
                80.0,
                "Top region height",
            )?;
            let top_height =
                ((height as f64 * top_percent / 100.0).round() as u64 / 2 * 2).clamp(2, height - 2);
            let bottom_height = height - top_height;
            let order = params
                .get("region_order")
                .map(String::as_str)
                .unwrap_or("a_first");
            let (top, bottom) = match order {
                "a_first" => (camera, content),
                "b_first" => (content, camera),
                _ => return Err("Invalid region order.".into()),
            };
            Ok(format!(
                "split=2[ra][rb];{};{};[top][bottom]vstack=inputs=2,setsar=1",
                transform_region_crop("ra", "top", top, width, top_height),
                transform_region_crop("rb", "bottom", bottom, width, bottom_height)
            ))
        }
        "squares" => {
            let half_height = height / 2 / 2 * 2;
            let lower_height = height - half_height;
            Ok(format!(
                "split=2[ra][rb];{};{};[camera][content]vstack=inputs=2,setsar=1",
                transform_region_crop("ra", "camera", camera, width, half_height),
                transform_region_crop("rb", "content", content, width, lower_height)
            ))
        }
        "freecam" => {
            let size = check_range(
                parse_number(params, "freecam_size")?,
                15.0,
                90.0,
                "Camera size",
            )?;
            let x = check_range(parse_number(params, "freecam_x")?, 0.0, 100.0, "Camera X")?;
            let y = check_range(parse_number(params, "freecam_y")?, 0.0, 100.0, "Camera Y")?;
            let camera_width = ((width as f64 * size / 100.0).round() as u64 / 2 * 2).max(2);
            let (cx, cy, cw, ch) = camera;
            let source_width =
                ((info.width.unwrap_or(16).max(2) as f64 * cw / 100.0).trunc() as u64 / 2 * 2)
                    .max(2);
            let source_height =
                ((info.height.unwrap_or(9).max(2) as f64 * ch / 100.0).trunc() as u64 / 2 * 2)
                    .max(2);
            let camera_height = ((camera_width as f64 * source_height as f64 / source_width as f64)
                .round() as u64
                / 2
                * 2)
            .max(2);
            // A very thin, tall camera region can produce an absurd overlay
            // height (and a multi-GB mask allocation). Reject it up front.
            if camera_height > 16_384 {
                return Err(
                    "Camera region is too narrow for this Freecam size. Widen the camera region."
                        .into(),
                );
            }
            let mask = rounded_camera_mask(camera_width as u32, camera_height as u32)?;
            Ok(format!("split=2[ra][rb];{};[ra]crop=trunc(iw*{cw}/100/2)*2:trunc(ih*{ch}/100/2)*2:trunc(iw*{cx}/100/2)*2:trunc(ih*{cy}/100/2)*2,scale={camera_width}:{camera_height}:flags=lanczos,format=rgba[camera_raw];movie=filename={}:dec_threads=1,format=gray[round_mask];[camera_raw][round_mask]alphamerge[camera];[content][camera]overlay=(W-w)*{x}/100:(H-h)*{y}/100,setsar=1", transform_region_crop("rb", "content", content, width, height), movie_filter_path(&mask)))
        }
        _ => Err("Invalid multi-region layout.".into()),
    }
}

pub(crate) fn rounded_camera_mask(width: u32, height: u32) -> Result<PathBuf, String> {
    use image::ImageEncoder;
    let radius = (width as f64 * 0.025).round().max(2.0);
    let mask = image::GrayImage::from_fn(width, height, |x, y| {
        let dx = (radius - (x.min(width - 1 - x) as f64)).max(0.0);
        let dy = (radius - (y.min(height - 1 - y) as f64)).max(0.0);
        image::Luma([((radius + 0.5 - dx.hypot(dy)) * 255.0).clamp(0.0, 255.0) as u8])
    });
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(mask.as_raw(), width, height, image::ColorType::L8.into())
        .map_err(|error| error.to_string())?;
    social_tag_asset_path_in(&app_cache_directory()?.join("camera-masks"), &bytes, "png")
}

pub(crate) fn clipper_watermark_filter(
    params: &HashMap<String, String>,
    layout: &str,
    width: u64,
    height: u64,
    info: &MediaInfo,
) -> Result<Option<String>, String> {
    if !enabled(params, "watermark_enabled") {
        return Ok(None);
    }
    let text = params
        .get("watermark_text")
        .map(|value| value.trim())
        .unwrap_or_default();
    if text.is_empty() {
        return Err("Watermark text cannot be empty when watermark is enabled.".into());
    }
    if text.chars().count() > 64 || text.chars().any(char::is_control) {
        return Err("Watermark text must be a single line of at most 64 characters.".into());
    }
    let size = check_range(
        parse_number(params, "watermark_size")?,
        18.0,
        160.0,
        "Watermark size",
    )?;
    let opacity = check_range(
        parse_number(params, "watermark_opacity")?,
        10.0,
        100.0,
        "Watermark opacity",
    )? / 100.0;
    let windows = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let font_dir = PathBuf::from(windows).join("Fonts");
    let font = ["arialbd.ttf", "arial.ttf"]
        .into_iter()
        .map(|name| font_dir.join(name))
        .find(|path| path.is_file())
        .ok_or("A Windows Arial font is required for the Clipper watermark.")?;

    let safe_top = height as f64 * 0.08;
    let safe_bottom = height as f64 * 0.78;
    let bar_height = (size * 1.5).round().max(12.0);
    let safe_low = safe_top + bar_height / 2.0;
    let safe_high = (safe_bottom - bar_height / 2.0).max(safe_low);
    let background_enabled = enabled(params, "watermark_background")
        && matches!(layout, "split" | "squares" | "freecam");
    let (x, y, background) = match layout {
        "split" => {
            let seam = height as f64
                * check_range(
                    parse_number(params, "region_a_height")?,
                    20.0,
                    80.0,
                    "Top region height",
                )?
                / 100.0;
            let center = seam.clamp(safe_low, safe_high);
            (
                "(w-text_w)/2".into(),
                format!("{center:.3}-text_h/2"),
                background_enabled.then(|| {
                    format!(
                        "drawbox=x=0:y={:.3}:w=iw:h={bar_height:.0}:color=black:t=fill",
                        center - bar_height / 2.0
                    )
                }),
            )
        }
        "squares" => {
            let center = (height as f64 / 2.0).clamp(safe_low, safe_high);
            (
                "(w-text_w)/2".into(),
                format!("{center:.3}-text_h/2"),
                background_enabled.then(|| {
                    format!(
                        "drawbox=x=0:y={:.3}:w=iw:h={bar_height:.0}:color=black:t=fill",
                        center - bar_height / 2.0
                    )
                }),
            )
        }
        "freecam" => {
            let camera = transform_region(params, "region_a")?;
            let size_percent = check_range(
                parse_number(params, "freecam_size")?,
                15.0,
                90.0,
                "Camera size",
            )?;
            let output_x = check_range(parse_number(params, "freecam_x")?, 0.0, 100.0, "Camera X")?;
            let output_y = check_range(parse_number(params, "freecam_y")?, 0.0, 100.0, "Camera Y")?;
            let camera_width =
                ((width as f64 * size_percent / 100.0).round() as u64 / 2 * 2).max(2);
            let source_width = (info.width.unwrap_or(16) as f64 * camera.2 / 200.0)
                .floor()
                .max(1.0)
                * 2.0;
            let source_height = (info.height.unwrap_or(9) as f64 * camera.3 / 200.0)
                .floor()
                .max(1.0)
                * 2.0;
            let camera_height =
                (camera_width as f64 * source_height / source_width.max(1.0)).max(2.0);
            let left = (width as f64 - camera_width as f64).max(0.0) * output_x / 100.0;
            let top = (height as f64 - camera_height).max(0.0) * output_y / 100.0;
            let center = (top + camera_height).clamp(safe_low, safe_high);
            (
                format!(
                    "max(w*0.10\\,min({:.3}-text_w/2\\,w*0.90-text_w))",
                    left + camera_width as f64 / 2.0
                ),
                format!("{center:.3}-text_h/2"),
                background_enabled.then(|| {
                    format!(
                        "drawbox=x={left:.3}:y={:.3}:w={camera_width}:h={bar_height:.0}:color=black:t=fill",
                        center - bar_height / 2.0
                    )
                }),
            )
        }
        _ => ("(w-text_w)/2".into(), "h*0.78-text_h".into(), None),
    };
    let border_opacity = opacity * 0.9;
    let shadow_opacity = opacity * 0.8;
    let background = background
        .map(|filter| format!("{filter},"))
        .unwrap_or_default();
    Ok(Some(format!(
        "{background}drawtext=fontfile='{}':text='{}':expansion=none:fontcolor=white@{opacity:.3}:fontsize={size:.0}:x={x}:y={y}:borderw=3:bordercolor=black@{border_opacity:.3}:shadowx=2:shadowy=2:shadowcolor=black@{shadow_opacity:.3}",
        drawtext_escape(&font.to_string_lossy()),
        drawtext_escape(text)
    )))
}

// Keep the originals inside the executable: export must not depend on files on
// the developer's Desktop or on an SVG decoder in the user's FFmpeg build.
pub(crate) fn social_tag_mark_path(platform: &str, style: &str) -> Result<PathBuf, String> {
    social_tag_mark_path_in(&app_cache_directory()?.join("social-tags"), platform, style)
}

pub(crate) fn social_tag_mark_path_in(
    directory: &Path,
    platform: &str,
    style: &str,
) -> Result<PathBuf, String> {
    let bytes: &[u8] = match (platform, style) {
        ("twitch", _) => include_bytes!("../../resources/social-tags/twitch.png"),
        (_, "boxed") => include_bytes!("../../resources/social-tags/kick-boxed.png"),
        _ => include_bytes!("../../resources/social-tags/kick-plain.png"),
    };
    social_tag_asset_path_in(directory, bytes, "png")
}

pub(crate) fn social_tag_asset_path(bytes: &[u8], extension: &str) -> Result<PathBuf, String> {
    social_tag_asset_path_in(
        &app_cache_directory()?.join("social-tags"),
        bytes,
        extension,
    )
}

pub(crate) fn social_tag_asset_path_in(
    directory: &Path,
    bytes: &[u8],
    extension: &str,
) -> Result<PathBuf, String> {
    static CACHE_LOCK: Mutex<()> = Mutex::new(());
    let _guard = CACHE_LOCK.lock().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let path = directory.join(format!("{:x}.{extension}", Sha256::digest(bytes)));
    if std::fs::read(&path).ok().as_deref() != Some(bytes) {
        // Publish a complete file, including when DEV and stable export together.
        let pending = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&pending, bytes).map_err(|e| e.to_string())?;
        if let Err(error) = std::fs::rename(&pending, &path) {
            let _ = std::fs::remove_file(&pending);
            if std::fs::read(&path).ok().as_deref() != Some(bytes) {
                return Err(error.to_string());
            }
        }
    }
    Ok(path)
}

pub(crate) struct KickBannerPlacement {
    pub(crate) width: u64,
    pub(crate) height: u64,
    pub(crate) camera_left: f64,
    pub(crate) camera_width: f64,
    pub(crate) camera_bottom: f64,
    pub(crate) seam: f64,
    pub(crate) camera_below: bool,
    pub(crate) seam_offset: f64,
}

pub(crate) fn clipper_kick_banner_filter(
    params: &HashMap<String, String>,
    username: &str,
    requested: f64,
    placement: KickBannerPlacement,
) -> Result<String, String> {
    let text = username.to_uppercase();
    let estimated_units: f64 = text
        .chars()
        .map(|letter| match letter {
            'I' | '1' | '.' | ',' | ':' | '!' | '|' => 0.29,
            'M' | 'W' | '@' => 0.68,
            _ => 0.5,
        })
        .sum();
    let units = if params.contains_key("social_tag_text_units") {
        check_range(
            parse_number(params, "social_tag_text_units")?,
            0.1,
            100.0,
            "Social Tag text width",
        )?
    } else {
        estimated_units
    };
    // Preserve the reference font size for ordinary names. Move a longer
    // wordmark left before shrinking it against the logo and phone action rail.
    let factor = (requested / 54.0).min(1.0);
    let base = placement.camera_width / 1080.0;
    let logo_scale = base * factor * 1.035;
    let logo_width = (350.0 * logo_scale).round().max(1.0);
    let logo_x = if placement.camera_width < placement.width as f64 {
        placement.camera_left.ceil()
    } else {
        placement.camera_left - 31.5 * base * factor
    };
    let min_prefix_x = logo_x + 350.0 * logo_scale + 18.0 * base * factor;
    let reference_prefix_x = placement.camera_left + 516.0 * base * factor;
    let safe_right = placement.camera_left + placement.camera_width * 0.78;
    let full_text_width = base * factor * 1.18 * (147.0 + 32.0 * units);
    let text_fit = ((safe_right - min_prefix_x) / full_text_width).clamp(0.1, 1.0);
    let text_scale = base * factor * 1.18 * text_fit;
    let prefix_x = (safe_right - full_text_width * text_fit)
        .min(reference_prefix_x)
        .max(min_prefix_x);
    let strip_height = 113.0 * logo_scale;
    let max_offset = (placement.height as f64 * 0.22).min(if placement.camera_below {
        placement.seam
    } else {
        placement.height as f64 - placement.camera_bottom
    });
    let offset = max_offset.max(0.0) * placement.seam_offset / 100.0;
    let y = if placement.camera_below {
        placement.seam - offset
    } else {
        placement.camera_bottom - strip_height + offset
    };
    let bottom = y + strip_height;
    let bar_y = bottom - 69.0 * base * factor * 1.18;
    let logo_height = (101.0 * logo_scale).round().max(1.0);
    let logo_y = bottom - 113.0 * logo_scale;
    let prefix_width = (147.0 * text_scale).round().max(1.0);
    // The supplied prefix tile has black padding below its glyphs. Crop it so
    // the tile cannot protrude beneath the banner after scaling.
    let prefix_height = (94.0 * text_scale).round().max(1.0);
    let bar_width = placement.camera_width.round().max(1.0);
    let bar_height = (69.0 * base * factor * 1.18).round().max(1.0);
    let prefix_y = bottom - 106.0 * text_scale;
    // Clip the prefix tile at the black bar's top edge. Its opaque black
    // padding can otherwise leave a one-pixel lip above the bar after rounding.
    let prefix_clip = (bar_y.round() - prefix_y.round())
        .max(0.0)
        .min(prefix_height - 1.0);
    let prefix_visible_height = prefix_height - prefix_clip;
    let prefix_overlay_y = prefix_y.round() + prefix_clip;
    let art = social_tag_asset_path(
        include_bytes!("../../resources/social-tags/kick-banner.png"),
        "png",
    )?;
    let font = social_tag_asset_path(
        include_bytes!("../../resources/social-tags/gotham-xnarrow-black.otf"),
        "otf",
    )?;
    let text_x = prefix_x + 147.0 * text_scale;
    let text_center_y = bottom - 35.0 * base * factor * 1.18;
    let text_size = (32.0 * text_scale).round().max(1.0);
    // drawbox uses limited-range YUV black (Y=16). Convert pixels before drawing;
    // merely relabeling full-range frames makes the bar gray beside RGB artwork.
    let bar_x = placement
        .camera_left
        .round()
        .clamp(0.0, placement.width as f64);
    Ok(format!(
        "scale=in_range=auto:out_range=tv,format=yuv444p,setparams=range=limited,drawbox=x={bar_x:.0}:y={bar_y:.0}:w={bar_width:.0}:h={bar_height:.0}:color=black:t=fill[social_barred];movie=filename={}:dec_threads=1,crop=1080:101:0:979,format=rgba,premultiply=inplace=1,split=2[social_logo_source][social_prefix_source];[social_logo_source]crop=350:101:0:0,scale={logo_width:.0}:{logo_height:.0}:flags=lanczos,unpremultiply=inplace=1,format=rgba[social_logo];[social_prefix_source]crop=147:94:419:0,scale={prefix_width:.0}:{prefix_height:.0}:flags=lanczos,unpremultiply=inplace=1,format=rgba,crop={prefix_width:.0}:{prefix_visible_height:.0}:0:{prefix_clip:.0}[social_prefix];[social_barred][social_logo]overlay=x={logo_x:.0}:y={logo_y:.0}:format=yuv444:eof_action=repeat:repeatlast=1[social_logoed];[social_logoed][social_prefix]overlay=x={prefix_x:.0}:y={prefix_overlay_y:.0}:format=yuv444:eof_action=repeat:repeatlast=1,drawtext=fontfile='{}':text='{}':expansion=none:fontcolor=white:fontsize={text_size:.0}:x={text_x:.0}:y={text_center_y:.0}-text_h/2",
        movie_filter_path(&art),
        drawtext_escape(&font.to_string_lossy()),
        drawtext_escape(&text),
    ))
}

pub(crate) fn clipper_social_tag_filter(
    params: &HashMap<String, String>,
    layout: &str,
    width: u64,
    height: u64,
    info: &MediaInfo,
) -> Result<Option<String>, String> {
    if !enabled(params, "social_tag_enabled") {
        return Ok(None);
    }
    let username = params
        .get("social_tag_username")
        .map(|value| value.trim())
        .unwrap_or_default();
    let length = username.chars().count();
    if length == 0 || length > 32 || username.chars().any(char::is_control) {
        return Err("Social Tag username must be one line of 1 to 32 characters.".into());
    }
    let style = params
        .get("social_tag_style")
        .map(String::as_str)
        .unwrap_or("boxed");
    if !matches!(style, "boxed" | "plain" | "kick_banner") {
        return Err("Invalid Social Tag style.".into());
    }
    let position = params
        .get(if style == "boxed" {
            "social_tag_boxed_position"
        } else {
            "social_tag_plain_position"
        })
        .map(String::as_str)
        .unwrap_or("center");
    if !matches!(position, "left" | "center" | "right") {
        return Err("Invalid Social Tag position.".into());
    }
    // Older projects may still contain a side position for boxed badges.
    let position = if style == "boxed" { "center" } else { position };
    let platform = params
        .get("social_tag_platform")
        .map(String::as_str)
        .unwrap_or("kick");
    if !matches!(platform, "kick" | "twitch") {
        return Err("Invalid Social Tag platform.".into());
    }
    if style == "kick_banner" && platform != "kick" {
        return Err("The Kick.com banner requires the Kick platform.".into());
    }
    let requested = check_range(
        parse_number(params, "social_tag_size")?,
        20.0,
        96.0,
        "Social Tag size",
    )?;
    let estimated_units: f64 = username
        .chars()
        .map(|letter| match letter {
            'i' | 'l' | 'I' | '1' | '.' | ',' | ':' | '!' | '|' => 0.35,
            'm' | 'w' | 'M' | 'W' | '@' => 0.9,
            'A'..='Z' => 0.72,
            _ => 0.59,
        })
        .sum();
    let measured_units = params.contains_key("social_tag_text_units");
    let units = if measured_units {
        check_range(
            parse_number(params, "social_tag_text_units")?,
            0.1,
            100.0,
            "Social Tag text width",
        )?
    } else {
        estimated_units
    };
    let mut camera_left = 0.0;
    let mut camera_top = 0.0;
    let mut camera_width = width as f64;
    let mut camera_bottom = height as f64;
    let mut seam = height as f64 * 0.78;
    match layout {
        "split" => {
            let top_percent = check_range(
                parse_number(params, "region_a_height")?,
                20.0,
                80.0,
                "Top region height",
            )?;
            seam = ((height as f64 * top_percent / 100.0).round() as u64 / 2 * 2) as f64;
            if params.get("region_order").map(String::as_str) == Some("b_first") {
                camera_top = seam;
                camera_bottom = height as f64;
            } else {
                camera_bottom = seam;
            }
        }
        "squares" => {
            seam = (height / 2 / 2 * 2) as f64;
            camera_bottom = seam;
        }
        "freecam" => {
            let camera = transform_region(params, "region_a")?;
            let percent = check_range(
                parse_number(params, "freecam_size")?,
                15.0,
                90.0,
                "Camera size",
            )?;
            let x = check_range(parse_number(params, "freecam_x")?, 0.0, 100.0, "Camera X")?;
            let y = check_range(parse_number(params, "freecam_y")?, 0.0, 100.0, "Camera Y")?;
            camera_width = ((width as f64 * percent / 100.0).round() as u64 / 2 * 2) as f64;
            let source_width = (info.width.unwrap_or(16) as f64 * camera.2 / 200.0)
                .floor()
                .max(1.0)
                * 2.0;
            let source_height = (info.height.unwrap_or(9) as f64 * camera.3 / 200.0)
                .floor()
                .max(1.0)
                * 2.0;
            let camera_height =
                ((camera_width * source_height / source_width.max(1.0)).round() as u64 / 2 * 2)
                    .max(2) as f64;
            camera_left = (width as f64 - camera_width).max(0.0) * x / 100.0;
            camera_top = (height as f64 - camera_height).max(0.0) * y / 100.0;
            camera_bottom = camera_top + camera_height;
            seam = camera_bottom;
        }
        "original" | "blur" => {
            let band_height = clipper_band_height(width, height, info) as f64;
            camera_bottom = (height as f64 + band_height) / 2.0;
            seam = camera_bottom;
        }
        "fill" => {
            camera_bottom = height as f64 * 0.55;
            seam = camera_bottom;
        }
        _ => {}
    }
    if style == "kick_banner" {
        let camera_below =
            layout == "split" && params.get("region_order").map(String::as_str) == Some("b_first");
        let seam_offset = if params.contains_key("social_tag_seam_offset") {
            check_range(
                parse_number(params, "social_tag_seam_offset")?,
                0.0,
                100.0,
                "Kick banner distance from camera",
            )?
        } else {
            0.0
        };
        return clipper_kick_banner_filter(
            params,
            username,
            requested,
            KickBannerPlacement {
                width,
                height,
                camera_left,
                camera_width,
                camera_bottom,
                seam,
                camera_below,
                seam_offset,
            },
        )
        .map(Some);
    }
    let camera_right = camera_left + camera_width;
    // Only the boxed badge needs an inset against phone-side cropping.
    let inset = if style == "boxed" {
        width as f64 * 0.05
    } else {
        0.0
    };
    let safe_left = if style == "boxed" {
        inset.max(camera_left)
    } else {
        inset
    };
    let safe_right = if style == "boxed" {
        (width as f64 - inset).min(camera_right)
    } else {
        width as f64 - inset
    };
    let target_x = match position {
        "left" => camera_left.max(safe_left),
        "center" => camera_left + camera_width / 2.0,
        _ => camera_right.min(safe_right),
    };
    let available = if style == "boxed" {
        (match position {
            "left" => safe_right - target_x,
            "right" => target_x - safe_left,
            _ => 2.0 * (target_x - safe_left).min(safe_right - target_x),
        })
        .max(24.0)
    } else {
        width as f64 * 0.84
    };
    let unit_scale = if !measured_units && style == "boxed" {
        1.1
    } else {
        1.0
    };
    let fitted_units = units * unit_scale;
    let size = requested.min(available / (fitted_units + 2.4));
    let side = (size * 1.5).round().max(8.0);
    let gap = if style == "plain" { size * 0.22 } else { 0.0 };
    let padding = (size * 0.34).round().max(2.0);
    let text_width = units * size * unit_scale;
    let total_width = side + gap + text_width + if style == "boxed" { padding * 2.0 } else { 0.0 };
    let raw_x = match position {
        "left" => target_x,
        "center" => target_x - total_width / 2.0,
        _ => target_x - total_width,
    };
    let x = raw_x
        .clamp(safe_left, (safe_right - total_width).max(safe_left))
        .round();
    let base_center_y = if style == "boxed" {
        if layout == "split" && params.get("region_order").map(String::as_str) == Some("b_first") {
            camera_top + side / 2.0
        } else {
            (camera_bottom - side / 2.0).max(camera_top + side / 2.0)
        }
    } else {
        seam
    };
    let camera_below =
        layout == "split" && params.get("region_order").map(String::as_str) == Some("b_first");
    let distance = if params.contains_key("social_tag_seam_offset") {
        check_range(
            parse_number(params, "social_tag_seam_offset")?,
            0.0,
            100.0,
            "Social Tag distance from camera",
        )?
    } else {
        0.0
    };
    let room = (if camera_below {
        seam
    } else {
        height as f64 - camera_bottom
    }) - if style == "plain" { side / 2.0 } else { 0.0 };
    let offset = (height as f64 * 0.22).min(room).max(0.0) * distance / 100.0;
    let center_y = base_center_y + if camera_below { -offset } else { offset };
    let y = (center_y - side / 2.0).round();
    let font_dir = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
        .join("Fonts");
    let font = ["ariblk.ttf", "arialbd.ttf"]
        .into_iter()
        .map(|name| font_dir.join(name))
        .find(|path| path.is_file())
        .ok_or("A bold Windows font is required for Social Tag.")?;
    let mut filters = Vec::new();
    if style == "boxed" {
        let icon_background = if platform == "twitch" {
            "white"
        } else {
            "0x53fc19"
        };
        let plate_color = if platform == "twitch" {
            "0x9146ff"
        } else {
            "black@0.88"
        };
        filters.push(format!(
            "drawbox=x={x:.0}:y={y:.0}:w={side:.0}:h={side:.0}:color={icon_background}:t=fill"
        ));
        filters.push(format!(
            "drawbox=x={:.0}:y={y:.0}:w={:.0}:h={side:.0}:color={plate_color}:t=fill",
            x + side,
            (total_width - side).round()
        ));
    }
    let (mark_width, mark_height) = if platform == "twitch" {
        let mark_side = side * if style == "boxed" { 0.80 } else { 0.84 };
        (mark_side.round(), mark_side.round())
    } else {
        let mark_height = side * if style == "boxed" { 0.72 } else { 0.84 };
        ((mark_height * 8.0 / 9.0).round(), mark_height.round())
    };
    let text_x = x + side + gap + if style == "boxed" { padding } else { 0.0 };
    // Descenders carry little visual weight below the baseline. Center the
    // visible word, rather than only FFmpeg's full glyph bounding box.
    let descenders = username
        .chars()
        .filter(|letter| matches!(*letter, 'g' | 'j' | 'p' | 'q' | 'y'))
        .count();
    let optical_offset = if descenders == 0 {
        0.0
    } else if style == "boxed" {
        (size * 0.32 * descenders as f64 / length as f64)
            .min(size * 0.08)
            .round()
    } else {
        (size * 0.08).round()
    };
    let text_y = if style == "boxed" {
        if optical_offset > 0.0 {
            format!("{y:.0}+({side:.0}-text_h)/2+{optical_offset:.0}")
        } else {
            format!("{y:.0}+({side:.0}-text_h)/2")
        }
    } else {
        if optical_offset > 0.0 {
            format!("{center_y:.0}-text_h/2+{optical_offset:.0}")
        } else {
            format!("{center_y:.0}-text_h/2")
        }
    };
    let outline = if style == "boxed" {
        "".to_string()
    } else {
        ":borderw=3:bordercolor=black@0.95:shadowx=2:shadowy=2:shadowcolor=black@0.9".into()
    };
    filters.push(format!(
        "drawtext=fontfile='{}':text='{}':expansion=none:fontcolor=white:fontsize={size:.0}:x={text_x:.0}:y={text_y}{outline}",
        drawtext_escape(&font.to_string_lossy()),
        drawtext_escape(username)
    ));
    let mark = social_tag_mark_path(platform, style)?;
    let mark_x = (x + (side - mark_width) / 2.0).round();
    let mark_y = (y + (side - mark_height) / 2.0).round();
    // One high-resolution, alpha-bearing mark instead of separately rounded
    // rectangles. 4:4:4 compositing preserves odd-pixel edges before encoding.
    // Repeat the still image through the entire video, not only its first frame.
    Ok(Some(format!(
        "{}[social_base];movie=filename={}:dec_threads=1,scale={mark_width:.0}:{mark_height:.0}:flags=lanczos,format=rgba[social_mark];[social_base][social_mark]overlay=x={mark_x:.0}:y={mark_y:.0}:format=yuv444:eof_action=repeat:repeatlast=1",
        filters.join(","), movie_filter_path(&mark)
    )))
}

#[tauri::command]
pub(crate) async fn detect_camera_region(
    input: String,
    duration: f64,
    source_width: u32,
    source_height: u32,
) -> Result<face_detection::CameraDetectionResult, String> {
    let input = PathBuf::from(input);
    if !input.is_file() || source_width == 0 || source_height == 0 {
        return Err("Choose a valid video before detecting its camera region.".into());
    }
    let timestamps = face_detection::sample_timestamps(duration);
    // The whole frame and four overlapping corner tiles, each letterboxed to
    // 640x640 and stacked vertically in one raw RGB image.
    let even = |value: f32| ((value / 2.0).round() as u32 * 2).max(2);
    let tile_width = even(source_width as f32 * face_detection::TILE_FRACTION);
    let tile_height = even(source_height as f32 * face_detection::TILE_FRACTION);
    let fit = "scale=640:640:force_original_aspect_ratio=decrease:force_divisible_by=2,pad=640:640:(ow-iw)/2:(oh-ih)/2:color=black";
    let mut graph = format!("[0:v]split={}[whole]", face_detection::IMAGES_PER_SAMPLE);
    for index in 0..face_detection::IMAGES_PER_SAMPLE - 1 {
        graph.push_str(&format!("[source{index}]"));
    }
    graph.push_str(&format!(";[whole]{fit}[image0]"));
    for index in 0..face_detection::IMAGES_PER_SAMPLE - 1 {
        let (origin_x, origin_y) = face_detection::tile_origin(index);
        // Offsets are FFmpeg expressions so the crop stays inside the frame
        // even if the decoded size differs from the probed one.
        let x = if origin_x > 0.0 { "iw-ow" } else { "0" };
        let y = if origin_y > 0.0 { "ih-oh" } else { "0" };
        graph.push_str(&format!(
            ";[source{index}]crop=min({tile_width}\\,iw):min({tile_height}\\,ih):{x}:{y},{fit}[image{}]",
            index + 1
        ));
    }
    graph.push(';');
    for index in 0..face_detection::IMAGES_PER_SAMPLE {
        graph.push_str(&format!("[image{index}]"));
    }
    graph.push_str(&format!(
        "vstack=inputs={}[stack]",
        face_detection::IMAGES_PER_SAMPLE
    ));
    let image_bytes = 640 * 640 * 3;
    let sample_bytes = image_bytes * face_detection::IMAGES_PER_SAMPLE;
    let mut frames = Vec::with_capacity(timestamps.len());
    let mut extraction_errors = Vec::new();
    for (index, timestamp) in timestamps.iter().enumerate() {
        let output = process_timeout::output_within(
            hidden_command("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-ss"])
                .arg(format!("{timestamp:.3}"))
                .arg("-i")
                .arg(&input)
                .args(["-filter_complex", &graph, "-map", "[stack]"])
                .args([
                    "-frames:v",
                    "1",
                    "-an",
                    "-sn",
                    "-dn",
                    "-pix_fmt",
                    "rgb24",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ]),
            process_timeout::FRAME_SAMPLE,
        )
        .await
        .map_err(|error| format!("Camera analysis could not start FFmpeg: {error}"))?;
        if output.status.success() && output.stdout.len() == sample_bytes {
            frames.push((index, output.stdout));
        } else {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            extraction_errors.push(if detail.is_empty() {
                format!("No frame was available at {timestamp:.2}s.")
            } else {
                detail
            });
        }
    }
    if frames.is_empty() {
        return Err(extraction_errors
            .into_iter()
            .next()
            .unwrap_or_else(|| "No video frames could be analyzed.".into()));
    }
    tokio::task::spawn_blocking(move || {
        let mut detector = face_detection::YuNet::load()
            .map_err(|error| format!("Camera detector could not initialize: {error}"))?;
        let mut detected = Vec::with_capacity(frames.len());
        let mut rgb_samples = Vec::with_capacity(frames.len());
        for (index, mut pixels) in frames {
            let mut tiles = Vec::with_capacity(face_detection::IMAGES_PER_SAMPLE - 1);
            for tile in 1..face_detection::IMAGES_PER_SAMPLE {
                let image = &pixels[tile * image_bytes..(tile + 1) * image_bytes];
                let faces = detector
                    .detect(image)
                    .map_err(|error| format!("Camera detection failed: {error}"))?;
                tiles.push((
                    tile - 1,
                    face_detection::normalize_faces(faces, tile_width, tile_height),
                ));
            }
            pixels.truncate(image_bytes);
            pixels.shrink_to_fit();
            let whole = detector
                .detect(&pixels)
                .map_err(|error| format!("Camera detection failed: {error}"))?;
            detected.push(face_detection::FrameFaces {
                index,
                faces: face_detection::merge_tile_faces(
                    face_detection::normalize_faces(whole, source_width, source_height),
                    tiles,
                ),
            });
            rgb_samples.push(pixels);
        }
        let region = face_detection::choose_camera_region(detected, source_width, source_height).ok_or_else(|| {
            "No stable camera face was found. Move to a section where the camera is visible or adjust Camera Region manually."
                .to_string()
        })?;
        Ok(face_detection::refine_camera_region(region, &rgb_samples, source_width, source_height))
    })
    .await
    .map_err(|error| format!("Camera analysis task failed: {error}"))?
}
