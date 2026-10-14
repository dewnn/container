//! FFmpeg argument builder for every single-step Toolbox operation.
//!
//! `build_command` validates the request and dispatches to one builder per
//! tool; the builders live in the submodules grouped by media type.

mod audio;
mod export;
mod image;
mod overlay;
mod picture;

use audio::*;
use export::*;
use image::*;
use overlay::*;
use picture::*;

use crate::*;

pub(crate) async fn build_command(
    request: &OperationRequest,
    info: &MediaInfo,
) -> Result<(Vec<String>, PathBuf), String> {
    let input = PathBuf::from(&request.input);
    let p = &request.params;
    let op = request.operation.as_str();
    let mut args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        request.input.clone(),
    ];
    let mut extension = String::new();

    match op {
        "audio_lab" => build_audio_lab(info, p, &mut args, &mut extension)?,
        "transform" | "clipper" => build_transform(info, p, op, &mut args, &mut extension)?,
        "ratio" => build_ratio(info, p, &mut args, &mut extension)?,
        "image_ratio" => build_image_ratio(info, p, &mut args, &mut extension)?,
        "metadata_cleaner" => build_metadata_cleaner(info, &input, &mut args, &mut extension)?,
        "image_compressor" => return Err("image_compressor_internal".into()),
        "upscale" => build_upscale(info, &input, p, &mut args, &mut extension).await?,
        "resize" => build_resize(info, p, &mut args, &mut extension)?,
        "fps" | "cfr" => build_fps(info, p, &mut args, &mut extension)?,
        "interpolation" => build_interpolation(info, p, &mut args, &mut extension)?,
        "frame_blend" => build_frame_blend(info, p, &mut args, &mut extension)?,
        "speed" => build_speed(request, info, p, &mut args, &mut extension)?,
        "compression" => build_compression(p, &mut args, &mut extension)?,
        "potatoify" => build_potatoify(info, p, &mut args, &mut extension)?,
        "image_overlay" => {
            build_image_overlay(request, info, &input, p, &mut args, &mut extension).await?
        }
        "blur_pixelate" => build_blur_pixelate(info, &input, p, &mut args, &mut extension).await?,
        "subtitles" => build_subtitles(request, info, &input, p, &mut args, &mut extension).await?,
        "text" => build_text(info, &input, p, &mut args, &mut extension).await?,
        "color" => build_color(info, &input, p, &mut args, &mut extension)?,
        "noise" => build_noise(info, p, &mut args, &mut extension)?,
        "negate" => build_negate(info, &mut args, &mut extension)?,
        "encode" => build_encode(info, p, &mut args, &mut extension)?,
        "proxy" => build_proxy(info, &input, p, &mut args, &mut extension).await?,
        "fix_timestamps" => {
            build_fix_timestamps(request, info, &input, p, &mut args, &mut extension)?
        }
        "cut" => build_cut(request, info, &input, p, &mut args, &mut extension)?,
        "remux" => build_remux(info, p, &mut args, &mut extension)?,
        "screenshot" => build_screenshot(request, info, p, &mut args, &mut extension)?,
        "gif" => build_gif(request, info, &input, p, &mut args, &mut extension).await?,
        "remove_audio" => build_remove_audio(&mut args, &mut extension)?,
        "extract_audio" | "audio_convert" => {
            build_extract_audio(info, p, op, &mut args, &mut extension)?
        }
        "replace_audio" => build_replace_audio(p, &mut args, &mut extension)?,
        "distortion" => build_distortion(info, p, &mut args, &mut extension)?,
        "image_potatoify" => {
            return Err("image_potatoify_internal".into());
        }
        _ => return Err("Unknown operation.".into()),
    }
    if extension == "mp4" {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    let output_source = operation_output_source(request)?;
    let output = unique_output(&output_source, op, &extension)?;
    args.push(output.to_string_lossy().to_string());
    Ok((args, output))
}
