"""Extract aim-offset profiles (GearAnim_AimOffset nodes in Marcus's AnimTree,
AT_MarcusLayered) from your own unpacked GearGame.xxx into marcus_aim.json for the test
range. Gears data: output must stay under _local/.

Binary layout per profile (big-endian, 360): ProfileName (FName), HorizontalRange and
VerticalRange (2 floats each), AimComponents (count + per bone: BoneName, then 9 ×
{quat x y z w, translation x y z} for LU LC LD CU CC CD RU RC RD), 9 anim FNames.

  python tools/gears_aimoffset.py <unpacked GearGame.xxx> --out _local/exports/marcus/pack
"""
import argparse
import json
import os
import re
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
import ue3_props as up  # noqa: E402

# which nodes the range uses: standing rifle aim, aiming over mid cover
NODES = {"stand": "AT_MarcusLayered.GearAnim_AimOffset_2", "cover_mid": "AT_MarcusLayered.GearAnim_AimOffset_23"}
DIRS = ["LU", "LC", "LD", "CU", "CC", "CD", "RU", "RC", "RD"]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("pkg")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    if "_local" not in os.path.abspath(a.out).replace("\\", "/").split("/"):
        sys.exit("refusing to write outside _local/: exported Gears data must stay local")
    os.environ["UE3_ARRAY_BYTES"] = "10000000"
    pk = up.Pkg(a.pkg)

    def nm(d, o):
        i, n = struct.unpack_from(">ii", d, o)
        s = pk.names[i]
        return s if n == 0 else f"{s}_{n - 1}"

    out = {"libgears_aim": 1, "profiles": {}}
    for key, suffix in NODES.items():
        idx = next(i for i, e in enumerate(pk.exports) if pk.path(i).endswith(suffix))
        e = pk.exports[idx]
        r = up.Reader(pk, pk.d[e["offset"]:e["offset"] + e["size"]])
        r.i32()
        prof = [v for n, _, v in r.props() if n == "Profiles"][0]
        d = bytes.fromhex(re.search(r"bytes: ([0-9a-f]+)", str(prof)).group(1))
        o = 0
        name = nm(d, o); o += 8
        hr = struct.unpack_from(">2f", d, o); o += 8
        vr = struct.unpack_from(">2f", d, o); o += 8
        n = struct.unpack_from(">i", d, o)[0]; o += 4
        bones = {}
        for _ in range(n):
            b = nm(d, o); o += 8
            t = {}
            for dn in DIRS:
                q = struct.unpack_from(">4f", d, o); p = struct.unpack_from(">3f", d, o + 16)
                t[dn] = [round(x, 6) for x in q + p]
                o += 28
            bones[b] = t
        out["profiles"][key] = {"node": suffix, "profile": name, "h_range": hr, "v_range": vr, "bones": bones}
        print(key, suffix, name, "bones", n)
    os.makedirs(a.out, exist_ok=True)
    path = os.path.join(a.out, "marcus_aim.json")
    json.dump(out, open(path, "w"))
    print("wrote", path, os.path.getsize(path), "bytes")


if __name__ == "__main__":
    main()
