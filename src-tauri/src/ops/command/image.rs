//! Still image tool builders.

use crate::*;

pub(crate) fn build_ratio(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    let ratio = param(p, "ratio")?;
    let (rw, rh) = match ratio {
        "1:1" => (1, 1),
        "4:5" => (4, 5),
        "9:16" => (9, 16),
        "16:9" => (16, 9),
        "4:3" => (4, 3),
        _ => return Err("Invalid ratio.".into()),
    };
    args.extend(["-vf".into(), format!("crop='if(gt(iw/ih,{rw}/{rh}),trunc(ih*{rw}/{rh}/2)*2,iw)':'if(gt(iw/ih,{rw}/{rh}),ih,trunc(iw*{rh}/{rw}/2)*2)'"), "-c:v".into(), "libx264".into(), "-crf".into(), "16".into(), "-preset".into(), "veryfast".into()]);
    append_mp4_audio_codec(args, info);
    *extension = "mp4".into();
    Ok(())
}

pub(crate) fn build_image_ratio(
    info: &MediaInfo,
    p: &HashMap<String, String>,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if info.kind != "image" {
        return Err("Social Ratio / Crop requires an image file.".into());
    }
    let ratio = param(p, "ratio")?;
    let (rw, rh) = match ratio {
        "1:1" => (1, 1),
        "4:5" => (4, 5),
        "9:16" => (9, 16),
        "16:9" => (16, 9),
        "191:100" => (191, 100),
        "2:3" => (2, 3),
        "3:2" => (3, 2),
        "4:3" => (4, 3),
        _ => return Err("Invalid image ratio.".into()),
    };
    args.extend([
        "-vf".into(),
        format!("crop='if(gt(iw/ih,{rw}/{rh}),floor(ih*{rw}/{rh}),iw)':'if(gt(iw/ih,{rw}/{rh}),ih,floor(iw*{rh}/{rw}))'"),
        "-frames:v".into(),
        "1".into(),
    ]);
    *extension = match param(p, "format")? {
        "png" => {
            args.extend(["-c:v".into(), "png".into()]);
            "png".into()
        }
        "jpg" | "jpeg" => {
            args.extend(["-c:v".into(), "mjpeg".into(), "-q:v".into(), "2".into()]);
            "jpg".into()
        }
        _ => return Err("Invalid image output format.".into()),
    };
    Ok(())
}

pub(crate) fn build_metadata_cleaner(
    info: &MediaInfo,
    input: &Path,
    args: &mut Vec<String>,
    extension: &mut String,
) -> Result<(), String> {
    if info.kind != "image" {
        return Err("Metadata Cleaner requires an image file.".into());
    }
    *extension = image_extension(input);
    args.extend([
        "-map_metadata".into(),
        "-1".into(),
        "-map".into(),
        "0:v:0".into(),
        "-frames:v".into(),
        "1".into(),
        "-c:v".into(),
        "copy".into(),
    ]);
    Ok(())
}
