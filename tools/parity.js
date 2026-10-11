#!/usr/bin/env node
// Parity check: runs the same scenarios as `cargo run -p gears_cli -- all` through the
// JavaScript port (web/gears_core.js) and prints the [GEARS] lines in the same format.
//   node tools/parity.js > /tmp/js.txt
//   cargo run -q -p gears_cli -- all | grep GEARS > /tmp/rs.txt
//   diff /tmp/rs.txt /tmp/js.txt      (no output = the port matches)
const path = require("path");
const G = require(path.join(__dirname, "..", "web", "gears_core.js"));

const DT = 1 / 60;
const inp = (x, y, a) => ({ stick_x: x, stick_y: y, cam_yaw: 0, a });

function run(name, world, start, steps, end) {
  const c = new G.Character(G.defaultConfig(), start, 0);
  const out = [];
  while (c.time < end) {
    const t = c.time;
    const s = steps.find((s) => t < s.until);
    const before = c.events.length;
    c.tick(s ? s.input : inp(0, 0, false), DT, world);
    for (const e of c.events.slice(before)) {
      out.push(`[GEARS] t=${e.time.toFixed(3).padStart(6)} ${e.from} -> ${e.to} (${e.reason})`);
    }
  }
  return out;
}

function fuzz(seed, frames) {
  const world = G.WallWorld.twoSided([
    { a: G.V(300, -200), b: G.V(300, 200), normal: G.V(-1, 0), height: "Mid" },
    { a: G.V(-300, -200), b: G.V(-300, 200), normal: G.V(1, 0), height: "Mid" },
    { a: G.V(-150, 400), b: G.V(150, 400), normal: G.V(0, -1), height: "Standing" },
    { a: G.V(300, 260), b: G.V(300, 460), normal: G.V(-1, 0), height: "Mid" },
  ], 24);
  const c = new G.Character(G.defaultConfig(), G.V(0, 0), 0);
  let state = seed >>> 0;
  const next = () => { state = (Math.imul(state, 1664525) + 1013904223) >>> 0; return state >>> 16; };
  let input = inp(0, 0, false);
  const out = [];
  for (let frame = 0; frame < frames; frame++) {
    if (frame % 6 === 0) {
      const r = next();
      input = inp((r % 9) / 4 - 1, (Math.floor(r / 9) % 9) / 4 - 1, Math.floor(r / 81) % 3 !== 0);
    }
    const before = c.events.length;
    c.tick(input, DT, world);
    for (const e of c.events.slice(before)) {
      out.push(`[GEARS] t=${e.time.toFixed(3).padStart(6)} ${e.from} -> ${e.to} (${e.reason})`);
    }
  }
  out.push(`final pos=(${c.pos.x.toFixed(1)},${c.pos.y.toFixed(1)}) state=${c.stateName()}`);
  return out;
}

if (process.argv[2] === "fuzz") {
  console.log(fuzz(parseInt(process.argv[3] || "1", 10), 3600).join("\n"));
  process.exit(0);
}

const wall = new G.WallWorld([{ a: G.V(300, -200), b: G.V(300, 200), normal: G.V(-1, 0), height: "Mid" }]);
const lines = [
  ...run("slide", wall, G.V(0, 0), [{ until: 0.1, input: inp(0, 1, true) }, { until: 2, input: inp(0, 0, false) }], 1.0),
  ...run("chain", wall, G.V(0, 150), [
    { until: 0.1, input: inp(0, 1, true) },
    { until: 0.2, input: inp(0, 1, false) },
    { until: 3, input: inp(0, 1, true) },
  ], 2.0),
  ...run("roadie", wall, G.V(-200, 0), [{ until: 3, input: inp(0, 1, true) }], 1.6),
];
console.log(lines.join("\n"));
