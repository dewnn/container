<script lang="ts">
  import type { Tool } from "../tools";
  import kickBanner from "../../../src-tauri/resources/social-tags/kick-banner.png";
  import kickMark from "../../assets/kick-mark.svg";
  import twitchMark from "../../assets/twitch-mark.svg";
  let { language, clipperOutputVisible, multiRegionClipper, selected, socialBannerPreviewStyle, socialTagPreviewStyle, toolValue }:{ language:"tr"|"en"; clipperOutputVisible:boolean; multiRegionClipper:boolean; selected:Tool|null; socialBannerPreviewStyle:()=>string; socialTagPreviewStyle:()=>string; toolValue:(key:string)=>string }=$props();
</script>

{#if selected?.id==="clipper"&&(!multiRegionClipper||clipperOutputVisible)&&toolValue("social_tag_enabled")==="true"&&toolValue("social_tag_username").trim()}
  {#if toolValue("social_tag_style")==="kick_banner"}
    <div class="clipper-social-banner" style={socialBannerPreviewStyle()} aria-label={language==="tr"?"Kick.com bandı önizlemesi":"Kick.com banner preview"}>
      <span class="clipper-social-banner-bg"></span>
      <span class="clipper-social-banner-logo"><img class="clipper-social-banner-logo-art" src={kickBanner} alt="" /></span>
      <span class="clipper-social-banner-prefix"><img class="clipper-social-banner-prefix-art" src={kickBanner} alt="" /></span>
      <span class="clipper-social-banner-name">{toolValue("social_tag_username").trim().toUpperCase()}</span>
    </div>
  {:else}
    <div class="clipper-social-tag" class:boxed={toolValue("social_tag_style")==="boxed"} class:twitch={toolValue("social_tag_platform")==="twitch"} style={socialTagPreviewStyle()}>
      <span class="clipper-social-icon"><img src={toolValue("social_tag_platform")==="twitch"?twitchMark:kickMark} alt="" /></span>
      <span class="clipper-social-name">{toolValue("social_tag_username").trim()}</span>
    </div>
  {/if}
{/if}
