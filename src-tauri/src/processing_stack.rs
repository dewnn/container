use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Deserialize)]
pub(crate) struct Step {
    pub operation: String,
    pub params: HashMap<String,String>,
    pub enabled: bool,
}

pub(crate) type SourceRanges = Vec<(f64, f64)>;

// Only a quality-verified automatic H.264 profile may replace High's CPU
// encoder. Filter/input/audio arguments and the destination stay untouched.
pub(crate) fn verified_high_args(args:Vec<String>,profile:&[String])->Vec<String>{
    if !profile.windows(2).any(|pair|pair[0]=="-c:v"&&pair[1]!="libx264"){return args}
    let mut result=Vec::new();let mut index=0;
    while index<args.len(){
        if matches!(args[index].as_str(),"-c:v"|"-crf"|"-preset"|"-pix_fmt"){index+=2;}
        else{result.push(args[index].clone());index+=1;}
    }
    if let Some(destination)=result.pop(){result.extend_from_slice(profile);result.push(destination);}
    result
}

// Fuse only the supported, single-input SmartCut -> Clipper path. Other
// stacks retain the established lossless-intermediate implementation.
pub(crate) fn fuse_smartcut_clipper(args:Vec<String>,cuts_graph:&str,has_audio:bool)->Option<Vec<String>>{
    if args.iter().filter(|arg|arg.as_str()=="-i").count()!=1||args.iter().any(|arg|arg=="-filter_complex"){return None}
    let position=args.iter().position(|arg|arg=="-vf")?;
    let filter=args.get(position+1)?;
    let graph=format!("{};[stack_source]{filter}[stack_video]",cuts_graph.trim().trim_end_matches(';').replace("[vout]","[stack_source]"));
    let mut result=Vec::new();let mut index=0;
    while index<args.len(){
        if matches!(args[index].as_str(),"-vf"|"-map"|"-c:a"|"-codec:a"|"-b:a"){index+=2;}
        else{result.push(args[index].clone());index+=1;}
    }
    let destination=result.pop()?;
    result.extend(["-filter_complex".into(),graph,"-map".into(),"[stack_video]".into()]);
    if has_audio{result.extend(["-map".into(),"[aout]".into(),"-c:a".into(),"aac".into(),"-b:a".into(),"192k".into()]);}
    else{result.push("-an".into());}
    result.push(destination);Some(result)
}

// Stack editors use the original source timeline. Translate a source window
// through previously kept ranges instead of treating it as shortened time.
pub(crate) fn source_window(ranges:&[(f64,f64)],start:f64,end:f64)->Result<(SourceRanges,SourceRanges),String>{
    if !start.is_finite()||!end.is_finite()||start<0.0||end<=start{return Err("Invalid source cut range.".into())}
    let mut current=Vec::new();
    let mut kept=Vec::new();
    let mut offset=0.0;
    for &(a,b) in ranges{
        if !a.is_finite()||!b.is_finite()||a<0.0||b<=a{return Err("Invalid Stack source timeline.".into())}
        let from=a.max(start);let to=b.min(end);
        if to>from{current.push((offset+from-a,offset+to-a));kept.push((from,to));}
        offset+=b-a;
    }
    if current.is_empty(){return Err("This source range was removed by an earlier Stack cut. Choose a kept source range.".into())}
    Ok((current,kept))
}

pub(crate) fn output_args(mut args:Vec<String>,destination:&Path,intermediate:bool,quality:Option<&str>)->Result<Vec<String>,String>{
    if args.pop().is_none(){return Err("Stack stage has no output argument.".into())}
    // Encoder profiles/rate control belong to the stage's original codec.
    // Retaining e.g. H.264's profile=high breaks FFV1, and retaining qp=0
    // overrides the user's final CRF quality selection.
    if intermediate||quality.is_some(){
        let mut cleaned=Vec::with_capacity(args.len());
        let mut index=0;
        while index<args.len(){
            let video_option=matches!(args[index].as_str(),"-c:v"|"-codec:v"|"-vcodec"|"-profile:v"|"-level:v"|"-level"|"-qp"|"-crf"|"-preset"|"-tune"|"-b:v"|"-maxrate"|"-bufsize"|"-rc"|"-cq"|"-global_quality");
            let audio_option=intermediate&&matches!(args[index].as_str(),"-c:a"|"-codec:a"|"-acodec"|"-b:a");
            if video_option||audio_option{if index+1>=args.len(){return Err("Stack encoder option is missing its value.".into())}index+=2;}
            else{cleaned.push(args[index].clone());index+=1;}
        }
        args=cleaned;
    }
    if intermediate{
        args.extend(["-c:v".into(),"ffv1".into(),"-level".into(),"3".into(),"-c:a".into(),"flac".into(),"-f".into(),"matroska".into()]);
    }else if let Some(quality)=quality{
        if quality=="lossless"{args.extend(["-c:v".into(),"libx264".into(),"-qp".into(),"0".into()]);}
        else{let crf=match quality{"high"=>"14","medium"=>"22","small"=>"26",_=>return Err("Invalid Stack quality.".into())};args.extend(["-c:v".into(),"libx264".into(),"-crf".into(),crf.into(),"-preset".into(),"veryfast".into()]);}
    }
    args.push(destination.to_string_lossy().into_owned());
    Ok(args)
}

#[cfg(test)]
mod tests{
    #[test]
    fn verified_high_profile_preserves_graph_audio_and_destination(){
        let args=vec!["-i","source.mp4","-filter_complex","[0:v]null[v]","-c:v","libx264","-crf","14","-preset","veryfast","-pix_fmt","yuv420p","-c:a","aac","out.mp4"].into_iter().map(String::from).collect::<Vec<_>>();
        let cpu=vec!["-c:v","libx264"].into_iter().map(String::from).collect::<Vec<_>>();
        assert_eq!(super::verified_high_args(args.clone(),&cpu),args);
        let gpu=vec!["-c:v","h264_amf","-quality","speed","-rc","cqp","-qp_i","14","-qp_p","14","-pix_fmt","nv12"].into_iter().map(String::from).collect::<Vec<_>>();
        let result=super::verified_high_args(args,&gpu);
        assert!(result.windows(gpu.len()).any(|window|window==gpu));
        assert!(result.windows(2).any(|pair|pair==["-filter_complex","[0:v]null[v]"]));
        assert!(result.windows(2).any(|pair|pair==["-c:a","aac"]));
        assert_eq!(result.last().unwrap(),"out.mp4");
        assert!(!result.iter().any(|arg|arg=="-crf"||arg=="-preset"));
    }
    #[test]
    fn fused_pair_preserves_input_quality_and_maps_trimmed_audio(){
        let args=vec!["-i","kaynak ğ & (1).mp4","-vf","scale=90:160","-c:v","libx264","-crf","14","-c:a","copy","out.mp4"].into_iter().map(String::from).collect::<Vec<_>>();
        let graph="[0:v]null[vout];[0:a]anull[aout];";
        let fused=super::fuse_smartcut_clipper(args.clone(),graph,true).unwrap();
        assert_eq!(fused.iter().filter(|value|value.as_str()=="-i").count(),1);
        assert!(fused.windows(2).any(|pair|pair==["-i","kaynak ğ & (1).mp4"]));
        assert!(fused.windows(2).any(|pair|pair==["-map","[aout]"]));
        assert!(fused.windows(2).any(|pair|pair==["-crf","14"]));
        assert!(!fused.iter().any(|value|value=="-vf"||value=="copy"));
        assert_eq!(fused.last().unwrap(),"out.mp4");
        let silent=super::fuse_smartcut_clipper(args.clone(),"[0:v]null[vout];",false).unwrap();
        assert!(silent.iter().any(|value|value=="-an"));
        assert!(!silent.iter().any(|value|value=="[aout]"));
        let mut unsupported=args;unsupported.extend(["-i".into(),"other.mp4".into()]);
        assert!(super::fuse_smartcut_clipper(unsupported,graph,true).is_none());
    }
    #[test]
    fn source_cut_maps_through_smartcut_and_repeated_cuts(){
        let ranges=vec![(2.0,8.0),(15.0,25.0)];
        let (current,kept)=super::source_window(&ranges,5.0,20.0).unwrap();
        assert_eq!(current,vec![(3.0,6.0),(6.0,11.0)]);
        assert_eq!(kept,vec![(5.0,8.0),(15.0,20.0)]);
        let (current,kept)=super::source_window(&kept,7.0,18.0).unwrap();
        assert_eq!(current,vec![(2.0,3.0),(3.0,6.0)]);
        assert_eq!(kept,vec![(7.0,8.0),(15.0,18.0)]);
        assert!(super::source_window(&ranges,9.0,14.0).is_err());
        assert!(super::source_window(&ranges,f64::NAN,20.0).is_err());
        assert_eq!(super::source_window(&ranges,0.0,60.0).unwrap().1,ranges);
    }
    #[test]
    fn stack_step_deserializes_without_extraneous_frontend_fields(){
        let step:super::Step=serde_json::from_str(r#"{"operation":"cut","params":{"start":"0","end":"1"},"enabled":true}"#).unwrap();
        assert_eq!(step.operation,"cut");
        assert_eq!(step.params["end"],"1");
        assert!(step.enabled);
    }
    #[test]
    fn stage_output_uses_lossless_intermediate_and_exact_target(){
        let target=std::path::Path::new("C:\\temp folder\\stage-0.mkv");
        let args=super::output_args(vec!["-i".into(),"source.mp4".into(),"old.mp4".into()],target,true,None).unwrap();
        assert!(args.windows(2).any(|pair|pair==["-c:v","ffv1"]));
        assert_eq!(args.last().unwrap(),&target.to_string_lossy());
    }
    #[test]
    fn replacing_codec_removes_incompatible_profile_and_rate_control(){
        let args=vec!["-i","source.mp4","-profile:v","high","-qp","0","-c:v","libx264","-c:a","aac","-b:a","192k","old.mp4"].into_iter().map(String::from).collect::<Vec<_>>();
        let intermediate=super::output_args(args.clone(),std::path::Path::new("stage.mkv"),true,None).unwrap();
        assert!(!intermediate.iter().any(|arg|arg=="-profile:v"||arg=="-qp"||arg=="-b:a"));
        assert!(intermediate.windows(2).any(|pair|pair==["-c:a","flac"]));
        let final_args=super::output_args(args,std::path::Path::new("final.mp4"),false,Some("medium")).unwrap();
        assert!(!final_args.iter().any(|arg|arg=="-profile:v"||arg=="-qp"));
        assert!(final_args.windows(2).any(|pair|pair==["-crf","22"]));
        assert!(final_args.windows(2).any(|pair|pair==["-c:a","aac"]));
    }
}
