#!/usr/bin/env python3
"""Pack Marcus's mesh and a chosen set of animations into one .glb for the test range.

Inputs are files *you* exported from *your own* Gears of War 3 dump with UE Viewer
(umodel), all under _local/:
  - the skeletal mesh as glTF   (umodel -export -gltf ... COG_Marcus_Fenix_CamSkel)
  - one or more AnimSets as psa (umodel -export ... AnimSetMarcus_CamSkel AnimSet)
    plus the .props.txt umodel writes next to each psa
Output (also under _local/, never committed, never published):
  - marcus.glb           mesh + skeleton + the selected clips (root bone kept in place)
  - marcus_anims.json    per clip: length, frames, root-motion travel (uu), for docs/tests

Our own code. Formats: ActorX psa (public format, see UE Viewer Exporters/Psk.h) and
glTF 2.0. Coordinate rules mirror UE Viewer's exporters:
  - psa keys are written mirrored (MIRROR_MESH: Y and W of quats, Y of positions
    negated) → undo that to get engine values;
  - glTF: swap Y/Z, positions × 0.01, root bone rotation conjugated.
UE3 AnimSets with bAnimRotationOnly use the animation's translation only for bones in
UseTranslationBoneNames; every other bone keeps the mesh's reference translation.

  python tools/gears_anim_pack.py --mesh <mesh.gltf> --psa <a.psa> [<b.psa> ...] \
      --out _local/exports/marcus/pack [--clips name1,name2 | --all]
  python tools/gears_anim_pack.py --mesh <COGAssaultRifle.gltf> --name lancer \
      --out _local/exports/marcus/pack          (the Lancer: mesh only)
"""
import argparse
import json
import math
import os
import re
import struct
import sys

# Clips the test range uses, by Gears 3 sequence name.
DEFAULT_CLIPS = [
    # locomotion (AnimSetMarcus_CamSkel_LayeredNew)
    "Idle_Core", "AR_Walk_Fwd_Ready_Core", "AR_Run_Fwd_Ready_Core", "Roadie_Run_Core",
    "Cov_Mid_Idle_Core", "Cov_Std_Idle_Core", "Cov_Mid_Run_Core", "Cov_Std_Run_Core",
    "ADD_AR_Idle_Ready", "ADD_AR_Walk_Ready", "ADD_AR_Run_Ready", "ADD_AR_RoadieRun_Fwd",
    "ADD_AR_Cov_Mid_Idle_Ready", "ADD_AR_Cov_Std_Idle_Ready", "ADD_AR_Cov_RoadieRun",
    # moves (AnimSetMarcus_CamSkel)
    "AR_Run_Fwd2Cov_Mid_Start", "AR_Run_Fwd2Cov_Mid_Stop",
    "AR_Run_Fwd2Cov_Std_Start", "AR_Run_Fwd2Cov_Std_Stop",
    "AR_Cov_Mid_Slip", "AR_Cov_Std_Slip",
    "AR_Cov_Mid_SwatTurn_Start", "AR_Cov_Mid_SwatTurn_End",
    "AR_Cov_Std_SwatTurn_Start", "AR_Cov_Std_SwatTurn_End",
    "AR_Evade_Fwd_Start", "AR_Evade_Fwd_End", "AR_Evade_Bwd_Start", "AR_Evade_Bwd_End",
    "AR_Evade_Lt_Start", "AR_Evade_Lt_End", "AR_Evade_Rt_Start", "AR_Evade_Rt_End",
    "AR_Cov_Mid_Idle2Idle_Ready_Aim", "AR_Cov_Std_Idle2Idle_Ready_Aim",
    # vault over mid cover (GSM_MantleOverCoverBase BSList_Jump / Fall / Land + Mirrored)
    "AR_Mantle_Start", "AR_Mantle_Mid", "AR_Mantle_End",
    "AR_Mantle_Mirrored_Start", "AR_Mantle_Mirrored_Mid", "AR_Mantle_Mirrored_End",
    # aiming and reloading (weapon layers, AnimSetMarcus_CamSkel_LayeredNew)
    "ADD_AR_Idle_Ready_Aim", "ADD_AR_Walk_Aim", "ADD_AR_Reload", "ADD_SG_Reload",
    "ADD_AR_Reload_Fail", "ADD_SG_Reload_Fail", "ADD_SG_Reload_Success",
    # strafing while aiming, popping up over mid cover
    "AR_Walk_Fwd_Aim_Core", "AR_Walk_Bwd_Aim_Core", "AR_Walk_Lt_Aim_Core", "AR_Walk_Rt_Aim_Core",
    "AR_Cov_Mid_Idle_Ready2DownSights_Aim",
    # Gnasher pump (BS_PawnShotgunCock), roadie run steering lean (AnimSetMarcus_Overlay)
    "ADD_SG_Idle_Ready_Chamber", "ADD_Overlay_AR_RoadieRun_Lean_L", "ADD_Overlay_AR_RoadieRun_Lean_R",
    # weapon anims (COG_AssaultRifle, COG_Gnasher_Anim)
    "AR_Fire", "AR_Reload", "AR_Reload_Fumble", "SG_Chamber", "SG_Reload", "SG_Reload_Fail", "SG_Reload_Success",
]

# Clips whose root track the page needs (the jump rises by root motion): stored per
# frame as [forward, up] uu from frame 0 in the animation's glTF "extras".
ROOT_CURVE = {"AR_Mantle_Start", "AR_Mantle_Mirrored_Start"}

ROOT = "b_MF_Root"

# Looping cycles: UE3 interpolates the last key back to the first over one more frame,
# so the clip gets frame 0 again at n / rate (otherwise every loop skips a frame).
LOOPING = {
    "AR_Walk_Fwd_Ready_Core", "AR_Run_Fwd_Ready_Core", "Roadie_Run_Core",
    "Cov_Mid_Run_Core", "Cov_Std_Run_Core",
    "ADD_AR_Walk_Ready", "ADD_AR_Run_Ready", "ADD_AR_RoadieRun_Fwd", "ADD_AR_Cov_RoadieRun",
    "ADD_AR_Walk_Aim",
    "AR_Walk_Fwd_Aim_Core", "AR_Walk_Bwd_Aim_Core", "AR_Walk_Lt_Aim_Core", "AR_Walk_Rt_Aim_Core",
}


# ------------------------------------------------------------------ psa reader
def read_psa(path):
    d = open(path, "rb").read()
    p, chunks = 0, {}
    while p < len(d):
        cid, _flag, size, count = struct.unpack_from("<20sIii", d, p)
        p += 32
        chunks[cid.rstrip(b"\0").decode()] = (p, size, count)
        p += size * count
    bp, bsize, bcount = chunks["BONENAMES"]
    bones = [d[bp + i * bsize: bp + i * bsize + 64].split(b"\0")[0].decode() for i in range(bcount)]
    ip, isize, icount = chunks["ANIMINFO"]
    anims = []
    for i in range(icount):
        o = ip + i * isize
        name = d[o:o + 64].split(b"\0")[0].decode()
        total_bones, _ri, _kcs, _kq, _kr, track_time, rate, _sb, first, nframes = struct.unpack_from(
            "<iiiiffffii", d, o + 128)
        anims.append({"name": name, "bones": total_bones, "rate": rate, "first": first,
                      "frames": nframes, "track_time": track_time})
    kp, ksize, _kcount = chunks["ANIMKEYS"]
    return {"bones": bones, "anims": anims, "data": d, "keys_at": kp, "key_size": ksize}


def psa_key(psa, frame_index, bone):
    """Engine-space (un-mirrored) position (uu) and quaternion (x, y, z, w)."""
    o = psa["keys_at"] + (frame_index * len(psa["bones"]) + bone) * psa["key_size"]
    px, py, pz, qx, qy, qz, qw, _t = struct.unpack_from("<8f", psa["data"], o)
    return (px, -py, pz), (qx, -qy, qz, -qw)


def read_props(psa_path):
    """bAnimRotationOnly and UseTranslationBoneNames from umodel's .props.txt."""
    p = psa_path[:-4] + ".props.txt"
    if not os.path.exists(p):
        return False, set()
    t = open(p, encoding="utf-8", errors="replace").read()
    rot_only = re.search(r"^bAnimRotationOnly = true", t, re.M) is not None
    use = set()
    m = re.search(r"^UseTranslationBoneNames\[\d+\] =\s*\{(.*?)^\}", t, re.M | re.S)
    if m:
        use = set(re.findall(r"= (\S+)", m.group(1)))
    return rot_only, use


# --------------------------------------------------------------- glTF helpers
def to_gltf_pos(p):
    return (p[0] * 0.01, p[2] * 0.01, p[1] * 0.01)


def to_gltf_rot(q, is_root):
    x, y, z, w = q[0], q[2], q[1], q[3]
    if is_root:
        x, y, z = -x, -y, -z
    n = math.sqrt(x * x + y * y + z * z + w * w) or 1.0
    return (x / n, y / n, z / n, w / n)


def load_gltf(path):
    g = json.load(open(path))
    base = os.path.dirname(path)
    bins = [open(os.path.join(base, b["uri"]), "rb").read() for b in g["buffers"]]
    return g, bins


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--mesh", required=True)
    ap.add_argument("--psa", nargs="*", default=[])
    ap.add_argument("--out", required=True)
    ap.add_argument("--clips", default=None, help="comma-separated sequence names")
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--name", default="marcus", help="output base name (marcus.glb, ...)")
    ap.add_argument("--tex", action="append", default=[],
                    help="Material=image.png: embed the image as that material's base colour "
                         "(Material '*' = every material without one)")
    a = ap.parse_args()
    if "_local" not in os.path.abspath(a.out).replace("\\", "/").split("/"):
        sys.exit("refusing to write outside _local/: exported Gears data must stay local")

    g, bins = load_gltf(a.mesh)
    assert len(bins) == 1, "expected one buffer"
    blob = bytearray(bins[0])
    skin = g["skins"][0]
    node_by_bone = {g["nodes"][j]["name"].lower(): j for j in skin["joints"]}

    wanted = None if a.all else (a.clips.split(",") if a.clips else DEFAULT_CLIPS)
    psas = []
    for path in a.psa:
        psa = read_psa(path)
        psa["rot_only"], psa["use_trans"] = read_props(path)
        psa["use_trans"] = {n.lower() for n in psa["use_trans"]}
        psas.append(psa)

    def align4():
        while len(blob) % 4:
            blob.append(0)

    def add_accessor(fmt_count, comp, data, typ, mn=None, mx=None):
        align4()
        off = len(blob)
        blob.extend(data)
        g["bufferViews"].append({"buffer": 0, "byteOffset": off, "byteLength": len(data)})
        acc = {"bufferView": len(g["bufferViews"]) - 1, "componentType": 5126,
               "count": fmt_count, "type": typ}
        if mn is not None:
            acc["min"], acc["max"] = mn, mx
        g["accessors"].append(acc)
        return len(g["accessors"]) - 1

    g["animations"] = []
    info = {}
    seen = set()
    for psa in psas:
        for an in psa["anims"]:
            name = an["name"]
            if (wanted is not None and name not in wanted) or name in seen:
                continue
            seen.add(name)
            n = an["frames"]
            rate = an["rate"] if an["rate"] > 0 else 30.0
            loop = name in LOOPING and n > 1
            frames = list(range(n)) + ([0] if loop else [])
            m = len(frames)
            times = [i / rate for i in range(m)]
            t_acc = add_accessor(m, 5126, struct.pack(f"<{m}f", *times), "SCALAR",
                                 [0.0], [times[-1] if m else 0.0])
            channels, samplers = [], []
            root_start = root_end = None
            root_track = None
            for b, bone in enumerate(psa["bones"]):
                node = node_by_bone.get(bone.lower())
                if node is None:
                    continue
                is_root = bone == ROOT or node == skin["joints"][0]
                rots, poss = [], []
                for f in frames:
                    pos, rot = psa_key(psa, an["first"] + f, b)
                    rots.append(to_gltf_rot(rot, is_root))
                    poss.append(pos)
                if bone == ROOT:
                    root_start, root_end = poss[0], poss[n - 1]
                    root_track = poss[:n]
                flat = [c for r in rots for c in r]
                r_acc = add_accessor(m, 5126, struct.pack(f"<{len(flat)}f", *flat), "VEC4")
                samplers.append({"input": t_acc, "output": r_acc, "interpolation": "LINEAR"})
                channels.append({"sampler": len(samplers) - 1, "target": {"node": node, "path": "rotation"}})
                # translation only where Gears uses it (never the root: we drive position)
                if bone != ROOT and (not psa["rot_only"] or bone.lower() in psa["use_trans"]):
                    tp = [c for p in poss for c in to_gltf_pos(p)]
                    p_acc = add_accessor(m, 5126, struct.pack(f"<{len(tp)}f", *tp), "VEC3")
                    samplers.append({"input": t_acc, "output": p_acc, "interpolation": "LINEAR"})
                    channels.append({"sampler": len(samplers) - 1, "target": {"node": node, "path": "translation"}})
            anim = {"name": name, "channels": channels, "samplers": samplers}
            if name in ROOT_CURVE and root_track:
                # mesh forward is engine +Y, up is +Z
                anim["extras"] = {"root": [[round(q[1] - root_track[0][1], 2),
                                            round(q[2] - root_track[0][2], 2)] for q in root_track]}
            g["animations"].append(anim)
            travel = None
            if root_start is not None:
                travel = [round(root_end[i] - root_start[i], 2) for i in range(3)]
            info[name] = {"length": round(n / rate, 4), "frames": n, "loops": loop,
                          "rate": rate, "root_travel_uu_engine": travel,
                          "root_start_uu_engine": [round(v, 2) for v in root_start] if root_start else None}

    # textures: embed as JPEG (opaque) or PNG (with alpha, alpha-tested)
    if a.tex:
        import io
        from PIL import Image
        g.setdefault("images", []); g.setdefault("textures", []); g.setdefault("samplers", [{}])
        by_mat = dict(t.split("=", 1) for t in a.tex)
        cache = {}
        for m in g.get("materials", []):
            path = by_mat.get(m.get("name")) or by_mat.get("*")
            if not path:
                continue
            if path not in cache:
                im = Image.open(path)
                alpha = im.mode == "RGBA" and im.getchannel("A").getextrema()[0] < 250
                buf = io.BytesIO()
                if alpha:
                    im.save(buf, "PNG", optimize=True)
                else:
                    im.convert("RGB").save(buf, "JPEG", quality=88)
                align4()
                off = len(blob)
                blob.extend(buf.getvalue())
                g["bufferViews"].append({"buffer": 0, "byteOffset": off, "byteLength": len(buf.getvalue())})
                g["images"].append({"bufferView": len(g["bufferViews"]) - 1,
                                    "mimeType": "image/png" if alpha else "image/jpeg"})
                g["textures"].append({"source": len(g["images"]) - 1, "sampler": 0})
                cache[path] = (len(g["textures"]) - 1, alpha)
            ti, alpha = cache[path]
            pbr = m.setdefault("pbrMetallicRoughness", {})
            pbr["baseColorTexture"] = {"index": ti}
            pbr.pop("baseColorFactor", None)
            if alpha:
                m["alphaMode"], m["alphaCutoff"] = "MASK", 0.5

    missing = sorted(set(wanted or []) - seen)
    # pack .glb
    for b in g["buffers"]:
        b.pop("uri", None)
    align4()
    g["buffers"] = [{"byteLength": len(blob)}]
    g["asset"]["generator"] = "libgears gears_anim_pack.py (local export, do not distribute)"
    js = json.dumps(g, separators=(",", ":")).encode()
    js += b" " * ((4 - len(js) % 4) % 4)
    glb = struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(js) + 8 + len(blob))
    glb += struct.pack("<II", len(js), 0x4E4F534A) + js
    glb += struct.pack("<II", len(blob), 0x004E4942) + bytes(blob)
    os.makedirs(a.out, exist_ok=True)
    open(os.path.join(a.out, a.name + ".glb"), "wb").write(glb)
    if a.psa:
        json.dump(info, open(os.path.join(a.out, a.name + "_anims.json"), "w"), indent=1)
    print(f"wrote {os.path.join(a.out, a.name + '.glb')} ({len(glb) / 1e6:.1f} MB), {len(info)} clips")
    if a.psa and missing:
        print("not found:", ", ".join(missing))


if __name__ == "__main__":
    main()
