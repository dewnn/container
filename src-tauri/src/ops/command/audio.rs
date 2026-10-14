//! Audio tool builders.

use crate::*;

pub(crate) fn build_audio_lab(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if !matches!(info.kind.as_str(), "audio" | "video") || info.audio_tracks.is_empty() {
        return Err("Audio Lab requires an audio stream.".into());
    }
    let target = match param(p, "preset")? {
        "youtube" => -14.0,
        "podcast" => -16.0,
        "broadcast" => -23.0,
        "custom" => check_range(parse_number(p, "lufs")?, -70.0, -5.0, "Loudness")?,
        _ => return Err("Invalid audio preset.".into()),
    };
    let peak = check_range(parse_number(p, "true_peak")?, -9.0, 0.0, "True Peak")?;
    let mut filters = Vec::new();
    match param(p, "noise_reduction")? {
        "off" => {}
        "light" => filters.push("afftdn=nr=6".to_string()),
        "medium" => filters.push("afftdn=nr=12".to_string()),
        _ => return Err("Invalid noise reduction.".into()),
    }
    match param(p, "voice_enhance")? {
        "off" => {}
        "on" => filters.push("highpass=f=80,lowpass=f=12000".into()),
        _ => return Err("Invalid voice setting.".into()),
    }
    match param(p, "compressor")? {
        "off" => {}
        "on" => filters
            .push("acompressor=threshold=0.125:ratio=3:attack=20:release=250:makeup=1".into()),
        _ => return Err("Invalid compressor.".into()),
    }
    filters.push(format!("loudnorm=I={target}:TP={peak}:LRA=11"));
    let fade_in = check_range(parse_number(p, "fade_in")?, 0.0, 60.0, "Fade in")?;
    let fade_out = check_range(parse_number(p, "fade_out")?, 0.0, 60.0, "Fade out")?;
    if fade_in > 0.0 {
        filters.push(format!("afade=t=in:d={fade_in}"));
    }
    if fade_out > 0.0 {
        let duration = info.duration.ok_or("Fade out requires a known duration.")?;
        if fade_out > duration {
            return Err("Fade out exceeds duration.".into());
        }
        filters.push(format!(
            "afade=t=out:st={}:d={fade_out}",
            duration - fade_out
        ));
    }
    args.extend([
        "-map".into(),
        "0:a:0".into(),
        "-af".into(),
        filters.join(","),
        "-ar".into(),
        "48000".into(),
    ]);
    match param(p, "channels")? {
        "source" => {}
        "mono" => args.extend(["-ac".into(), "1".into()]),
        "stereo" => args.extend(["-ac".into(), "2".into()]),
        _ => return Err("Invalid channel mode.".into()),
    }
    if info.kind == "video" {
        args.extend([
            "-map".into(),
            "0:v:0".into(),
            "-c:v".into(),
            "copy".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
        ]);
        *extension = "mkv".into();
    } else {
        *extension = param(p, "format")?.into();
        let codec = match extension.as_str() {
            "flac" => "flac",
            "wav" => "pcm_s24le",
            "mp3" => "libmp3lame",
            "opus" => "libopus",
            _ => return Err("Invalid audio format.".into()),
        };
        args.extend(["-c:a".into(), codec.into()]);
    }
    Ok(())
}

pub(crate) fn build_remove_audio(
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-c:v".into(),
        "copy".into(),
        "-an".into(),
    ]);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_extract_audio(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    op: &str,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if op == "extract_audio" && info.audio_codec.is_none() {
        return Err("This video has no audio stream.".into());
    }
    if op == "extract_audio" {
        args.push("-vn".into());
        let requested_format = param(p, "format")?;
        append_audio_routing(args, info, p, false, "main")?;
        if requested_format == "copy" {
            *extension = "mka".into();
        } else {
            let (format, args_audio) = audio_format(requested_format)?;
            args.extend(args_audio);
            *extension = if p.get("audio_mode").is_some_and(|mode| mode == "all") {
                "mka".into()
            } else {
                format.into()
            };
        }
    } else {
        let (format, args_audio) = audio_format(param(p, "format")?)?;
        args.extend(args_audio);
        *extension = format.into();
    }
    Ok(())
}

pub(crate) fn build_replace_audio(
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let audio = PathBuf::from(param(p, "audio_path")?);
    if !audio.is_file() {
        return Err("Replacement audio was not found.".into());
    }
    args.extend([
        "-i".into(),
        audio.to_string_lossy().into(),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "1:a:0".into(),
        "-c:v".into(),
        "copy".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "320k".into(),
        "-shortest".into(),
    ]);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_distortion(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if info.kind == "video" && info.audio_codec.is_none() {
        return Err("This video has no audio stream.".into());
    }
    let level = check_range(parse_number(p, "level")?, 1.0, 10.0, "Severity")?;
    let gain = level * 10.0;
    let points = [0, 600, 1500, 3000, 6000, 12000, 16000]
        .iter()
        .map(|f| format!("entry({f},{gain})"))
        .collect::<Vec<_>>()
        .join(";");
    if info.kind == "video" {
        args.extend([
            "-map".into(),
            "0:v:0".into(),
            "-map".into(),
            "0:a:0".into(),
            "-c:v".into(),
            "copy".into(),
        ]);
    }
    args.extend([
        "-af".into(),
        format!("firequalizer=gain_entry='{points}'"),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "256k".into(),
    ]);
    *extension = if info.kind == "video" { "mp4" } else { "m4a" }.into();
    Ok(())
}
