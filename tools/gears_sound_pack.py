"""Bundle Gears 3 weapon sounds (exported from your own dump) into one file for the
test range: gears_sounds.json = {name: base64 MP3}.

  1. export with UE Viewer (needs -sounds):
     umodel -sounds -export -out=_local/exports/sounds GearGame.xxx <Wave> SoundNodeWave
  2. python tools/gears_sound_pack.py --xma-dir _local/exports/sounds --out _local/exports/pack

Xbox 360 XMA is decoded with ffmpeg (xma2 decoder) to MP3. Output must stay in _local/:
these are Gears files.
"""
import argparse
import base64
import glob
import json
import os
import subprocess
import sys
import tempfile


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--xma-dir", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    if "_local" not in os.path.abspath(a.out).replace("\\", "/").split("/"):
        sys.exit("refusing to write outside _local/: exported Gears data must stay local")
    out = {}
    with tempfile.TemporaryDirectory() as tmp:
        for f in sorted(glob.glob(os.path.join(a.xma_dir, "**", "*.xma"), recursive=True)):
            name = os.path.splitext(os.path.basename(f))[0]
            mp3 = os.path.join(tmp, name + ".mp3")
            base = ["ffmpeg", "-v", "error", "-y", "-i", f]
            enc = ["-c:a", "libmp3lame", "-q:a", "3", mp3]
            r = subprocess.run(base + enc)
            if r.returncode != 0 or not os.path.exists(mp3) or os.path.getsize(mp3) == 0:
                # some waves switch channel layout mid-stream: force stereo
                subprocess.run(base + ["-af", "aresample,aformat=channel_layouts=stereo"] + enc)
            if os.path.exists(mp3) and os.path.getsize(mp3) > 0:
                out[name] = base64.b64encode(open(mp3, "rb").read()).decode()
            else:
                print("could not convert", name)
    os.makedirs(a.out, exist_ok=True)
    path = os.path.join(a.out, "gears_sounds.json")
    json.dump({"libgears_sounds": 1, "sounds": out}, open(path, "w"))
    print(f"wrote {path} ({os.path.getsize(path) / 1e3:.0f} KB), {len(out)} sounds")


if __name__ == "__main__":
    main()
