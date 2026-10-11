#!/usr/bin/env python3
"""Inspect Unreal Engine 3 Xbox 360 packages (.xxx) from your own Gears dump.

Reads only file headers. Prints, per package: endianness, file version / licensee
version, package flags, compression method, chunk count, and name/export/import
counts. Optionally verifies files against the game's Xbox360TOC.txt MD5 list.

Never writes into the game files. Output is a report only.

Usage:
  python tools/inspect_packages.py <CookedXbox360 dir or .xxx files...> [--toc Xbox360TOC.txt]
"""
import argparse
import hashlib
import os
import struct
import sys

TAG = 0x9E2A83C1
COMPRESSION = {0: "none", 1: "zlib", 2: "lzo", 4: "lzx"}


class Reader:
    def __init__(self, data, endian):
        self.d, self.p, self.e = data, 0, endian

    def u32(self):
        v = struct.unpack_from(self.e + "I", self.d, self.p)[0]
        self.p += 4
        return v

    def i32(self):
        v = struct.unpack_from(self.e + "i", self.d, self.p)[0]
        self.p += 4
        return v

    def fstring(self):
        n = self.i32()
        if n == 0:
            return ""
        if n > 0:
            s = self.d[self.p:self.p + n].split(b"\0")[0].decode("latin-1")
            self.p += n
        else:
            s = self.d[self.p:self.p - 2 * n].decode("utf-16-be" if self.e == ">" else "utf-16-le").rstrip("\0")
            self.p += -2 * n
        return s


def guess_codec(b):
    """Rough guess from the first bytes of a compressed block."""
    if len(b) < 2:
        return "unknown"
    if b[0] == 0x78 and b[1] in (0x01, 0x5E, 0x9C, 0xDA):
        return "zlib"
    if b[:4] == bytes.fromhex("9e2a83c1"):
        return "uncompressed"
    return "not zlib (likely Xbox LZX)"


def inspect(path):
    with open(path, "rb") as f:
        head = f.read(65536)
    out = {"file": os.path.basename(path), "size": os.path.getsize(path)}
    if len(head) < 8:
        out["error"] = "too small"
        return out
    if struct.unpack_from(">I", head)[0] == TAG:
        endian = ">"
    elif struct.unpack_from("<I", head)[0] == TAG:
        endian = "<"
    else:
        out["error"] = "bad package tag (encrypted or not a UE3 package)"
        out["first_bytes"] = head[:16].hex()
        return out
    r = Reader(head, endian)
    r.u32()
    second = r.u32()
    # "Fully compressed" packages (script packages on 360): tag, block size, total
    # compressed size, total uncompressed size, then (compressed, uncompressed) per block.
    if second in (0x20000, 0x10000, 0x8000):
        comp, uncomp = r.u32(), r.u32()
        out["layout"] = "fully compressed"
        out["block_size"] = hex(second)
        out["compressed"] = comp
        out["uncompressed"] = uncomp
        blocks = -(-uncomp // second)
        out["blocks"] = blocks
        data_start = 16 + 8 * blocks
        out["codec_guess"] = guess_codec(head[data_start:data_start + 4])
        return out
    ver = second
    out["layout"] = "standard header"
    out["endian"] = "big (Xbox 360)" if endian == ">" else "little"
    out["version"] = ver & 0xFFFF
    out["licensee"] = ver >> 16
    out["header_size"] = r.i32()
    out["folder"] = r.fstring()
    out["flags"] = hex(r.u32())
    out["names"] = r.i32(); r.i32()
    out["exports"] = r.i32(); r.i32()
    out["imports"] = r.i32(); r.i32()
    r.i32()  # depends offset
    v = out["version"]
    if v >= 623:
        r.i32(); r.i32(); r.i32()  # import/export guids
    if v >= 584:
        r.i32()  # thumbnail table
    r.p += 16  # guid
    gens = r.i32()
    if 0 <= gens < 64:
        r.p += 12 * gens
        out["engine"] = r.i32()
        out["cooker"] = r.i32()
        comp = r.u32()
        out["compression"] = COMPRESSION.get(comp, f"unknown({comp})")
        out["chunks"] = r.i32()
        if out["chunks"] > 0:
            r.i32(); r.i32()
            comp_off = r.i32()
            # each compressed chunk starts with its own tag + block header
            hdr = Reader(open(path, "rb").read(comp_off + 4096)[comp_off:], endian)
            if hdr.u32() == TAG:
                bs = hdr.u32(); hdr.u32(); total = hdr.u32()
                n = -(-total // bs) if bs else 0
                start = 16 + 8 * n
                out["codec_guess"] = guess_codec(hdr.d[start:start + 4])
    else:
        out["note"] = f"unexpected generation count {gens}; header layout differs, check manually"
    return out


def load_toc(path):
    toc = {}
    with open(path, encoding="latin-1") as f:
        for line in f:
            parts = line.strip().split(" ")
            if len(parts) >= 4:
                toc[parts[2].replace("\\", "/").split("/")[-1].lower()] = (int(parts[0]), parts[3])
    return toc


def md5(path):
    h = hashlib.md5()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+")
    ap.add_argument("--toc", help="Xbox360TOC.txt to verify sizes and MD5s against")
    a = ap.parse_args()
    files = []
    for p in a.paths:
        if os.path.isdir(p):
            files += sorted(os.path.join(p, n) for n in os.listdir(p) if n.lower().endswith(".xxx"))
        else:
            files.append(p)
    toc = load_toc(a.toc) if a.toc else {}
    for p in files:
        info = inspect(p)
        if toc:
            want = toc.get(os.path.basename(p).lower())
            if not want:
                info["toc"] = "not listed"
            elif want[0] != info["size"]:
                info["toc"] = f"SIZE MISMATCH (toc {want[0]})"
            else:
                info["toc"] = "md5 ok" if md5(p) == want[1] else "MD5 MISMATCH"
        print("  ".join(f"{k}={v}" for k, v in info.items()))


if __name__ == "__main__":
    sys.exit(main())
