//! PNG optimization stays local and is compiled into the application.
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// Quantize premultiplied linear RGB with an exactly invertible transform.
// This avoids tint shifts from the library's approximate default alpha/gamma transform.
struct PngColorSpace;
impl exoquant::ColorSpace for PngColorSpace {
    fn to_linear(&self, color: exoquant::Colorf) -> exoquant::Colorf {
        exoquant::Colorf {r:color.r.powf(2.2)*color.a,g:color.g.powf(2.2)*color.a,b:color.b.powf(2.2)*color.a,a:color.a}
    }
    fn from_linear(&self, color: exoquant::Colorf) -> exoquant::Colorf {
        if color.a <= 0.0 { return exoquant::Colorf::default(); }
        exoquant::Colorf {r:(color.r/color.a).max(0.0).powf(1.0/2.2),g:(color.g/color.a).max(0.0).powf(1.0/2.2),b:(color.b/color.a).max(0.0).powf(1.0/2.2),a:color.a}
    }
}

fn check_cancel(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) { Err("Job cancelled.".into()) } else { Ok(()) }
}

fn optimize(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut options = oxipng::Options::from_preset(3);
    options.timeout = Some(Duration::from_secs(10));
    options.max_decompressed_size = Some(256 * 1024 * 1024);
    // Do not change RGB hidden behind alpha or strip colour profiles/metadata.
    options.optimize_alpha = false;
    let optimized = oxipng::optimize_from_memory(bytes, &options)
        .map_err(|error| format!("PNG optimization failed: {error}"))?;
    Ok(if optimized.len() < bytes.len() { optimized } else { bytes.to_vec() })
}

type PngChunks<'a> = Vec<(&'a [u8], &'a [u8])>;
fn chunks(bytes: &[u8]) -> Result<PngChunks<'_>, String> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return Err("Invalid PNG file.".into()); }
    let mut result = Vec::new();
    let mut offset = 8usize;
    while offset < bytes.len() {
        let header = bytes.get(offset..offset + 8).ok_or("Truncated PNG header.")?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset.checked_add(length).and_then(|v| v.checked_add(12)).ok_or("Invalid PNG length.")?;
        let chunk = bytes.get(offset..end).ok_or("Truncated PNG chunk.")?;
        result.push((&chunk[4..8], chunk));
        offset = end;
    }
    Ok(result)
}

fn indexed_png(source: &[u8], pixels: &[exoquant::Color], width: u32, height: u32, colors: usize) -> Result<Vec<u8>, String> {
    use exoquant::optimizer::Optimizer;
    let colorspace = PngColorSpace;
    // Bound palette training cost, but remap every pixel (no resizing).
    let stride = (pixels.len() / 100_000).max(1);
    let histogram = pixels.iter().step_by(stride).copied().collect();
    let optimizer = exoquant::optimizer::KMeans;
    let palette = exoquant::generate_palette(&histogram, &colorspace, &optimizer, colors);
    let mut palette = optimizer.optimize_palette(&colorspace, &palette, &histogram, 8);
    for color in &mut palette {
        if color.a <= 1 { color.a = 0; }
        if color.a >= 254 { color.a = 255; }
    }
    // Reserve exact alpha endpoints when the quantizer rounded them away.
    for alpha in [0,255] {
        if let Some(pixel) = pixels.iter().find(|pixel| pixel.a == alpha) {
            if !palette.iter().any(|color| color.a == alpha) {
                let index = if alpha == 0 {0} else {palette.len()-1};
                palette[index] = *pixel;
            }
        }
    }
    let dither = exoquant::ditherer::FloydSteinberg::new();
    // Already-indexed artwork should keep its clean flat regions, not gain grain.
    let mut indices = if source.get(25) == Some(&3) {
        exoquant::Remapper::new(&palette, &colorspace, &exoquant::ditherer::None).remap(pixels, width as usize)
    } else {
        exoquant::Remapper::new(&palette, &colorspace, &dither).remap(pixels, width as usize)
    };
    for (pixel,index) in pixels.iter().zip(&mut indices) {
        if [0,255].contains(&pixel.a) && palette[*index as usize].a != pixel.a {
            *index = palette.iter().enumerate().filter(|(_,color)| color.a == pixel.a)
                .min_by_key(|(_,color)| (i32::from(color.r)-i32::from(pixel.r)).pow(2)+(i32::from(color.g)-i32::from(pixel.g)).pow(2)+(i32::from(color.b)-i32::from(pixel.b)).pow(2))
                .ok_or("PNG alpha endpoint missing.")?.0 as u8;
        }
    }
    // Check the visible result on BOTH black and white backgrounds, including alpha.
    // Refuse strong degradation even when a very small target was requested.
    let mut error = 0f64;
    for (pixel, index) in pixels.iter().zip(&indices) {
        let mapped = palette[*index as usize];
        for background in [0.0, 255.0] {
            for (a, b) in [(pixel.r, mapped.r), (pixel.g, mapped.g), (pixel.b, mapped.b)] {
                let original = f64::from(a) * f64::from(pixel.a) / 255.0 + background * (1.0 - f64::from(pixel.a) / 255.0);
                let result = f64::from(b) * f64::from(mapped.a) / 255.0 + background * (1.0 - f64::from(mapped.a) / 255.0);
                error += (original - result).powi(2);
            }
        }
        // Never turn fully transparent/opaque pixels into translucent pixels.
        if (pixel.a == 0 && mapped.a != 0) || (pixel.a == 255 && mapped.a != 255) {
            return Err("PNG palette would change transparency boundaries.".into());
        }
    }
    let mse = error / (pixels.len() as f64 * 6.0);
    if mse > 255f64.powi(2) / 10f64.powf(38.0 / 10.0) {
        return Err(format!("PNG palette would exceed the colour error limit ({:.2} dB).",10.0*(255f64.powi(2)/mse).log10()));
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(palette.iter().flat_map(|c| [c.r,c.g,c.b]).collect::<Vec<_>>());
        encoder.set_trns(palette.iter().map(|c| c.a).collect::<Vec<_>>());
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&indices).map_err(|e| e.to_string())?;
    }
    // Preserve colour interpretation and safe descriptive metadata from the source.
    let mut preserved = bytes[..33].to_vec(); // PNG signature + IHDR
    for (kind, chunk) in chunks(source)? {
        if [b"iCCP", b"sRGB", b"gAMA", b"cHRM", b"cICP", b"pHYs", b"eXIf", b"tEXt", b"zTXt", b"iTXt"].iter().any(|name| name.as_slice() == kind) {
            preserved.extend_from_slice(chunk);
        }
    }
    preserved.extend_from_slice(&bytes[33..]);
    optimize(&preserved)
}

pub fn compress(bytes: &[u8], palette: bool, target: Option<u64>, cancelled: &AtomicBool) -> Result<Vec<u8>, String> {
    check_cancel(cancelled)?;
    let source_chunks = chunks(bytes)?;
    let mut best = optimize(bytes)?;
    check_cancel(cancelled)?;
    if !palette || target.is_some_and(|size| best.len() as u64 <= size) { return Ok(best); }
    // Keep APNG frames and 16-bit samples intact; palette conversion is 8-bit only.
    if source_chunks.iter().any(|(kind, _)| *kind == b"acTL") || bytes[24] == 16 { return Ok(best); }
    let width=u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height=u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if u64::from(width)*u64::from(height)>16_000_000 { return Ok(best); }
    let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?.into_rgba8();
    let pixels: Vec<_> = decoded.pixels().map(|p| exoquant::Color::new(p[0],p[1],p[2],p[3])).collect();
    let levels: &[usize] = if target.is_some() { &[256,192,128,96,64] } else { &[256] };
    for &colors in levels {
        check_cancel(cancelled)?;
        if let Ok(candidate) = indexed_png(bytes, &pixels, decoded.width(), decoded.height(), colors) {
            if candidate.len() < best.len() { best = candidate; }
        }
        check_cancel(cancelled)?;
        if target.is_some_and(|size| best.len() as u64 <= size) { break; }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let image = image::RgbaImage::from_fn(128,128,|x,y| image::Rgba([(x*2) as u8,(y*2) as u8,80,if x<16 {0} else {255}]));
        let mut bytes=Vec::new();
        let mut encoder=png::Encoder::new(&mut bytes,128,128);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::NoCompression);
        encoder.write_header().unwrap().write_image_data(image.as_raw()).unwrap();
        bytes
    }
    #[test]
    fn lossless_preserves_every_rgba_pixel_and_never_grows() {
        let bytes=fixture();let cancel=AtomicBool::new(false);
        let result=compress(&bytes,false,Some(1),&cancel).unwrap();
        assert!(result.len()<bytes.len());
        assert_eq!(image::load_from_memory(&bytes).unwrap().into_rgba8(),image::load_from_memory(&result).unwrap().into_rgba8());
        assert!(compress(&result,false,None,&cancel).unwrap().len()<=result.len());
    }
    #[test]
    fn palette_keeps_dimensions_transparency_and_returns_unreachable_targets() {
        let bytes=fixture();let result=compress(&bytes,true,Some(1),&AtomicBool::new(false)).unwrap();
        let image=image::load_from_memory(&result).unwrap().into_rgba8();
        assert_eq!(image.dimensions(),(128,128));assert_eq!(image.get_pixel(0,0)[3],0);assert_eq!(image.get_pixel(127,127)[3],255);
        assert!(result.len()<=bytes.len());assert!(result.len()>1);
    }
    #[test]
    fn cancellation_and_invalid_input_do_not_produce_output() {
        assert!(compress(&fixture(),true,None,&AtomicBool::new(true)).unwrap_err().contains("cancelled"));
        assert!(compress(b"not png",false,None,&AtomicBool::new(false)).is_err());
    }
    #[test]
    fn palette_really_encodes_indexed_png_and_preserves_colour_profile() {
        let bytes=fixture();
        let image=image::load_from_memory(&bytes).unwrap().into_rgba8();
        let pixels:Vec<_>=image.pixels().map(|p| exoquant::Color::new(p[0]/32*32,p[1]/32*32,p[2],p[3])).collect();
        let output=indexed_png(&bytes,&pixels,128,128,256).unwrap();
        assert_eq!(output[25],3,"palette option must exercise indexed PNG encoding");
        let mut original=Vec::new();
        let mut encoder=png::Encoder::new(&mut original,64,64);
        encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_gamma(png::ScaledFloat::new(0.45455));
        encoder.write_header().unwrap().write_image_data(&vec![255;64*64*4]).unwrap();
        let result=compress(&original,true,None,&AtomicBool::new(false)).unwrap();
        let gamma=|data:&[u8]| chunks(data).unwrap().into_iter().find(|(kind,_)| *kind==b"gAMA").unwrap().1.to_vec();
        assert_eq!(gamma(&original),gamma(&result));
    }
    #[test]
    fn severe_palette_degradation_is_rejected_instead_of_forcing_a_target() {
        let bytes=fixture();let image=image::load_from_memory(&bytes).unwrap().into_rgba8();
        let pixels:Vec<_>=image.pixels().map(|p| exoquant::Color::new(p[0],p[1],p[2],p[3])).collect();
        assert!(indexed_png(&bytes,&pixels,128,128,2).unwrap_err().contains("colour error limit"));
    }
    #[test]
    fn sixteen_bit_samples_are_never_quantized() {
        let mut original=Vec::new();let mut encoder=png::Encoder::new(&mut original,8,8);
        encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Sixteen);
        let pixels:Vec<_>=(0..256u16).flat_map(|v| (v*251).to_be_bytes()).collect();
        encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
        let result=compress(&original,true,Some(1),&AtomicBool::new(false)).unwrap();
        assert_eq!(image::load_from_memory(&original).unwrap().into_rgba16(),image::load_from_memory(&result).unwrap().into_rgba16());
    }
    #[test]
    fn animated_png_keeps_all_frames() {
        let mut original=Vec::new();let mut encoder=png::Encoder::new(&mut original,4,4);
        encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);encoder.set_animated(2,0).unwrap();
        {let mut writer=encoder.write_header().unwrap();writer.write_image_data(&[128;64]).unwrap();writer.write_image_data(&[64;64]).unwrap();}
        let result=compress(&original,true,Some(1),&AtomicBool::new(false)).unwrap();
        let frames=|data:&[u8]| chunks(data).unwrap().into_iter().filter(|(kind,_)| *kind==b"fcTL").count();
        assert_eq!(frames(&original),2);assert_eq!(frames(&result),2);
        let mut decoder=png::Decoder::new(std::io::Cursor::new(result)).read_info().unwrap();
        assert_eq!(decoder.info().animation_control.unwrap().num_frames,2);
        let mut pixels=vec![0;64];decoder.next_frame(&mut pixels).unwrap();assert_eq!(pixels,[128;64]);
        decoder.next_frame(&mut pixels).unwrap();assert_eq!(pixels,[64;64]);
    }
    #[test]
    #[ignore="set CONTAINER_PNG_TEST_MEDIA to benchmark a supplied PNG"]
    fn supplied_png_benchmark() {
        let path=std::env::var("CONTAINER_PNG_TEST_MEDIA").expect("CONTAINER_PNG_TEST_MEDIA");
        let bytes=std::fs::read(&path).unwrap();let original=image::load_from_memory(&bytes).unwrap().into_rgba8();
        let output=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/png-audit");std::fs::create_dir_all(&output).unwrap();
        for (name,palette,target) in [("lossless",false,Some(1_024_000)),("palette",true,None),("palette-target",true,Some(100_000))] {
            let start=std::time::Instant::now();let result=compress(&bytes,palette,target,&AtomicBool::new(false)).unwrap();
            let decoded=image::load_from_memory(&result).unwrap().into_rgba8();
            assert_eq!(original.dimensions(),decoded.dimensions());assert!(result.len()<=bytes.len());
            if !palette {assert_eq!(original,decoded);}
            for (before,after) in original.pixels().zip(decoded.pixels()) {
                if [0,255].contains(&before[3]) {assert_eq!(before[3],after[3]);}
            }
            std::fs::write(output.join(format!("{name}.png")),&result).unwrap();
            println!("{name}: {} -> {} bytes, {:.2}s, PNG colour type {}",bytes.len(),result.len(),start.elapsed().as_secs_f64(),result[25]);
        }
    }
}
