"""Build everything the desktop range (game/) and the browser range need, from YOUR OWN
Gears of War 3 dump. Nothing from Gears is in the repo; all output goes to _local/.

    python tools/setup.py --dump "_local/dumps/gears3/Gears 3" --umodel path/to/umodel.exe
                          [--decompress path/to/decompress.exe]

Steps (each skipped if its output already exists, use --force to redo):
  1. UE Viewer exports: Marcus / Lancer / Gnasher meshes (glTF), their animation sets
     (psa), diffuse textures (png, read from the .tfc texture caches) and the weapon,
     movement, footstep and Marcus chatter sounds (xma, needs -sounds).
  2. tools/gears_anim_pack.py  → marcus.glb, lancer.glb, gnasher.glb (anims + textures)
  3. tools/gears_sound_pack.py → gears_sounds.json (XMA → MP3 with ffmpeg)
  4. tools/gears_aimoffset.py  → marcus_aim.json (needs GearGame.xxx decompressed:
     UE Viewer's decompress tool, Tools/PackageUnpack)

Needs: Python 3.10+, Pillow (pip install pillow), ffmpeg with the xma2 decoder on PATH,
UE Viewer (umodel) and, for aim offsets, its decompress tool.
"""
import argparse
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
EXP = os.path.join(ROOT, "_local", "exports", "marcus")
SND = os.path.join(ROOT, "_local", "exports", "sounds")
PACK = os.path.join(EXP, "pack")
UNPACKED = os.path.join(ROOT, "_local", "re", "gears3", "unpacked")

MESHES = ["COG_Marcus_Fenix_CamSkel", "COGAssaultRifle", "COG_Gnasher_Skel"]
ANIMSETS = ["AnimSetMarcus_CamSkel_LayeredNew", "AnimSetMarcus_CamSkel", "AnimSetMarcus_Overlay",
            "COG_AssaultRifle", "COG_Gnasher_Anim"]
TEXTURES = ["T_COG_Summer_Marcus_D", "T_Cog_Marcus_Summer_Hair_D", "COG_AssaultRifle_II_D",
            "COG_AssaultRifle_Blades_Tex", "Tex_COG_Gnasher"]
SOUNDS = [  # GearGame.xxx
    "CogARifleFireLoopPlayer01", "CogARifleFireStopPlayer01", "ShotgunFirePlayer01", "ShotgunFirePlayer02",
    "ShotgunFirePlayer03", "shotguncock01", "CogRifleReloadArm01", "CogRifleReloadInsert01", "ShotgunReloadArm01",
    "ShotgunReloadInsert01", "ShotgunReloadEject01", "ActiveReloadSuccess03", "ActiveReloadSuperSuccess02",
    "ActiveReloadChunk", "ActiveReloadChunkB", "ShotgunRaise01", "CogRifleRaise", "RifleAmmoImpactBodyFlesh01",
    "ShotgunAmmoImpactBodyFlesh01", "ShotgunAmmoBounce01", "ShotgunAmmoBounce02", "CogBodyCoverHighIn01",
    "CogBodyCoverLowIn01", "CogBodyCoverLowIn03", "CogBodyCoverHighOut01", "CogBodyCoverLowOut01", "CogBodyRoll01",
    "CogBodyRoll02", "CogBodyRoll03", "CoverTurn01", "CoverTurn03", "CoverCombat_LeapOver01", "CoverCombat_LeapOver02",
    "CoverCombat_Recover01", "Footsteps_Marcus_Concrete01", "Footsteps_Marcus_Concrete02",
    "Footsteps_Marcus_Concrete03", "Footsteps_Marcus_Concrete04", "Footsteps_Marcus_ConcreteLand01",
    "Footsteps_Marcus_ConcreteSlide01",
]
EXTRA_SOUNDS = [("GEARGAMETDM_SF.xxx", "CogRifleFireEmpty04")]
CHATTER_PKGS = ["GUD_COGMarcusG_SF_LOC_INT.xxx", "GUD_COGMarcusG1_SF_LOC_INT.xxx", "GUD_COGMarcusG2_SF_LOC_INT.xxx"]
CHATTER = ["GotOne_Medium01", "Next_Loud01", "WhosNext_Loud01", "WhosNext_Medium02", "EatIt_Medium01",
           "NothingButBits_Medium01", "AndStayDown_Medium02", "RestInPieces_Loud01", "SmokedHim_Medium02",
           "TooUglyToLive_Medium01", "ThatWasEasy_Medium01", "Gotcha_Medium02", "GoodNight_Loud01",
           "LookMaNoFace_Medium02", "ThatsATwofer_Loud01", "ThisWillDo_Medium01", "ArmedAndReady_Medium03",
           "ReadyAndWaiting_Medium01"]


def run(cmd, quiet=True):
    r = subprocess.run(cmd, capture_output=quiet, text=True)
    return r.returncode == 0


def find(base, name, ext):
    for d, _, fs in os.walk(base):
        for f in fs:
            if f.lower() == (name + ext).lower():
                return os.path.join(d, f)
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dump", required=True, help='the "Gears 3" folder of your dump (has GearGame/CookedXbox360)')
    ap.add_argument("--umodel", default="umodel")
    ap.add_argument("--decompress", default=None, help="UE Viewer's decompress tool (for aim offsets)")
    ap.add_argument("--force", action="store_true")
    a = ap.parse_args()
    cooked = os.path.join(a.dump, "GearGame", "CookedXbox360")
    if not os.path.isdir(cooked):
        sys.exit(f"no GearGame/CookedXbox360 under {a.dump}")
    um = [a.umodel, f"-path={cooked}"]
    ok = lambda p: os.path.exists(p) and not a.force  # noqa: E731

    print("1/4 exporting with UE Viewer ...")
    for m in MESHES:
        if not (ok(find(EXP, m, ".gltf") or "")):
            run(um + ["-export", "-gltf", f"-out={EXP}", "GearGame.xxx", m, "SkeletalMesh"])
    for s in ANIMSETS:
        if not (ok(find(EXP, s, ".psa") or "")):
            run(um + ["-export", f"-out={EXP}", "GearGame.xxx", s, "AnimSet"])
    for t in TEXTURES:
        if not (ok(find(EXP, t, ".png") or "")):
            run(um + ["-export", "-png", f"-out={EXP}", "GearGame.xxx", t, "Texture2D"])
    for s in SOUNDS:
        if not (ok(find(SND, s, ".xma") or "")):
            run(um + ["-sounds", "-export", f"-out={SND}", "GearGame.xxx", s, "SoundNodeWave"])
    for pkg, s in EXTRA_SOUNDS:
        if not (ok(find(SND, s, ".xma") or "")):
            run(um + ["-sounds", "-export", f"-out={SND}", pkg, s, "SoundNodeWave"])
    for v in CHATTER:
        name = "MarcusChatter_" + v
        if ok(find(SND, name, ".xma") or ""):
            continue
        for pkg in CHATTER_PKGS:
            if run(um + ["-sounds", "-export", f"-out={SND}", pkg, name, "SoundNodeWave"]) and find(SND, name, ".xma"):
                break
    missing = [n for n in MESHES if not find(EXP, n, ".gltf")] + [n for n in ANIMSETS if not find(EXP, n, ".psa")]
    if missing:
        sys.exit("UE Viewer didn't export: " + ", ".join(missing) + " (check --umodel and --dump)")

    print("2/4 packing models and animations ...")
    py = [sys.executable, os.path.join(ROOT, "tools", "gears_anim_pack.py")]
    g = lambda n: find(EXP, n, ".gltf")  # noqa: E731
    p = lambda n: find(EXP, n, ".psa")  # noqa: E731
    tex = lambda n: find(EXP, n, ".png")  # noqa: E731
    texargs = lambda pairs: sum([["--tex", f"{m}={tex(t)}"] for m, t in pairs if tex(t)], [])  # noqa: E731
    run(py + ["--mesh", g("COG_Marcus_Fenix_CamSkel"), "--psa", p("AnimSetMarcus_CamSkel_LayeredNew"),
              p("AnimSetMarcus_CamSkel"), p("AnimSetMarcus_Overlay"), "--out", PACK]
        + texargs([("MI_COG_Marcus_Summer_Hair", "T_Cog_Marcus_Summer_Hair_D"), ("*", "T_COG_Summer_Marcus_D")]), quiet=False)
    run(py + ["--mesh", g("COGAssaultRifle"), "--psa", p("COG_AssaultRifle"), "--name", "lancer", "--out", PACK]
        + texargs([("BlurringBlades", "COG_AssaultRifle_Blades_Tex"), ("*", "COG_AssaultRifle_II_D")]), quiet=False)
    run(py + ["--mesh", g("COG_Gnasher_Skel"), "--psa", p("COG_Gnasher_Anim"), "--name", "gnasher", "--out", PACK]
        + texargs([("*", "Tex_COG_Gnasher")]), quiet=False)

    print("3/4 packing sounds ...")
    run([sys.executable, os.path.join(ROOT, "tools", "gears_sound_pack.py"), "--xma-dir", SND, "--out", PACK], quiet=False)

    print("4/4 aim offsets ...")
    gg = os.path.join(UNPACKED, "GearGame.xxx")
    if not os.path.exists(gg) and a.decompress:
        os.makedirs(UNPACKED, exist_ok=True)
        run([a.decompress, "-lzx", f"-out={UNPACKED}", os.path.join(cooked, "GearGame.xxx")], quiet=False)
    if os.path.exists(gg):
        run([sys.executable, os.path.join(ROOT, "tools", "gears_aimoffset.py"), gg, "--out", PACK], quiet=False)
    else:
        print("   skipped: give --decompress (UE Viewer's decompress tool) to read the aim-offset data")

    print("done →", PACK)
    for f in ["marcus.glb", "lancer.glb", "gnasher.glb", "gears_sounds.json", "marcus_aim.json"]:
        print(f"   {'ok ' if os.path.exists(os.path.join(PACK, f)) else 'MISSING'} {f}")


if __name__ == "__main__":
    main()
