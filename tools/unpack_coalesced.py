#!/usr/bin/env python3
"""Unpack a Gears of War 3 (Xbox 360) Coalesced_<LANG>.bin into plain .ini files.

Format (observed on Gears 3 360): big-endian int32 counts; strings are UE FStrings
where a negative length means UTF-16 *little-endian* characters including a null.
  int32 file_count
  per file:    FString path, int32 section_count
  per section: FString name,  int32 key_count
  per key:     FString key,   FString value

Reads your own dump; writes .ini files into the output folder (keep it in _local/).

Usage:
  python tools/unpack_coalesced.py Coalesced_INT.bin _local/re/gears3/config
"""
import os
import struct
import sys


def main(src, out):
    d = open(src, "rb").read()
    p = 0

    def i32():
        nonlocal p
        v = struct.unpack_from(">i", d, p)[0]
        p += 4
        return v

    def fstr():
        nonlocal p
        n = i32()
        if n == 0:
            return ""
        if n < 0:
            s = d[p:p - 2 * n].decode("utf-16-le")
            p += -2 * n
        else:
            s = d[p:p + n].decode("latin-1")
            p += n
        return s.rstrip("\0")

    files = i32()
    written = 0
    for _ in range(files):
        path = fstr().replace("\\", "/")
        name = path.split("/")[-1]
        lines = []
        for _ in range(i32()):
            lines.append(f"[{fstr()}]")
            for _ in range(i32()):
                k, v = fstr(), fstr()
                lines.append(f"{k}={v}")
            lines.append("")
        os.makedirs(out, exist_ok=True)
        with open(os.path.join(out, name), "w", encoding="utf-8") as f:
            f.write("\n".join(lines))
        written += 1
    print(f"{written} of {files} files written to {out}; parsed {p} of {len(d)} bytes")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
