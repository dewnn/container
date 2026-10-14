// Shared editor types used by App.svelte and its panels.
import type { MediaKind, Tool } from "./tools";
import type { ProjectResource } from "./projectResources";
import type { StageHistory } from "./stageHistory";

export interface MediaInfo {
  path: string;
  name: string;
  kind: MediaKind;
  duration: number | null;
  width: number | null;
  height: number | null;
  fps: number | null;
  codec: string;
  audio_codec: string | null;
  audio_tracks: { index:number; codec:string; channels:number|null; channel_layout:string|null; language:string|null; bitrate:number|null; is_default:boolean }[];
  pixel_format: string | null;
  bits_per_raw_sample: number | null;
  color_transfer: string | null;
  color_primaries: string | null;
  color_space: string | null;
  bitrate: number | null;
  size: number;
  start_timecode: string | null;
}
export interface ProgressEvent { job_id?: string; percent: number; time: number; speed: string; frame: string; status: string }
export interface JobResult { output: string; elapsed: number }
export interface QualityCandidate { crf: number; vmaf: number; estimated_size_mb: number; rating: string }
export interface QualityAnalysis { recommended_crf: number; target_vmaf: number; candidates: QualityCandidate[]; sample_count: number; sampled_seconds: number; elapsed: number }
export interface CameraDetectionResult { x:number; y:number; width:number; height:number; focal_x:number; focal_y:number; confidence:number; samples:number; matched_samples:number }
export interface FfmpegStatus { ready: boolean; ffmpeg_version: string | null; ffprobe_version: string | null }
export interface FfmpegCapabilities { vidstab:boolean; subtitles:boolean; overlay:boolean; blur:boolean; concat:boolean }
export interface DownloaderStatus { ready:boolean; version:string|null }
export interface SubtitleTrack { index:number; codec:string; language:string|null; title:string|null }
export interface OutputCleanupResult { cleaned:boolean; path:string }
export interface FontOption { name:string; path:string }
export interface TextLayer { id:number; text:string; x:number; y:number; size:number; wrap_width?:number; color:string; opacity:number; align:"left"|"center"|"right"; fontName:string; font_path:string; outline:number; outline_color:string; shadow:number; shadow_color:string; background:boolean; background_color:string; background_opacity:number; background_padding:number }
export interface StackStep { id:number; tool:Tool; params:Record<string,string>; textLayers:TextLayer[]; colorEnabled:Record<string,boolean>; enabled:boolean; smartcutSession?:unknown; sourceWidth?:number }
export interface EditorSnapshot { media:MediaInfo; mediaUrl:string; selected:Tool|null; activeKind:MediaKind; output:string; outputSettingsKey?:string; outputMode?:"tool"|"stack"; renderedImageUrl:string; colorEnabled:Record<string,boolean>; colorPreviewVisible:boolean; textLayers:TextLayer[]; activeTextId:number|null; qualityAnalysis:QualityAnalysis|null; customNumberFields:Record<string,boolean>; mergeInputs?:string[]; processingStack?:StackStep[]; stackQuality?:string; editingStackStepId?:number|null }
export interface RecoverySession { version:1; savedAt:number; mediaPath:string; workspaceMode:"toolbox"|"autocut"|"batch"; toolbox:EditorSnapshot|null; autocut:unknown; batch:unknown; resources?:ProjectResource[]; stageHistory?:StageHistory<RecoverySession> }
