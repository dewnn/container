import { recoveredMediaUrl } from "../src/lib/recovery.ts";

const fresh = "http://asset.localhost/C%3A/video.mp4";
const restored = recoveredMediaUrl(fresh, 1234);
if (restored !== `${fresh}?recovery=1234`) {
  throw new Error(`Unexpected recovered URL: ${restored}`);
}

const queried = recoveredMediaUrl(`${fresh}?prepared=1`, 5678);
if (queried !== `${fresh}?prepared=1&recovery=5678`) {
  throw new Error(`Existing query was not preserved: ${queried}`);
}

if (restored === fresh || queried.includes("??")) {
  throw new Error("Recovery must create a fresh, valid media resource identity.");
}

console.log("recovery media source scenarios: 3 passed");
