import { moveTimelineBoundary, type TimelineBoundary, type TimelineRange } from "../src/lib/timelineRange.ts";

interface Scenario {
  name: string;
  range: TimelineRange;
  boundary: TimelineBoundary;
  at: number;
  duration: number;
  expected: TimelineRange;
}

const scenarios: Scenario[] = [
  { name:"IN inside range", range:{start:5,end:20}, boundary:"start", at:8, duration:50, expected:{start:8,end:20} },
  { name:"OUT inside range", range:{start:5,end:20}, boundary:"end", at:15, duration:50, expected:{start:5,end:15} },
  { name:"IN beyond OUT shifts range", range:{start:5,end:15}, boundary:"start", at:20, duration:50, expected:{start:20,end:30} },
  { name:"OUT before IN shifts range", range:{start:20,end:30}, boundary:"end", at:10, duration:50, expected:{start:0,end:10} },
];

for (const scenario of scenarios) {
  const actual = moveTimelineBoundary(scenario.range, scenario.boundary, scenario.at, scenario.duration);
  if (Math.abs(actual.start-scenario.expected.start)>1e-9 || Math.abs(actual.end-scenario.expected.end)>1e-9) {
    throw new Error(`${scenario.name}: expected ${JSON.stringify(scenario.expected)}, got ${JSON.stringify(actual)}`);
  }
}

for (const [name,range,boundary,at,duration] of [
  ["IN at media end",{start:35,end:45},"start",50,50],
  ["OUT at media start",{start:5,end:15},"end",0,50],
] as const) {
  const actual=moveTimelineBoundary(range,boundary,at,duration);
  if(actual.start<0||actual.end>duration||actual.start>=actual.end)throw new Error(`${name}: invalid ${JSON.stringify(actual)}`);
}

console.log("timeline Cut/GIF interaction scenarios: 6 passed");
