use ndarray::Array4;
use ort::{execution_providers::CPUExecutionProvider, session::Session, value::TensorRef};
use serde::Serialize;
use std::collections::HashSet;

const MODEL_BYTES: &[u8] = include_bytes!("../resources/face_detection_yunet_2023mar.onnx");
const INPUT_SIZE: usize = 640;
const SAMPLE_COUNT: usize = 11;
const STRIDES: [usize; 3] = [8, 16, 32];

#[derive(Debug, Clone)]
pub(crate) struct Face {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    confidence: f32,
}

#[derive(Debug, Clone)]
pub struct FrameFaces {
    pub index: usize,
    pub faces: Vec<NormalizedFace>,
}

#[derive(Debug, Clone)]
pub struct NormalizedFace {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CameraDetectionResult {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub focal_x: f32,
    pub focal_y: f32,
    pub confidence: f32,
    pub samples: usize,
    pub matched_samples: usize,
}

#[derive(Default)]
struct FaceCluster {
    faces: Vec<NormalizedFace>,
    frames: HashSet<usize>,
}

impl FaceCluster {
    fn representative(&self) -> NormalizedFace {
        NormalizedFace {
            x: median(self.faces.iter().map(|face| face.x)),
            y: median(self.faces.iter().map(|face| face.y)),
            width: median(self.faces.iter().map(|face| face.width)),
            height: median(self.faces.iter().map(|face| face.height)),
            confidence: median(self.faces.iter().map(|face| face.confidence)),
        }
    }
}

fn median(values: impl Iterator<Item = f32>) -> f32 {
    let mut values: Vec<f32> = values.filter(|value| value.is_finite()).collect();
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

pub fn sample_timestamps(duration: f64) -> Vec<f64> {
    if !duration.is_finite() || duration <= 0.0 {
        return vec![0.0];
    }
    if duration < 3.0 {
        return vec![(duration * 0.5).max(0.0)];
    }
    (0..SAMPLE_COUNT)
        .map(|index| {
            let fraction = 0.06 + 0.88 * index as f64 / (SAMPLE_COUNT - 1) as f64;
            (duration * fraction).min((duration - 0.05).max(0.0))
        })
        .collect()
}

pub struct YuNet {
    session: Session,
    input: Array4<f32>,
}

impl YuNet {
    pub fn load() -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|error| error.to_string())?
            .with_intra_threads(1)
            .map_err(|error| error.to_string())?
            .with_inter_threads(1)
            .map_err(|error| error.to_string())?
            .with_execution_providers([CPUExecutionProvider::default().build()])
            .map_err(|error| error.to_string())?
            .commit_from_memory(MODEL_BYTES)
            .map_err(|error| error.to_string())?;
        let shape = session
            .inputs
            .first()
            .and_then(|input| input.input_type.tensor_shape())
            .ok_or("YuNet input shape is unavailable")?;
        if shape.len() != 4 || shape[2] != INPUT_SIZE as i64 || shape[3] != INPUT_SIZE as i64 {
            return Err("YuNet model has an unexpected input shape.".into());
        }
        Ok(Self {
            session,
            input: Array4::zeros((1, 3, INPUT_SIZE, INPUT_SIZE)),
        })
    }

    pub fn detect(&mut self, rgb: &[u8]) -> Result<Vec<Face>, String> {
        let expected = INPUT_SIZE * INPUT_SIZE * 3;
        if rgb.len() != expected {
            return Err(format!(
                "Camera analysis produced {} bytes; expected {expected}.",
                rgb.len()
            ));
        }
        for y in 0..INPUT_SIZE {
            for x in 0..INPUT_SIZE {
                let offset = (y * INPUT_SIZE + x) * 3;
                self.input[[0, 0, y, x]] = rgb[offset + 2] as f32;
                self.input[[0, 1, y, x]] = rgb[offset + 1] as f32;
                self.input[[0, 2, y, x]] = rgb[offset] as f32;
            }
        }
        let outputs = self
            .session
            .run(
                ort::inputs!["input" => TensorRef::from_array_view(&self.input)
                .map_err(|error| error.to_string())?],
            )
            .map_err(|error| format!("YuNet inference failed: {error}"))?;
        let mut faces = Vec::new();
        for stride in STRIDES {
            let feature = INPUT_SIZE.div_ceil(stride);
            let cls_name = format!("cls_{stride}");
            let obj_name = format!("obj_{stride}");
            let bbox_name = format!("bbox_{stride}");
            let (_, cls) = outputs[cls_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|error| error.to_string())?;
            let (_, obj) = outputs[obj_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|error| error.to_string())?;
            let (_, bbox) = outputs[bbox_name.as_str()]
                .try_extract_tensor::<f32>()
                .map_err(|error| error.to_string())?;
            for index in 0..feature * feature {
                let confidence = (cls[index].clamp(0.0, 1.0) * obj[index].clamp(0.0, 1.0)).sqrt();
                // OpenCV's own YuNet wrapper defaults to 0.60. Temporal
                // consistency below rejects isolated false positives, so the
                // official threshold improves recall for small webcam faces
                // without letting one weak frame decide the crop.
                if confidence < 0.60 {
                    continue;
                }
                let column = index % feature;
                let row = index / feature;
                let center_x = (column as f32 + bbox[index * 4]) * stride as f32;
                let center_y = (row as f32 + bbox[index * 4 + 1]) * stride as f32;
                let width = bbox[index * 4 + 2].exp() * stride as f32;
                let height = bbox[index * 4 + 3].exp() * stride as f32;
                faces.push(Face {
                    x: center_x - width / 2.0,
                    y: center_y - height / 2.0,
                    width,
                    height,
                    confidence,
                });
            }
        }
        Ok(non_maximum_suppression(faces, 0.3))
    }
}

fn intersection_over_union(left: &Face, right: &Face) -> f32 {
    let x1 = left.x.max(right.x);
    let y1 = left.y.max(right.y);
    let x2 = (left.x + left.width).min(right.x + right.width);
    let y2 = (left.y + left.height).min(right.y + right.height);
    if x2 <= x1 || y2 <= y1 {
        return 0.0;
    }
    let intersection = (x2 - x1) * (y2 - y1);
    intersection / (left.width * left.height + right.width * right.height - intersection).max(1.0)
}

fn non_maximum_suppression(mut faces: Vec<Face>, threshold: f32) -> Vec<Face> {
    faces.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
    let mut kept = Vec::new();
    while let Some(face) = faces.first().cloned() {
        faces.remove(0);
        faces.retain(|candidate| intersection_over_union(&face, candidate) <= threshold);
        kept.push(face);
    }
    kept
}

pub fn normalize_faces(
    faces: Vec<Face>,
    source_width: u32,
    source_height: u32,
) -> Vec<NormalizedFace> {
    let source_width = source_width.max(1) as f32;
    let source_height = source_height.max(1) as f32;
    let scale = (INPUT_SIZE as f32 / source_width).min(INPUT_SIZE as f32 / source_height);
    let scaled_width = source_width * scale;
    let scaled_height = source_height * scale;
    let pad_x = (INPUT_SIZE as f32 - scaled_width) / 2.0;
    let pad_y = (INPUT_SIZE as f32 - scaled_height) / 2.0;
    faces
        .into_iter()
        .filter_map(|face| {
            let x1 = ((face.x - pad_x) / scaled_width).clamp(0.0, 1.0);
            let y1 = ((face.y - pad_y) / scaled_height).clamp(0.0, 1.0);
            let x2 = ((face.x + face.width - pad_x) / scaled_width).clamp(0.0, 1.0);
            let y2 = ((face.y + face.height - pad_y) / scaled_height).clamp(0.0, 1.0);
            let width = x2 - x1;
            let height = y2 - y1;
            (width >= 0.012 && height >= 0.012 && width <= 0.65 && height <= 0.75).then_some(
                NormalizedFace {
                    x: x1,
                    y: y1,
                    width,
                    height,
                    confidence: face.confidence,
                },
            )
        })
        .collect()
}

/// Each sample is analysed once as a whole and once as four overlapping
/// corner tiles. A tile covers this fraction of the frame in each direction,
/// so small webcam faces reach the detector at about 1.8x the resolution.
pub const TILE_FRACTION: f32 = 0.55;
/// Number of 640x640 images per sample: the full frame plus four tiles.
pub const IMAGES_PER_SAMPLE: usize = 5;

/// Origin of tile `index` (0..4: top-left, top-right, bottom-left,
/// bottom-right) in normalized source coordinates.
pub fn tile_origin(index: usize) -> (f32, f32) {
    let far = 1.0 - TILE_FRACTION;
    (
        if index % 2 == 1 { far } else { 0.0 },
        if index >= 2 { far } else { 0.0 },
    )
}

/// Maps faces found in a tile back to whole-frame coordinates and merges them
/// with the faces found on the whole frame. Faces cut by an inner tile edge are
/// dropped: the neighbouring tile or the whole frame sees them complete.
pub fn merge_tile_faces(
    mut whole: Vec<NormalizedFace>,
    tiles: Vec<(usize, Vec<NormalizedFace>)>,
) -> Vec<NormalizedFace> {
    for (index, faces) in tiles {
        let (origin_x, origin_y) = tile_origin(index);
        for face in faces {
            let inner_left = origin_x > 0.0 && face.x < 0.01;
            let inner_top = origin_y > 0.0 && face.y < 0.01;
            let inner_right = origin_x == 0.0 && face.x + face.width > 0.99;
            let inner_bottom = origin_y == 0.0 && face.y + face.height > 0.99;
            if inner_left || inner_top || inner_right || inner_bottom {
                continue;
            }
            let mapped = NormalizedFace {
                x: origin_x + face.x * TILE_FRACTION,
                y: origin_y + face.y * TILE_FRACTION,
                width: face.width * TILE_FRACTION,
                height: face.height * TILE_FRACTION,
                confidence: face.confidence,
            };
            match whole
                .iter_mut()
                .find(|existing| normalized_overlap(existing, &mapped) > 0.3)
            {
                Some(existing) if existing.confidence >= mapped.confidence => {}
                Some(existing) => *existing = mapped,
                None => whole.push(mapped),
            }
        }
    }
    whole
}

fn normalized_overlap(left: &NormalizedFace, right: &NormalizedFace) -> f32 {
    let x1 = left.x.max(right.x);
    let y1 = left.y.max(right.y);
    let x2 = (left.x + left.width).min(right.x + right.width);
    let y2 = (left.y + left.height).min(right.y + right.height);
    if x2 <= x1 || y2 <= y1 {
        return 0.0;
    }
    let intersection = (x2 - x1) * (y2 - y1);
    intersection / (left.width * left.height + right.width * right.height - intersection).max(1e-6)
}

fn cluster_match(cluster: &FaceCluster, face: &NormalizedFace) -> Option<f32> {
    let mean = cluster.representative();
    let center_x = face.x + face.width / 2.0;
    let center_y = face.y + face.height / 2.0;
    let mean_x = mean.x + mean.width / 2.0;
    let mean_y = mean.y + mean.height / 2.0;
    let distance = ((center_x - mean_x).powi(2) + (center_y - mean_y).powi(2)).sqrt();
    let scale_delta = (face.width.max(0.001) / mean.width.max(0.001)).ln().abs()
        + (face.height.max(0.001) / mean.height.max(0.001)).ln().abs();
    let tolerance = 0.055 + mean.width.max(mean.height) * 0.45;
    (distance <= tolerance && scale_delta <= 0.95).then_some(distance + scale_delta * 0.025)
}

pub fn choose_camera_region(
    frames: Vec<FrameFaces>,
    source_width: u32,
    source_height: u32,
) -> Option<CameraDetectionResult> {
    let sample_count = frames.len();
    let mut clusters: Vec<FaceCluster> = Vec::new();
    for frame in frames {
        let mut faces = frame.faces;
        faces.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
        for face in faces {
            let best = clusters
                .iter()
                .enumerate()
                .filter(|(_, cluster)| !cluster.frames.contains(&frame.index))
                .filter_map(|(index, cluster)| {
                    cluster_match(cluster, &face).map(|score| (index, score))
                })
                .min_by(|left, right| left.1.total_cmp(&right.1));
            if let Some((index, _)) = best {
                clusters[index].frames.insert(frame.index);
                clusters[index].faces.push(face);
            } else {
                clusters.push(FaceCluster {
                    faces: vec![face],
                    frames: HashSet::from([frame.index]),
                });
            }
        }
    }
    let minimum_matches = match sample_count {
        0..=2 => 1,
        3..=7 => 2,
        _ => 3,
    };
    let last_frame = clusters
        .iter()
        .flat_map(|cluster| cluster.frames.iter().copied())
        .max()
        .unwrap_or(0)
        .max(1);
    let (cluster, score) = clusters
        .iter()
        .filter(|cluster| cluster.frames.len() >= minimum_matches)
        .filter(|cluster| {
            if sample_count < 6 {
                return true;
            }
            let first = cluster.frames.iter().copied().min().unwrap_or(0);
            let last = cluster.frames.iter().copied().max().unwrap_or(first);
            (last - first) as f32 / last_frame as f32 >= 0.28
        })
        .map(|cluster| {
            let mean = cluster.representative();
            let center_x = mean.x + mean.width / 2.0;
            let center_y = mean.y + mean.height / 2.0;
            let spread = median(cluster.faces.iter().map(|face| {
                let x = face.x + face.width / 2.0;
                let y = face.y + face.height / 2.0;
                ((x - center_x).powi(2) + (y - center_y).powi(2)).sqrt()
            }));
            let stability = (1.0 - spread * 8.0).clamp(0.0, 1.0);
            let coverage = cluster.frames.len() as f32 / sample_count.max(1) as f32;
            let first = cluster.frames.iter().copied().min().unwrap_or(0);
            let last = cluster.frames.iter().copied().max().unwrap_or(first);
            let temporal_span = (last - first) as f32 / last_frame as f32;
            let nearest_edge = center_x
                .min(1.0 - center_x)
                .min(center_y.min(1.0 - center_y));
            let edge_score = ((0.38 - nearest_edge) / 0.38).clamp(0.0, 1.0);
            let area = mean.width * mean.height;
            let overlay_size_score = if area <= 0.07 {
                1.0
            } else {
                (0.16 - area).max(0.0) / 0.09
            };
            let base_score = coverage * 0.43
                + temporal_span * 0.14
                + mean.confidence * 0.18
                + stability * 0.14
                + edge_score * 0.09
                + overlay_size_score * 0.02;
            // A webcam overlay should stay in one source location. Multiplying
            // by stability prevents a moving actor in the underlying content
            // from winning merely because a face is visible in every sample.
            let score = base_score * (0.45 + stability * 0.55);
            (cluster, score)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))?;
    if score < 0.48 {
        return None;
    }
    let mean = cluster.representative();
    let source_ratio = source_width.max(1) as f32 / source_height.max(1) as f32;
    let focal_x = mean.x + mean.width / 2.0;
    let focal_y = mean.y + mean.height / 2.0;
    // Streamer webcams are normally 16:9 overlays. The detector sees the face,
    // but the useful crop is the full webcam card around it: include the chair,
    // shoulders and deliberate headroom while stopping before the chat below.
    // A base panel allowance plus a face term keeps room/chair context for
    // normal webcams without letting a close face swallow half of the frame.
    // Small webcams are about 5.5-7x the face width, which that base overstates,
    // so the smaller of the two estimates wins. The edge search below then
    // snaps to the actual border.
    let height_scale = (16.0 / 9.0) / source_ratio;
    let panel_estimate =
        (0.145 + mean.width * 2.05).max((0.10 + mean.height * 1.20) * height_scale);
    let proportional_estimate =
        (0.02 + mean.width * 5.5).max((0.01 + mean.height * 2.6) * height_scale);
    let mut width = panel_estimate.min(proportional_estimate).clamp(0.06, 0.36);
    let mut height = width * source_ratio * 9.0 / 16.0;
    if height > 0.78 {
        height = 0.78;
        width = height / source_ratio * 16.0 / 9.0;
    }
    let x = (focal_x - width / 2.0).clamp(0.0, 1.0 - width);
    let maximum_y = (0.985 - height).max(0.0);
    // Small/distant faces need more room above them to capture the complete
    // webcam card without reaching the chat below. A close face already fills
    // the card, so reduce that headroom instead of shifting the whole crop too
    // far upward.
    let close_face = ((mean.width - 0.035) / 0.030).clamp(0.0, 1.0);
    let headroom = 0.90 - close_face * 0.30;
    let y = (mean.y - mean.height * headroom).clamp(0.0, maximum_y);
    Some(CameraDetectionResult {
        x: x * 100.0,
        y: y * 100.0,
        width: width * 100.0,
        height: height * 100.0,
        focal_x: focal_x * 100.0,
        focal_y: focal_y * 100.0,
        confidence: score.clamp(0.0, 1.0),
        samples: sample_count,
        matched_samples: cluster.frames.len(),
    })
}

// Face geometry provides a useful first crop, but not the edges of a webcam
// overlay. Look for persistent straight boundaries around that crop in the
// already decoded frames. No corner or aspect ratio is assumed here.
pub fn refine_camera_region(
    mut region: CameraDetectionResult,
    rgb_frames: &[Vec<u8>],
    source_width: u32,
    source_height: u32,
) -> CameraDetectionResult {
    if rgb_frames.is_empty() || source_width == 0 || source_height == 0 {
        return region;
    }
    let frames: Vec<&[u8]> = rgb_frames
        .iter()
        .filter(|frame| frame.len() == INPUT_SIZE * INPUT_SIZE * 3)
        .map(Vec::as_slice)
        .collect();
    if frames.is_empty() {
        return region;
    }
    let face_x = region.focal_x / 100.0;
    let face_y = region.focal_y / 100.0;
    let left = region.x / 100.0;
    let top = region.y / 100.0;
    let right = (region.x + region.width) / 100.0;
    let bottom = (region.y + region.height) / 100.0;
    let geometry = SampleGeometry::new(source_width, source_height);
    let motion_map = scene_motion_map(&frames, geometry);
    let motion = motion_map.as_deref().map(|map| MotionContext {
        map,
        camera: region_motion(map, geometry, left, top, right, bottom),
    });
    let reach_y = ((bottom - top) * 0.6).clamp(0.04, 0.13);
    let reach_x = ((right - left) * 0.6).clamp(0.04, 0.13);
    let inset_x = ((right - left) * 0.1).min(0.02);
    let horizontal_start = left + inset_x;
    let horizontal_end = right - inset_x;
    let best_top = find_boundary(
        &frames,
        motion,
        geometry,
        true,
        (top - reach_y).max(0.0),
        (face_y - 0.025).min(top + reach_y),
        horizontal_start,
        horizontal_end,
        top,
        face_y,
    );
    let best_bottom = find_boundary(
        &frames,
        motion,
        geometry,
        true,
        (face_y + 0.025).max(bottom - reach_y),
        (bottom + reach_y).min(1.0),
        horizontal_start,
        horizontal_end,
        bottom,
        face_y,
    );
    let y1 = best_top.unwrap_or(top);
    let y2 = if bottom > 0.975 && best_bottom.is_none_or(|edge| edge > 0.99) {
        1.0
    } else {
        best_bottom.unwrap_or(bottom)
    };
    let inset_y = ((y2 - y1) * 0.1).min(0.02);
    let vertical_start = y1 + inset_y;
    let vertical_end = y2 - inset_y;
    let best_left = find_boundary(
        &frames,
        motion,
        geometry,
        false,
        (left - reach_x).max(0.0),
        (face_x - 0.025).min(left + reach_x),
        vertical_start,
        vertical_end,
        left,
        face_x,
    );
    let best_right = find_boundary(
        &frames,
        motion,
        geometry,
        false,
        (face_x + 0.025).max(right - reach_x),
        (right + reach_x).min(1.0),
        vertical_start,
        vertical_end,
        right,
        face_x,
    );
    let left_attached =
        left < 0.015 && boundary_strength(&frames, geometry, true, y1, 0.005, 0.035) >= 11.0;
    let right_attached =
        right > 0.985 && boundary_strength(&frames, geometry, true, y1, 0.965, 0.995) >= 11.0;
    let x1 = if left_attached {
        0.0
    } else {
        best_left.unwrap_or(left)
    };
    let x2 = if right_attached || (right > 0.995 && best_right.is_none_or(|edge| edge > 0.98)) {
        1.0
    } else {
        best_right.unwrap_or(right)
    };
    // A texture edge inside the camera can be stronger than its actual border.
    // Reject implausibly small or large boxes and keep the face-only fallback.
    if x2 - x1 < 0.045 || y2 - y1 < 0.045 || x2 - x1 > 0.65 || y2 - y1 > 0.75 {
        return region;
    }
    region.x = x1 * 100.0;
    region.y = y1 * 100.0;
    region.width = (x2 - x1) * 100.0;
    region.height = (y2 - y1) * 100.0;
    region
}

#[derive(Clone, Copy)]
struct SampleGeometry {
    scaled_width: f32,
    scaled_height: f32,
    pad_x: f32,
    pad_y: f32,
}

impl SampleGeometry {
    fn new(source_width: u32, source_height: u32) -> Self {
        let scale =
            (INPUT_SIZE as f32 / source_width as f32).min(INPUT_SIZE as f32 / source_height as f32);
        let scaled_width = source_width as f32 * scale;
        let scaled_height = source_height as f32 * scale;
        Self {
            scaled_width,
            scaled_height,
            pad_x: (INPUT_SIZE as f32 - scaled_width) / 2.0,
            pad_y: (INPUT_SIZE as f32 - scaled_height) / 2.0,
        }
    }

    fn color(self, frame: &[u8], x: f32, y: f32) -> [u8; 3] {
        let px = (self.pad_x + x.clamp(0.0, 1.0) * self.scaled_width)
            .round()
            .clamp(0.0, (INPUT_SIZE - 1) as f32) as usize;
        let py = (self.pad_y + y.clamp(0.0, 1.0) * self.scaled_height)
            .round()
            .clamp(0.0, (INPUT_SIZE - 1) as f32) as usize;
        let offset = (py * INPUT_SIZE + px) * 3;
        [frame[offset], frame[offset + 1], frame[offset + 2]]
    }
}

#[allow(clippy::too_many_arguments)]
fn find_boundary(
    frames: &[&[u8]],
    motion: Option<MotionContext>,
    geometry: SampleGeometry,
    horizontal: bool,
    start: f32,
    end: f32,
    line_start: f32,
    line_end: f32,
    expected: f32,
    inside: f32,
) -> Option<f32> {
    if start >= end || line_start >= line_end {
        return None;
    }
    let resolution = if horizontal {
        geometry.scaled_height
    } else {
        geometry.scaled_width
    };
    let first = (start * resolution).ceil() as usize;
    let last = (end * resolution).floor() as usize;
    let mut candidates = Vec::new();
    for index in first..=last {
        let position = index as f32 / resolution;
        if !(0.004..=0.996).contains(&position) {
            continue;
        }
        let strength =
            boundary_strength(frames, geometry, horizontal, position, line_start, line_end);
        // Colour contrast cannot tell the webcam's own border from a stronger
        // line just outside it (the edge of a chat bar or of chat text). The
        // webcam side of the real border is calm, while a line beyond the
        // webcam has gameplay or scrolling chat on its inner side.
        let outside_penalty = motion.map_or(0.0, |context| {
            let inner = band_motion(
                context.map,
                geometry,
                horizontal,
                position,
                position > inside,
                line_start,
                line_end,
            );
            (inner - context.camera * 1.5 - 3.0).max(0.0) * 2.0
        });
        // A small prior favours the existing face-based estimate when several
        // similarly strong UI lines appear near the camera.
        let rank = strength - (position - expected).abs() * 35.0 - outside_penalty;
        candidates.push((position, strength, rank));
    }
    let &(position, strength, _) = candidates
        .iter()
        .max_by(|left, right| left.2.total_cmp(&right.2))?;
    (strength >= 11.0).then_some(position)
}

fn boundary_strength(
    frames: &[&[u8]],
    geometry: SampleGeometry,
    horizontal: bool,
    position: f32,
    line_start: f32,
    line_end: f32,
) -> f32 {
    let resolution = if horizontal {
        geometry.scaled_height
    } else {
        geometry.scaled_width
    };
    let mut frame_scores = Vec::with_capacity(frames.len());
    for frame in frames {
        let mut contrasts = Vec::with_capacity(24);
        for sample in 0..24 {
            let along = line_start + (line_end - line_start) * (sample as f32 + 0.5) / 24.0;
            let delta = 1.5 / resolution;
            let (before, after) = if horizontal {
                (
                    geometry.color(frame, along, position - delta),
                    geometry.color(frame, along, position + delta),
                )
            } else {
                (
                    geometry.color(frame, position - delta, along),
                    geometry.color(frame, position + delta, along),
                )
            };
            contrasts.push(
                before
                    .iter()
                    .zip(after)
                    .map(|(a, b)| a.abs_diff(b) as f32)
                    .sum::<f32>()
                    / 3.0,
            );
        }
        // A webcam border continues along the line. Averaging alone lets a
        // bright curved game/UI edge win even when most of the line is not a
        // boundary. Require support across the line, not just a few pixels.
        contrasts.sort_by(f32::total_cmp);
        frame_scores.push(contrasts[6] * 0.5 + median(contrasts.into_iter()) * 0.5);
    }
    median(frame_scores.into_iter())
}

/// Per-pixel temporal change across the sampled frames (mean absolute
/// deviation from the per-pixel median, in grey levels).
fn motion_map(frames: &[&[u8]]) -> Vec<f32> {
    let pixels = INPUT_SIZE * INPUT_SIZE;
    let mut map = vec![0.0_f32; pixels];
    if frames.len() < 3 {
        return map;
    }
    let mut values = Vec::with_capacity(frames.len());
    for (pixel, slot) in map.iter_mut().enumerate() {
        values.clear();
        for frame in frames {
            let offset = pixel * 3;
            values.push(
                frame[offset] as f32 * 0.299
                    + frame[offset + 1] as f32 * 0.587
                    + frame[offset + 2] as f32 * 0.114,
            );
        }
        values.sort_by(f32::total_cmp);
        let middle = values[values.len() / 2];
        *slot = values
            .iter()
            .map(|value| (value - middle).abs())
            .sum::<f32>()
            / values.len() as f32;
    }
    map
}

/// The motion map is only meaningful when the content actually moves between
/// samples. Paused games, menus and slideshows fall back to colour edges.
fn scene_motion_map(frames: &[&[u8]], geometry: SampleGeometry) -> Option<Vec<f32>> {
    if frames.len() < 5 {
        return None;
    }
    let map = motion_map(frames);
    let mut samples = Vec::with_capacity(32 * 18);
    for row in 0..18 {
        for column in 0..32 {
            let x = (column as f32 + 0.5) / 32.0;
            let y = (row as f32 + 0.5) / 18.0;
            samples.push(motion_at(&map, geometry, x, y));
        }
    }
    (median(samples.into_iter()) >= 3.0).then_some(map)
}

fn motion_at(map: &[f32], geometry: SampleGeometry, x: f32, y: f32) -> f32 {
    let px = (geometry.pad_x + x.clamp(0.0, 1.0) * geometry.scaled_width)
        .round()
        .clamp(0.0, (INPUT_SIZE - 1) as f32) as usize;
    let py = (geometry.pad_y + y.clamp(0.0, 1.0) * geometry.scaled_height)
        .round()
        .clamp(0.0, (INPUT_SIZE - 1) as f32) as usize;
    map[py * INPUT_SIZE + px]
}

#[derive(Clone, Copy)]
struct MotionContext<'a> {
    map: &'a [f32],
    /// Typical motion inside the face-based webcam guess.
    camera: f32,
}

fn region_motion(
    map: &[f32],
    geometry: SampleGeometry,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> f32 {
    let mut samples = Vec::with_capacity(16 * 12);
    for row in 0..12 {
        for column in 0..16 {
            let x = left + (right - left) * (column as f32 + 0.5) / 16.0;
            let y = top + (bottom - top) * (row as f32 + 0.5) / 12.0;
            samples.push(motion_at(map, geometry, x, y));
        }
    }
    median(samples.into_iter())
}

/// Median motion of a thin band beside a line, on the side that would be
/// inside the webcam if the line were its border.
fn band_motion(
    map: &[f32],
    geometry: SampleGeometry,
    horizontal: bool,
    position: f32,
    before: bool,
    line_start: f32,
    line_end: f32,
) -> f32 {
    let resolution = if horizontal {
        geometry.scaled_height
    } else {
        geometry.scaled_width
    };
    let direction = if before { -1.0 } else { 1.0 };
    let mut samples = Vec::with_capacity(24);
    for sample in 0..24 {
        let along = line_start + (line_end - line_start) * (sample as f32 + 0.5) / 24.0;
        let mut total = 0.0;
        for step in 2..=7 {
            let offset = direction * step as f32 / resolution;
            total += if horizontal {
                motion_at(map, geometry, along, position + offset)
            } else {
                motion_at(map, geometry, position + offset, along)
            };
        }
        samples.push(total / 6.0);
    }
    median(samples.into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(x: f32, y: f32) -> NormalizedFace {
        NormalizedFace {
            x,
            y,
            width: 0.08,
            height: 0.14,
            confidence: 0.9,
        }
    }

    fn confident_face(x: f32, y: f32, confidence: f32) -> NormalizedFace {
        NormalizedFace {
            confidence,
            ..face(x, y)
        }
    }

    #[test]
    fn timestamps_cover_the_video_without_sampling_the_edges() {
        let timestamps = sample_timestamps(100.0);
        assert_eq!(timestamps.len(), SAMPLE_COUNT);
        assert!((timestamps[0] - 6.0).abs() < 0.01);
        assert!((timestamps[10] - 94.0).abs() < 0.01);
    }

    #[test]
    fn stable_corner_face_wins_over_moving_content_faces() {
        let frames = (0..9)
            .map(|index| FrameFaces {
                index,
                faces: vec![
                    face(0.84 + index as f32 * 0.0005, 0.75),
                    face(0.10 + index as f32 * 0.07, 0.25),
                ],
            })
            .collect();
        let result = choose_camera_region(frames, 1920, 1080).unwrap();
        assert_eq!(result.matched_samples, 9);
        assert!(result.x > 45.0);
        assert!(result.y > 40.0);
        assert!(result.y < 75.0);
        assert!(result.y + result.height < 99.5);
        assert!(result.width > 30.0 && result.width < 32.0);
        assert!(result.confidence > 0.8);
    }

    #[test]
    fn no_result_is_returned_without_repeated_detection() {
        let frames = (0..SAMPLE_COUNT)
            .map(|index| FrameFaces {
                index,
                faces: vec![face(index as f32 * 0.1, 0.2)],
            })
            .collect();
        assert!(choose_camera_region(frames, 1920, 1080).is_none());
    }

    #[test]
    fn camera_selection_works_in_each_screen_corner() {
        for (x, y) in [(0.03, 0.04), (0.84, 0.04), (0.03, 0.76), (0.84, 0.76)] {
            let frames = (0..SAMPLE_COUNT)
                .map(|index| FrameFaces {
                    index,
                    faces: vec![face(x + index as f32 * 0.0003, y)],
                })
                .collect();
            let result = choose_camera_region(frames, 1920, 1080).unwrap();
            assert_eq!(result.matched_samples, SAMPLE_COUNT);
            assert!((result.focal_x / 100.0 - (x + 0.04)).abs() < 0.02);
            assert!((result.focal_y / 100.0 - (y + 0.07)).abs() < 0.02);
        }
    }

    #[test]
    fn persistent_camera_beats_a_brief_high_confidence_content_face() {
        let frames = (0..SAMPLE_COUNT)
            .map(|index| {
                let mut faces = vec![confident_face(0.83, 0.74, 0.78)];
                if (4..=6).contains(&index) {
                    faces.push(confident_face(0.42, 0.28, 0.99));
                }
                FrameFaces { index, faces }
            })
            .collect();
        let result = choose_camera_region(frames, 1920, 1080).unwrap();
        assert!(result.focal_x > 80.0);
        assert!(result.focal_y > 70.0);
        assert_eq!(result.matched_samples, SAMPLE_COUNT);
    }

    #[test]
    fn one_outlier_does_not_pull_the_camera_crop_away() {
        let frames = (0..SAMPLE_COUNT)
            .map(|index| FrameFaces {
                index,
                faces: vec![if index == SAMPLE_COUNT / 2 {
                    face(0.115, 0.10)
                } else {
                    face(0.04 + index as f32 * 0.0002, 0.05)
                }],
            })
            .collect();
        let result = choose_camera_region(frames, 1920, 1080).unwrap();
        assert!(result.focal_x < 12.0);
        assert!(result.focal_y < 16.0);
    }

    fn synthetic_camera_frame(rect: (f32, f32, f32, f32), frame_index: usize) -> Vec<u8> {
        let mut rgb = vec![0_u8; INPUT_SIZE * INPUT_SIZE * 3];
        for y in 140..500 {
            for x in 0..640 {
                let nx = x as f32 / 640.0;
                let ny = (y - 140) as f32 / 360.0;
                let inside = nx >= rect.0 && nx < rect.2 && ny >= rect.1 && ny < rect.3;
                let offset = (y * INPUT_SIZE + x) * 3;
                let moving = ((x + frame_index * 9) % 43) as u8;
                let base = if inside { 105 } else { 24 };
                rgb[offset] = base + moving / 4;
                rgb[offset + 1] = base + moving / 5;
                rgb[offset + 2] = base + moving / 6;
            }
        }
        rgb
    }

    #[test]
    fn border_refinement_works_in_different_positions_and_aspect_ratios() {
        let cases = [
            (0.06, 0.08, 0.29, 0.31),
            (0.70, 0.06, 0.96, 0.29),
            (0.11, 0.72, 0.35, 0.96),
            (0.81, 0.75, 1.0, 1.0),
            (0.39, 0.41, 0.62, 0.68),
        ];
        for rect in cases {
            let frames: Vec<_> = (0..3)
                .map(|index| synthetic_camera_frame(rect, index))
                .collect();
            let focal_x = (rect.0 + rect.2) / 2.0;
            let focal_y = (rect.1 + rect.3) / 2.0;
            let initial = CameraDetectionResult {
                x: ((rect.0 - 0.035).max(0.0)) * 100.0,
                y: ((rect.1 - 0.035).max(0.0)) * 100.0,
                width: ((rect.2 + 0.035).min(1.0) - (rect.0 - 0.035).max(0.0)) * 100.0,
                height: ((rect.3 + 0.035).min(1.0) - (rect.1 - 0.035).max(0.0)) * 100.0,
                focal_x: focal_x * 100.0,
                focal_y: focal_y * 100.0,
                confidence: 0.9,
                samples: 3,
                matched_samples: 3,
            };
            let result = refine_camera_region(initial, &frames, 1920, 1080);
            assert!(
                (result.x / 100.0 - rect.0).abs() < 0.012,
                "left: {result:?}"
            );
            assert!((result.y / 100.0 - rect.1).abs() < 0.012, "top: {result:?}");
            assert!(
                ((result.x + result.width) / 100.0 - rect.2).abs() < 0.012,
                "right: {result:?}"
            );
            assert!(
                ((result.y + result.height) / 100.0 - rect.3).abs() < 0.012,
                "bottom: {result:?}"
            );
        }
    }

    #[test]
    fn partial_bright_content_edge_does_not_beat_a_continuous_camera_border() {
        let mut frame = synthetic_camera_frame((0.81, 0.75, 1.0, 1.0), 0);
        // A bright content strip crosses only the bottom part of the search
        // line, to the left of the actual webcam. Its average contrast is
        // stronger than the webcam edge, but it is not a panel boundary.
        for y in 470..500 {
            for x in 454..462 {
                let offset = (y * INPUT_SIZE + x) * 3;
                frame[offset..offset + 3].fill(255);
            }
        }
        let frames = vec![frame.as_slice()];
        let boundary = find_boundary(
            &frames,
            None,
            SampleGeometry::new(1920, 1080),
            false,
            0.65,
            0.87,
            0.77,
            0.98,
            0.70,
            0.90,
        )
        .unwrap();
        assert!((boundary - 0.81).abs() < 0.012, "{boundary}");
    }

    #[test]
    fn tile_faces_map_to_the_whole_frame_and_merge_duplicates() {
        let whole = vec![confident_face(0.80, 0.80, 0.70)];
        let tiles = vec![
            // Bottom-right tile sees the same face at higher confidence.
            (
                3,
                vec![NormalizedFace {
                    x: (0.80 - (1.0 - TILE_FRACTION)) / TILE_FRACTION,
                    y: (0.80 - (1.0 - TILE_FRACTION)) / TILE_FRACTION,
                    width: 0.08 / TILE_FRACTION,
                    height: 0.14 / TILE_FRACTION,
                    confidence: 0.95,
                }],
            ),
            // A small face only the top-left tile can see.
            (0, vec![confident_face(0.10, 0.20, 0.90)]),
            // A face cut by the inner edge of the top-left tile is ignored.
            (
                0,
                vec![NormalizedFace {
                    x: 0.95,
                    y: 0.40,
                    width: 0.05,
                    height: 0.10,
                    confidence: 0.9,
                }],
            ),
        ];
        let merged = merge_tile_faces(whole, tiles);
        assert_eq!(merged.len(), 2);
        assert!((merged[0].x - 0.80).abs() < 1e-4 && merged[0].confidence > 0.9);
        assert!((merged[1].x - 0.10 * TILE_FRACTION).abs() < 1e-4);
        assert!((merged[1].width - 0.08 * TILE_FRACTION).abs() < 1e-4);
    }

    #[test]
    fn bundled_model_loads_and_accepts_a_frame() {
        let mut detector = YuNet::load().unwrap();
        let blank = vec![0_u8; INPUT_SIZE * INPUT_SIZE * 3];
        assert!(detector.detect(&blank).unwrap().is_empty());
    }
}
