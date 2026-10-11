// gears_core.js — line-for-line JavaScript port of crates/gears_core (Rust), used by
// the browser sandbox. The Rust crate is the reference: tools/parity.js runs the
// same scripted inputs through both and diffs the [GEARS] logs. Keep them in sync.
// Units: Unreal units (uu), seconds, radians. X forward, Y right, Z up.
(function (root) {
  "use strict";
  const f = Math.fround; // emulate Rust f32 where timing decisions depend on it
  const PI = Math.PI, TAU = Math.PI * 2;

  // ---------------------------------------------------------------- math.rs
  const V = (x, y, z = 0) => ({ x, y, z });
  const add = (a, b) => V(a.x + b.x, a.y + b.y, a.z + b.z);
  const sub = (a, b) => V(a.x - b.x, a.y - b.y, a.z - b.z);
  const mul = (a, s) => V(a.x * s, a.y * s, a.z * s);
  const neg = (a) => V(-a.x, -a.y, -a.z);
  const dot = (a, b) => a.x * b.x + a.y * b.y + a.z * b.z;
  const len = (a) => Math.sqrt(dot(a, a));
  const len2d = (a) => Math.sqrt(a.x * a.x + a.y * a.y);
  const flat = (a) => V(a.x, a.y, 0);
  const norm = (a) => { const l = len(a); return l > 1e-6 ? mul(a, 1 / l) : V(0, 0, 0); };
  const fromYaw = (y) => V(Math.cos(y), Math.sin(y), 0);
  const yawOf = (a) => Math.atan2(a.y, a.x);
  const moveToward = (a, t, maxDelta) => {
    const d = sub(t, a), l = len(d);
    return l <= maxDelta || l < 1e-6 ? t : add(a, mul(d, maxDelta / l));
  };
  const urotToRad = (u) => (u * TAU) / 65536;
  const angleDiff = (a, b) => {
    let d = (b - a) % TAU;
    if (d > PI) d -= TAU; else if (d < -PI) d += TAU;
    return d;
  };

  // -------------------------------------------------------------- config.rs
  // Same values and sources as crates/gears_core/src/config.rs.
  const evadeAnim = (sf, sd, ef, ed) => ({ start_length: f(sf / 30), start_dist: sd, end_length: f(ef / 30), end_dist: ed });
  function defaultConfig() {
    return {
      ground_speed: 300, accel_rate: 2048, dead_zone: 0.2, collision_radius: 34, walking_pct: 0.5,
      roadie_speed_mod: 1.6, roadie_steering_pct: 0.33, roadie_hold_time: 0.3, turn_rate: 6.0,
      run2cover_speed_mod: 2.3, run2cover_min_duration: 0.2, run2cover_mirror_hold: 0.25,
      run2cover_max_dist: 420, run2cover_side_max_dist: 240, roadie_to_cover_dist: 60,
      seamless_cover_dot: -0.83, roadie_cover_dir_dot: 0.6, roadie_cover_after_move: 0.4,
      a_press_keeps_cover_time: 0.75, cover_run_speed_mod: 1.5,
      cover_move_pct_mid: 1.0, cover_move_pct_std: 1.0,
      break_from_cover_hold: 0.05, push_out_speed_mod: 0.75, push_out_anim_length: f(14 / 30),
      slip_speed_mod: 1.5, slip_max_edge_dist: 96, slip_stick_up: 0.5, slip_raw_stick_window: 0.3,
      slip_travel_side: 116.82, slip_travel_fwd: 75.18, slip_anim_length: f(0.7),
      swat_speed_mod: 1.7, swat_max_dist: 512, swat_camera_dot: 0.85, swat_to_roadie_delay: 0.2, swat_edge_dist: 96,
      swat_end_dist: 256, swat_anim_rate: 1.3, swat_end_anim_length: f(25 / 30),
      evade_speed_mod: 1.15, evade_retrigger_delay: 0.65, evade_chain_lead: 0.2,
      evade_cover_check_scale: 0.75,
      // AR_Evade_<Dir>_Start / _End: frames at 30 fps and root-motion travel (uu)
      evade_fwd: evadeAnim(12, 214.73, 22, 236.35), evade_bwd: evadeAnim(12, 204.96, 22, 270.60),
      evade_lt: evadeAnim(12, 214.24, 22, 257.37), evade_rt: evadeAnim(12, 215.54, 21, 244.31),
      // vault (GSM_MidLvlJumpOver / GSM_MantleOverCoverBase, AR_Mantle_Start/End, GearScout)
      vault_speed_mod: 1.1, vault_default_dist: 141, vault_min_root_scale: 0.5, vault_max_dist: 300,
      vault_slide_slot_dist: 96, vault_jump_frames: 21, vault_up_frames: 15, vault_over_frames: 20,
      vault_rise: 100.8, vault_jump_travel: 141.4, vault_fall_fwd_vel: 125, vault_fall_start_dist: 100,
      gravity: 750, collision_height: 78, vault_land_frames: 10, vault_roadie_after_land: 0.1,
    };
  }

  // --------------------------------------------------------------- world.rs
  const coverRight = (spot) => { const fw = neg(spot.normal); return norm(V(-fw.y, fw.x, 0)); };

  // ----------------------------------------------------------- testworld.rs
  class WallWorld {
    constructor(walls, standoff = 34) { this.walls = walls; this.standoff = standoff; }
    // every wall a slab `thickness` thick with cover on both faces
    static twoSided(walls, thickness) {
      const all = walls.slice();
      for (const w of walls) {
        const n = norm(w.normal);
        all.push({ a: sub(w.b, mul(n, thickness)), b: sub(w.a, mul(n, thickness)), normal: neg(n), height: w.height });
      }
      return new WallWorld(all);
    }
    findCover(from, dir, maxDist) {
      let best = null;
      for (const w of this.walls) {
        const n = norm(w.normal);
        const side = dot(sub(from, w.a), n);
        if (side <= 0 || dot(dir, neg(n)) <= 0.1) continue;
        const distToPlane = side - this.standoff;
        const t = Math.max(distToPlane / dot(dir, neg(n)), 0); // pressed against the wall: t = 0
        if (t > maxDist) continue;
        const hit = add(from, mul(dir, t));
        const seg = sub(w.b, w.a), slen = len(seg), sdir = norm(seg);
        const u = dot(sub(hit, w.a), sdir);
        if (u < 0 || u > slen) continue;
        const loc = add(add(w.a, mul(sdir, u)), mul(n, this.standoff));
        if (!this.isClear(from, loc, 0)) continue; // never through another wall
        const spot = this.spotOn(w, V(loc.x, loc.y, from.z));
        if (!best || t < best[0]) best = [t, spot];
      }
      return best ? best[1] : null;
    }
    spotOn(w, loc) {
      const n = norm(w.normal);
      const spot = {
        location: loc, normal: n, height: w.height || "Mid",
        dist_to_left_edge: 0, dist_to_right_edge: 0, can_slip_left: true, can_slip_right: true,
      };
      const r = coverRight(spot);
      const toA = dot(sub(w.a, loc), r), toB = dot(sub(w.b, loc), r);
      const [re, le] = toA > toB ? [toA, -toB] : [toB, -toA];
      spot.dist_to_left_edge = Math.max(le, 0);
      spot.dist_to_right_edge = Math.max(re, 0);
      return spot;
    }
    findSwatTarget(spot, edge, side, maxDist) {
      let best = null;
      for (const w of this.walls) {
        const n = norm(w.normal);
        if (dot(n, spot.normal) < 0.7) continue;
        const pa = dot(sub(w.a, edge), side), pb = dot(sub(w.b, edge), side);
        if (Math.min(pa, pb) <= 1) continue;
        const [near, along] = pa < pb ? [w.a, pa] : [w.b, pb];
        const across = dot(sub(near, edge), spot.normal) + this.standoff;
        if (along > maxDist || Math.abs(across) > 64) continue;
        const l0 = add(near, mul(n, this.standoff)), loc = V(l0.x, l0.y, spot.location.z);
        if (!this.isClear(edge, loc, 0)) continue;
        if (!best || along < best[0]) best = [along, this.spotOn(w, loc)];
      }
      return best ? best[1] : null;
    }
    // native GearPawn.FindMantleDistance (our version): far face of the mid cover ahead + radius
    mantleDistance(from, dir, maxDist, radius) {
      const to = add(from, mul(dir, maxDist)), hits = [];
      for (const w of this.walls) {
        const n = norm(w.normal);
        const s0 = dot(sub(from, w.a), n), s1 = dot(sub(to, w.a), n);
        if ((s0 > 0) === (s1 > 0)) continue;
        const t = s0 / (s0 - s1);
        const p = add(from, mul(sub(to, from), t));
        const seg = sub(w.b, w.a), u = dot(sub(p, w.a), norm(seg));
        if (u >= 0 && u <= len(seg)) hits.push([t * maxDist, w]);
      }
      hits.sort((a, b) => a[0] - b[0]);
      if (!hits.length) return null;
      const [first, w0] = hits[0];
      if ((w0.height || "Mid") !== "Mid" || dot(w0.normal, dir) >= 0) return null;
      const h1 = hits[1];
      const far = h1 && (h1[1].height || "Mid") === "Mid" && dot(h1[1].normal, dir) > 0 ? h1[0] : first;
      const dist = f(far + radius);
      return dist <= maxDist ? dist : null;
    }
    isClear(from, to, radius) {
      for (const w of this.walls) {
        const n = norm(w.normal);
        const s0 = dot(sub(from, w.a), n), s1 = dot(sub(to, w.a), n);
        if ((s0 > radius) === (s1 > radius) && (s0 < -radius) === (s1 < -radius)) continue;
        const t = s0 / (s0 - s1);
        const p = add(from, mul(sub(to, from), t));
        const seg = sub(w.b, w.a);
        const u = dot(sub(p, w.a), norm(seg));
        if (u >= 0 && u <= len(seg)) return false;
      }
      return true;
    }
  }

  // ----------------------------------------------------------- character.rs
  const isEvade = (k) => k === "EvadeFwd" || k === "EvadeBwd" || k === "EvadeLt" || k === "EvadeRt";
  function canChain(cur, next) {
    switch (cur) {
      case "RoadieRun": return next === "Run2Cover";
      case "Run2Cover": return next === "CoverSlip" || next === "CoverRun" || next === "SwatTurn" || next === "Vault";
      case "CoverSlip": return next === "RoadieRun" || next === "Run2Cover";
      case "CoverRun": return next === "SwatTurn";
      case "SwatTurn": return next === "CoverSlip" || next === "Vault";
      case "Vault": return next === "RoadieRun" || next === "Run2Cover";
      case "PushOut": return next !== "Vault"; // all but MidLvlJumpOver / MantleKick
      default: return isEvade(cur) && (next === "RoadieRun" || next === "Run2Cover");
    }
  }
  // push out behaves like a Move2Idle transition: any move cuts it short
  const canOverrideWith = (cur, next) => (isEvade(cur) && (next === "RoadieRun" || next === "Run2Cover")) || cur === "PushOut";
  const overrides = (next, cur) => next === "CoverSlip" && cur === "CoverRun";

  class Character {
    constructor(cfg, pos, yaw) {
      this.cfg = cfg; this.pos = pos; this.vel = V(0, 0, 0); this.yaw = yaw;
      this.time = f(0); this.mirrored = false;
      this.cover = null; this.active = null; this.pending = null; this.pendingCover = null;
      this.pendingSwat = null; this.pendingVault = null; this.swatRoadieTimer = null;
      this.roadieTimer = null; this.prevA = false; this.breakHold = f(0);
      this.lastMoveEnd = -999; this.lastEvadeTime = -999;
      this.events = [];
    }
    // height above the start ground (only a vault leaves it)
    height() {
      const a = this.active;
      if (!a || a.kind !== "Vault") return 0;
      const rise = this.cfg.vault_rise;
      if (a.elapsed < a.up) return rise * a.elapsed / a.up;
      if (a.elapsed < a.split) return rise;
      const t = Math.min(a.elapsed - a.split, a.land - a.split);
      return Math.max(rise - 0.5 * this.cfg.gravity * t * t, 0);
    }
    stateName() { return this.active ? this.active.kind : this.cover ? "InCover" : "Walk"; }
    log(from, reason) { this.events.push({ time: this.time, from, to: this.stateName(), reason }); }
    stickWorld(i) {
      const fw = fromYaw(i.cam_yaw), right = V(-fw.y, fw.x, 0); // exact +90°
      return add(mul(fw, i.stick_y), mul(right, i.stick_x));
    }
    // cover-remapped stick: [right along the wall, up toward it]; camera-relative elsewhere
    remappedStick(i) {
      if (!this.cover) return [i.stick_x, i.stick_y];
      const w = this.stickWorld(i), sp = this.cover.spot;
      return [dot(w, coverRight(sp)), dot(w, neg(sp.normal))];
    }
    static stickLen(i) { return Math.sqrt(i.stick_x * i.stick_x + i.stick_y * i.stick_y); }

    tick(input, dt, world) {
      dt = f(dt);
      this.time = f(this.time + dt);
      const pressed = input.a && !this.prevA, released = !input.a && this.prevA;
      this.prevA = input.a;
      if (pressed) this.onAPressed(input, world);
      if (released) this.onAReleased(input, world);
      if (this.roadieTimer !== null && this.time >= this.roadieTimer) {
        this.roadieTimer = null;
        this.tryToRoadieRun(input);
      }
      if (this.swatRoadieTimer !== null && this.time >= this.swatRoadieTimer) {
        this.swatRoadieTimer = null;
        this.transitionFromSwatToRoadie(input);
      }
      this.tickActive(input, dt, world);
      this.roadieAutoCover(input, world);
      this.checkBreakFromCover(input, dt);
      if (!this.active || this.active.kind === "PushOut") this.tickLocomotion(input, dt); // steer during a push out
    }

    onAPressed(input, world) {
      if (this.active && this.active.kind === "SwatTurn") {
        if (this.swatRoadieTimer === null) this.swatRoadieTimer = f(this.time + this.cfg.swat_to_roadie_delay);
        return;
      }
      if (this.cover) {
        if (this.canCoverSlip(input)) { this.request("CoverSlip", "A in cover: cover slip"); return; }
        const dist = this.canVault(input, world);
        if (dist !== null) {
          this.pendingVault = dist;
          if (this.request("Vault", "A in cover: vault")) return;
          this.pendingVault = null;
        }
        const target = this.canSwatTurn(input, world);
        if (target) {
          this.pendingSwat = target;
          if (this.request("SwatTurn", "A in cover: swat turn")) return;
          this.pendingSwat = null;
        }
        if (this.atCoverEdge(0.5) && this.tryToEvade(input, world)) return;
      }
      if (!this.cover && input.aim) return; // no roadie run while targeting
      this.roadieTimer = f(this.time + this.cfg.roadie_hold_time);
    }
    onAReleased(input, world) {
      if (this.active && (this.active.kind === "RoadieRun" || this.active.kind === "CoverRun")) {
        this.endMove("A released"); this.roadieTimer = null; return;
      }
      const tapped = this.roadieTimer !== null;
      if (this.cover) { if (tapped) this.tryToEvade(input, world); return; }
      if (tapped) {
        this.roadieTimer = null;
        if (!(this.canTryToRunToCover() && this.tryToRunToCover(input, world, 1.0, false))) {
          this.tryToEvade(input, world);
        }
      }
    }
    tryToRoadieRun(input) {
      if (!input.a || (this.active && isEvade(this.active.kind))) return;
      if (!this.cover) this.request("RoadieRun", "A held");
      else this.request("CoverRun", "A held in cover");
    }

    canDo(kind) {
      switch (kind) {
        case "RoadieRun": return !this.cover;
        case "Run2Cover": return this.pendingCover !== null;
        case "CoverRun": case "PushOut": return !!this.cover;
        case "SwatTurn": return !!this.cover && this.pendingSwat !== null;
        case "Vault": return !!this.cover && this.pendingVault !== null;
        case "CoverSlip": return !!this.cover;
        default: return true;
      }
    }
    request(kind, reason) {
      if (this.active) {
        const cur = this.active.kind;
        if (canOverrideWith(cur, kind) || overrides(kind, cur)) { this.startMove(kind, reason); return true; }
        if (canChain(cur, kind)) { this.pending = kind; return true; }
        return false;
      }
      if (!this.canDo(kind)) return false;
      this.startMove(kind, reason);
      return true;
    }
    startMove(kind, reason) {
      const from = this.stateName(), ground = this.cfg.ground_speed;
      const a = {
        kind, started: this.time, elapsed: f(0), duration: f(0), start: this.pos, target: this.pos,
        dir: fromYaw(this.yaw), chainChecked: false, holdBack: f(0), holdLeft: f(0),
        holdRight: f(0), monitoring: kind === "Run2Cover", split: f(0), mid: this.pos,
        up: f(0), over: f(0), land: f(0),
      };
      if (kind === "Run2Cover") {
        const spot = this.pendingCover; this.pendingCover = null;
        const dist = len(flat(sub(spot.location, this.pos)));
        a.duration = f(Math.max(dist / (ground * this.cfg.run2cover_speed_mod), this.cfg.run2cover_min_duration));
        a.target = spot.location;
        a.dir = norm(flat(sub(spot.location, this.pos)));
        this.cover = { spot: { ...spot }, since: this.time };
      } else if (kind === "CoverSlip") {
        const spot = this.cover.spot;
        const side = this.mirrored ? neg(coverRight(spot)) : coverRight(spot);
        const travel = add(mul(side, this.cfg.slip_travel_side), mul(neg(spot.normal), this.cfg.slip_travel_fwd));
        a.dir = norm(travel);
        a.duration = f(this.cfg.slip_anim_length / this.cfg.slip_speed_mod);
        a.target = add(this.pos, travel);
        this.cover = null;
      } else if (kind === "RoadieRun") {
        this.cover = null;
      } else if (kind === "SwatTurn") {
        const target = this.pendingSwat; this.pendingSwat = null;
        const dist = len(flat(sub(target.location, this.pos)));
        a.target = target.location;
        a.dir = norm(flat(sub(target.location, this.pos)));
        const endDist = this.cfg.swat_end_dist;
        const endTime = f(this.cfg.swat_end_anim_length / this.cfg.swat_anim_rate);
        const dash = f(Math.max(f(dist - endDist), 0));
        a.split = f(dash / f(ground * this.cfg.swat_speed_mod));
        a.mid = add(this.pos, mul(a.dir, dash));
        const left = f(Math.min(dist, endDist) / endDist);
        a.duration = f(Math.max(f(a.split + f(endTime * left)), f(1 / 60)));
        this.cover = { spot: { ...target }, since: this.time };
        this.swatRoadieTimer = null;
      } else if (kind === "PushOut") {
        const rate = this.cfg.push_out_speed_mod;
        a.dir = neg(fromYaw(this.yaw));
        a.duration = f(this.cfg.push_out_anim_length / rate);
        this.vel = mul(a.dir, f(this.cfg.ground_speed * rate));
        this.cover = null;
        this.breakHold = f(0);
      } else if (kind === "Vault") {
        const dist = this.pendingVault; this.pendingVault = null;
        const c = this.cover.spot, cfg = this.cfg, rate = cfg.vault_speed_mod;
        a.dir = neg(c.normal);
        this.yaw = yawOf(a.dir);
        const scale = f(Math.max(f(dist / cfg.vault_default_dist), cfg.vault_min_root_scale));
        const frame = f(1 / f(30 * f(rate)));
        a.up = f(cfg.vault_up_frames * frame);
        a.over = f(cfg.vault_over_frames * frame);
        a.split = f(cfg.vault_jump_frames * frame);
        const travel = f(cfg.vault_jump_travel * scale);
        a.mid = add(this.pos, mul(a.dir, travel));
        const g = cfg.gravity;
        const estHoriz = f(cfg.vault_fall_fwd_vel * Math.sqrt(cfg.vault_fall_start_dist / g));
        const ahead = Math.max(f(estHoriz - f(travel - dist)), 0);
        const drop = f(cfg.vault_rise - 0.5 * cfg.collision_height);
        const fwdVel = drop > 1 ? f(ahead / Math.sqrt(drop / g)) : cfg.vault_fall_fwd_vel;
        const fall = f(Math.sqrt(2 * cfg.vault_rise / g));
        a.land = f(a.split + fall);
        a.target = add(a.mid, mul(a.dir, f(fwdVel * fall)));
        a.duration = f(a.land + f(cfg.vault_land_frames * frame));
        this.vel = V(0, 0, 0);
      } else if (isEvade(kind)) {
        const e = this.evadeAnim(kind), rate = this.cfg.evade_speed_mod;
        a.dir = this.evadeMoveDir(kind);
        a.split = f(e.start_length / rate);
        a.duration = f(f(e.start_length + e.end_length) / rate);
        a.mid = add(this.pos, mul(a.dir, e.start_dist));
        a.target = add(a.mid, mul(a.dir, e.end_dist));
        this.vel = V(0, 0, 0);
        this.cover = null;
      }
      this.active = a;
      this.log(from, reason);
    }
    endMove(reason) {
      const a = this.active; if (!a) return;
      this.active = null;
      this.lastMoveEnd = this.time;
      if (a.kind === "CoverSlip") {
        this.vel = mul(a.dir, this.cfg.ground_speed);
        if (this.prevA && this.pending === null) this.pending = "RoadieRun";
      } else if (isEvade(a.kind)) {
        this.lastEvadeTime = this.time;
      } else if (a.kind === "Vault") {
        this.cover = null; this.vel = V(0, 0, 0);
        if (this.prevA && this.pending === null) this.pending = "RoadieRun";
      } else if (a.kind === "SwatTurn") {
        this.swatRoadieTimer = null;
        this.vel = V(0, 0, 0);
        if (this.cover) this.mirrored = !this.mirrored; // inferred: land facing the gap
      }
      this.log(a.kind, reason);
      if (this.pending !== null) {
        const next = this.pending; this.pending = null;
        if (this.canDo(next)) this.startMove(next, "queued move");
      }
    }

    canTryToRunToCover() { return !this.active || this.active.kind === "PushOut"; }
    tryToRunToCover(input, world, checkScale, skipMoveCheck) {
      if (this.cover || (!skipMoveCheck && !this.canTryToRunToCover())) return false;
      const dz = this.cfg.dead_zone;
      let sideways = false, dir;
      if (Character.stickLen(input) > dz) {
        if (Math.abs(input.stick_x) > Math.abs(input.stick_y)) sideways = true;
        else if (input.stick_y < -dz) return false;
        dir = norm(this.stickWorld(input));
      } else dir = fromYaw(this.yaw);
      const max = sideways ? this.cfg.run2cover_side_max_dist : this.cfg.run2cover_max_dist * checkScale;
      const spot = world.findCover(this.pos, dir, max);
      if (!spot) return false;
      this.pendingCover = spot;
      if (this.request("Run2Cover", "run to cover")) return true;
      this.pendingCover = null;
      return false;
    }
    roadieAutoCover(input, world) {
      const a = this.active;
      if (!a || a.kind !== "RoadieRun" || this.time - this.lastMoveEnd <= this.cfg.roadie_cover_after_move) return;
      const facing = fromYaw(this.yaw), dz = this.cfg.dead_zone;
      const check = input.stick_y > dz || Math.abs(input.stick_x) > dz ? norm(this.stickWorld(input)) : facing;
      if (dot(check, facing) <= this.cfg.roadie_cover_dir_dot) return;
      const spot = world.findCover(this.pos, check, this.cfg.roadie_to_cover_dist + this.cfg.collision_radius);
      if (!spot) return;
      const seamless = dot(spot.normal, facing) > this.cfg.seamless_cover_dot;
      const from = this.stateName();
      this.active = null; this.lastMoveEnd = this.time;
      this.pos = spot.location; this.vel = V(0, 0, 0); this.yaw = yawOf(neg(spot.normal));
      this.cover = { spot: { ...spot }, since: this.time };
      this.log(from, "roadie run into cover");
      if (seamless && spot.dist_to_left_edge > 1 && spot.dist_to_right_edge > 1 && input.a) {
        this.request("CoverRun", "seamless roadie → cover run");
      }
    }

    edgeDist() {
      if (!this.cover) return null;
      return this.mirrored ? this.cover.spot.dist_to_left_edge : this.cover.spot.dist_to_right_edge;
    }
    atCoverEdge(tol) { const d = this.edgeDist(); return d !== null && d <= tol + 1; }
    canCoverSlip(input) {
      const c = this.cover; if (!c) return false;
      const allowed = this.mirrored ? c.spot.can_slip_left : c.spot.can_slip_right;
      const edge = this.edgeDist();
      if (!allowed || edge > this.cfg.slip_max_edge_dist) return false;
      const up = this.time - c.since < this.cfg.slip_raw_stick_window
        ? input.stick_y : dot(this.stickWorld(input), neg(c.spot.normal));
      return up >= this.cfg.slip_stick_up;
    }

    canSwatTurn(input, world) {
      const c = this.cover; if (!c) return null;
      const dz = this.cfg.dead_zone, [right, up] = this.remappedStick(input);
      if (Math.abs(right) < dz || (this.mirrored && right > dz) || (!this.mirrored && right < -dz) || Math.abs(up) > Math.abs(right)) return null;
      const facing = neg(c.spot.normal);
      if (dot(fromYaw(input.cam_yaw), facing) < this.cfg.swat_camera_dot) return null;
      const edge = this.edgeDist();
      if (edge === null || edge > this.cfg.swat_edge_dist) return null;
      const side = this.mirrored ? neg(coverRight(c.spot)) : coverRight(c.spot);
      const edgePoint = add(c.spot.location, mul(side, edge));
      return world.findSwatTarget ? world.findSwatTarget(c.spot, edgePoint, side, this.cfg.swat_max_dist) : null;
    }
    // GSM_MidLvlJumpOver / GSM_MantleOverCoverBase.InternalCanDoSpecialMove → MantleDistance
    canVault(input, world) {
      const c = this.cover;
      if (!c || (c.spot.height || "Mid") !== "Mid" || !world.mantleDistance) return null;
      const [right, up] = this.remappedStick(input);
      if (Math.abs(up) < Math.abs(right) || up <= this.cfg.dead_zone) return null;
      const r = this.cfg.collision_radius;
      const sliding = this.active && this.active.kind === "Run2Cover";
      const toSlot = len(flat(sub(c.spot.location, this.pos)));
      if ((sliding && toSlot > this.cfg.vault_slide_slot_dist) || toSlot > 3 * r) return null;
      const from = c.spot.location, dir = neg(c.spot.normal);
      const dist = world.mantleDistance(from, dir, this.cfg.vault_max_dist, r);
      if (dist === null) return null;
      const g = this.cfg.gravity;
      const estHoriz = f(this.cfg.vault_fall_fwd_vel * Math.sqrt(this.cfg.vault_fall_start_dist / g));
      const fallStart = add(from, mul(dir, dist));
      if (!world.isClear(fallStart, add(fallStart, mul(dir, estHoriz + r)), 0)) return null;
      return dist;
    }
    transitionFromSwatToRoadie(input) {
      const a = this.active;
      if (!a || !input.a || a.kind !== "SwatTurn") return;
      if (len(flat(sub(a.target, this.pos))) > 2 * this.cfg.collision_radius) {
        this.cover = null; this.pending = null; this.active = null; this.lastMoveEnd = this.time;
        this.events.push({ time: this.time, from: "SwatTurn", to: "Walk", reason: "swat cut short by roadie run" });
        this.startMove("RoadieRun", "swat → roadie (A held)");
      } else if (this.canCoverSlip(input)) {
        this.request("CoverSlip", "swat → cover slip (A held)");
      }
    }
    chooseEvade(input) {
      const dz = this.cfg.dead_zone, [x, y] = this.remappedStick(input);
      if (Math.abs(y) > Math.abs(x)) return y < -dz ? "EvadeBwd" : "EvadeFwd";
      if (x > dz) return "EvadeRt";
      if (x < -dz) return "EvadeLt";
      return "EvadeFwd";
    }
    evadeAnim(kind) {
      return kind === "EvadeBwd" ? this.cfg.evade_bwd : kind === "EvadeLt" ? this.cfg.evade_lt
        : kind === "EvadeRt" ? this.cfg.evade_rt : this.cfg.evade_fwd;
    }
    // root motion is straight along one axis: exactly 90° / 180°
    evadeMoveDir(kind) {
      const off = kind === "EvadeRt" ? f(PI / 2) : kind === "EvadeLt" ? -f(PI / 2) : kind === "EvadeBwd" ? f(PI) : 0;
      return fromYaw(this.yaw + off);
    }
    // the clearance trace before an evade from cover (keeps the shipped 16834)
    evadeDir(kind) {
      const off = kind === "EvadeRt" ? urotToRad(16384) : kind === "EvadeLt" ? -urotToRad(16834)
        : kind === "EvadeBwd" ? -urotToRad(32768) : 0;
      return fromYaw(this.yaw + off);
    }
    tryToEvade(input, world) {
      if ((this.active && this.active.kind !== "PushOut") || this.time - this.lastEvadeTime < this.cfg.evade_retrigger_delay) return false;
      if (this.cover && Math.abs(this.remappedStick(input)[0]) <= this.cfg.dead_zone) return false;
      const kind = this.chooseEvade(input);
      if (!this.cover && (!this.active || this.active.kind === "PushOut")) this.yaw = input.cam_yaw;
      const dir = this.evadeDir(kind);
      if (this.cover && !world.isClear(this.pos, add(this.pos, mul(dir, 128)), 12)) return false;
      return this.request(kind, "evade");
    }

    tickActive(input, dt, world) {
      const a = this.active; if (!a) return;
      if (a.started === this.time) {
        if (a.kind === "RoadieRun" && Character.stickLen(input) > this.cfg.dead_zone) {
          this.yaw = yawOf(this.stickWorld(input));
        }
        return;
      }
      a.elapsed = f(a.elapsed + dt);
      const lerp = (t) => add(a.start, mul(sub(a.target, a.start), t));
      if (a.kind === "RoadieRun") {
        this.yaw += input.stick_x * this.cfg.roadie_steering_pct * this.cfg.turn_rate * dt;
        this.vel = mul(fromYaw(this.yaw), this.cfg.ground_speed * this.cfg.roadie_speed_mod);
        this.pos = add(this.pos, mul(this.vel, dt));
      } else if (a.kind === "CoverRun") {
        const c = this.cover.spot;
        const left = input.stick_x < -this.cfg.dead_zone;
        const side = left ? neg(coverRight(c)) : coverRight(c);
        this.mirrored = left;
        const limit = this.mirrored ? c.dist_to_left_edge : c.dist_to_right_edge;
        const step = Math.min(this.cfg.ground_speed * this.cfg.cover_run_speed_mod * dt, Math.max(limit, 0));
        this.pos = add(this.pos, mul(side, step));
        if (this.mirrored) { c.dist_to_left_edge -= step; c.dist_to_right_edge += step; }
        else { c.dist_to_right_edge -= step; c.dist_to_left_edge += step; }
        c.location = this.pos;
        if (limit - step <= 0.01) this.endMove("reached cover edge");
      } else if (a.kind === "Run2Cover") {
        // cover-remapped stick; back (away from the wall) aborts at once, sides need 0.25 s
        const dz = this.cfg.dead_zone, hold = this.cfg.run2cover_mirror_hold;
        const [right, up] = this.remappedStick(input);
        let abort = false;
        if (a.monitoring) {
          const held = (on, t) => (on ? f(t + dt) : f(0));
          a.holdBack = held(up < -dz && Math.abs(up) >= Math.abs(right), a.holdBack);
          a.holdLeft = held(right < -dz, a.holdLeft);
          a.holdRight = held(right > dz, a.holdRight);
          if (a.holdBack > 0) { a.monitoring = false; abort = true; }
          else if (a.holdLeft >= hold || a.holdRight >= hold) { a.monitoring = false; this.mirrored = a.holdLeft >= hold; }
        }
        const t = Math.min(a.elapsed / a.duration, 1);
        this.pos = lerp(t);
        if (this.cover) this.yaw += angleDiff(this.yaw, yawOf(neg(this.cover.spot.normal))) * Math.min(t, 1);
        if (abort) {
          this.cover = null; this.pending = null; this.pendingSwat = null;
          this.vel = mul(a.dir, this.cfg.ground_speed); // keeps moving, capped to run speed (assumption)
          this.endMove("slide aborted (stick back)");
        } else if (a.elapsed >= a.duration) {
          this.vel = V(0, 0, 0);
          if (this.cover) this.yaw = yawOf(neg(this.cover.spot.normal));
          if (a.monitoring) {
            if (a.holdLeft > 0) this.mirrored = true;
            else if (a.holdRight > 0) this.mirrored = false;
          }
          this.endMove("reached cover");
        }
      } else if (a.kind === "CoverSlip") {
        this.pos = lerp(Math.min(a.elapsed / a.duration, 1));
        this.yaw = yawOf(a.dir);
        if (a.elapsed >= a.duration) this.endMove("slip finished");
      } else if (a.kind === "SwatTurn") {
        this.pos = Character.twoPart(a);
        if (this.cover) this.yaw = yawOf(neg(this.cover.spot.normal));
        if (a.elapsed >= a.duration) this.endMove("swat turn finished");
      } else if (a.kind === "Vault") {
        if (a.elapsed < a.up) this.pos = a.start;
        else if (a.elapsed < a.over) this.pos = add(a.start, mul(sub(a.mid, a.start), f(a.elapsed - a.up) / f(a.over - a.up)));
        else if (a.elapsed < a.split) this.pos = a.mid;
        else this.pos = add(a.mid, mul(sub(a.target, a.mid), Math.min(f(a.elapsed - a.split) / f(a.land - a.split), 1)));
        this.yaw = yawOf(a.dir);
        const allow = f(a.land + f(this.cfg.vault_roadie_after_land / this.cfg.vault_speed_mod));
        if (a.elapsed >= allow && input.a && this.pending === null) {
          this.cover = null; this.vel = V(0, 0, 0); this.active = null; this.lastMoveEnd = this.time;
          this.events.push({ time: this.time, from: "Vault", to: "Walk", reason: "landing cut short by roadie run" });
          this.startMove("RoadieRun", "vault → roadie (A held)");
        } else if (a.elapsed >= a.duration) this.endMove("vault finished");
      } else if (a.kind === "PushOut") {
        if (a.elapsed >= a.duration) this.endMove("push out finished");
      } else if (isEvade(a.kind)) {
        this.pos = Character.twoPart(a);
        if (!a.chainChecked && a.duration - a.elapsed <= this.cfg.evade_chain_lead) {
          a.chainChecked = true;
          if (input.a) {
            if (!this.tryToRunToCover(input, world, this.cfg.evade_cover_check_scale, true)) {
              this.request("RoadieRun", "evade → roadie (A held)");
            }
            return;
          }
        }
        if (a.elapsed >= a.duration) this.endMove("evade finished");
      }
    }
    checkBreakFromCover(input, dt) {
      if (!this.cover || this.active) { this.breakHold = f(0); return; }
      const dz = this.cfg.dead_zone, [right, up] = this.remappedStick(input);
      const upDominant = Math.abs(up) > Math.abs(right);
      if (!upDominant || Math.abs(up) < dz) this.breakHold = f(0);
      if (upDominant && up < -dz) {
        this.breakHold = f(this.breakHold + dt);
        if (this.breakHold >= this.cfg.break_from_cover_hold) this.request("PushOut", "stick away from cover: push out");
      }
    }
    static twoPart(a) {
      if (a.elapsed < a.split) return add(a.start, mul(sub(a.mid, a.start), a.elapsed / a.split));
      const rest = f(a.duration - a.split);
      const t = rest > 0 ? Math.min(f(a.elapsed - a.split) / rest, 1) : 1;
      return add(a.mid, mul(sub(a.target, a.mid), t));
    }
    // moving along cover: sideways-dominant stick → face that way, move at
    // GroundSpeed × CoverMovementPct, the ends stop you
    tickCoverMove(input, dt) {
      const c = this.cover; if (!c) return;
      const dz = this.cfg.dead_zone, [right, up] = this.remappedStick(input);
      let along = 0;
      if (Math.abs(right) > dz && Math.abs(right) > Math.abs(up)) { this.mirrored = right < 0; along = Math.max(-1, Math.min(1, right)); }
      const pct = c.spot.height === "Standing" ? this.cfg.cover_move_pct_std : this.cfg.cover_move_pct_mid;
      const side = coverRight(c.spot);
      const cur = f(dot(this.vel, side));
      const goal = f(f(along * this.cfg.ground_speed) * pct);
      const maxDv = f(this.cfg.accel_rate * dt);
      let v = f(cur + Math.max(-maxDv, Math.min(maxDv, f(goal - cur))));
      const room = v >= 0 ? c.spot.dist_to_right_edge : c.spot.dist_to_left_edge;
      let step = f(v * dt);
      if (Math.abs(step) > room) { step = f(Math.max(room, 0) * Math.sign(step)); v = 0; }
      this.vel = mul(side, v);
      if (step !== 0) {
        this.pos = add(this.pos, mul(side, step));
        c.spot.dist_to_right_edge -= step; c.spot.dist_to_left_edge += step;
        c.spot.location = this.pos;
      }
    }
    tickLocomotion(input, dt) {
      if (this.cover) { this.tickCoverMove(input, dt); return; }
      let wish = this.stickWorld(input);
      if (len(wish) > 1) wish = norm(wish);
      if (Character.stickLen(input) <= this.cfg.dead_zone) wish = V(0, 0, 0);
      // aiming: forced walk at GroundSpeed × WalkingPct, facing the camera (strafe)
      const speed = input.aim ? this.cfg.ground_speed * this.cfg.walking_pct : this.cfg.ground_speed;
      this.vel = moveToward(this.vel, mul(wish, speed), this.cfg.accel_rate * dt);
      this.pos = add(this.pos, mul(this.vel, dt));
      if (input.aim) this.yaw = input.cam_yaw;
      else if (len2d(this.vel) > 1) this.yaw = yawOf(this.vel);
    }
  }

  const api = { V, Character, WallWorld, defaultConfig, coverRight, len2d };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  else root.GearsCore = api;
})(typeof window !== "undefined" ? window : globalThis);
