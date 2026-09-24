import { strict as assert } from "node:assert";
import { updatesAllowedForVersion } from "../src/lib/releaseChannel.ts";

assert.equal(updatesAllowedForVersion("0.16.0"), true, "stable builds use the production updater");
assert.equal(updatesAllowedForVersion("0.16.0-dev.1"), false, "DEV builds stay off the production updater");
assert.equal(updatesAllowedForVersion("0.16.0-beta.1"), false, "prerelease builds stay off the production updater");
assert.equal(updatesAllowedForVersion(""), false, "unknown versions do not query the updater");

console.log("release channel scenarios: 4 passed");
