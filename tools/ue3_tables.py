#!/usr/bin/env python3
"""Read the name/import/export tables of an *uncompressed* UE3 package (version 828,
Gears of War 3, Xbox 360, big-endian) and list its objects.

Decompress first with UE Viewer's `decompress` tool. Reads only; writes a report.
Layout reference: UE Viewer source, Unreal/UnrealPackage/UnPackage3.cpp (MIT-style
licence, credit Gildor). This file is our own implementation, no game data inside.

Usage:
  python tools/ue3_tables.py <package.xxx> [--class Class|Function|State|...]
        [--grep REGEX] [--tsv out.tsv]
"""
import argparse
import re
import struct
import sys


class Pkg:
    def __init__(self, path):
        self.d = open(path, "rb").read()
        self.e = ">" if self.d[:4] == bytes.fromhex("9e2a83c1") else "<"
        self.p = 0
        self._summary()
        self._names()
        self._imports()
        self._exports()

    # primitives
    def i32(self):
        v = struct.unpack_from(self.e + "i", self.d, self.p)[0]; self.p += 4; return v

    def u32(self):
        v = struct.unpack_from(self.e + "I", self.d, self.p)[0]; self.p += 4; return v

    def u64(self):
        v = struct.unpack_from(self.e + "Q", self.d, self.p)[0]; self.p += 8; return v

    def fstring(self):
        n = self.i32()
        if n == 0:
            return ""
        if n > 0:
            s = self.d[self.p:self.p + n]; self.p += n
            return s.rstrip(b"\0").decode("latin-1")
        s = self.d[self.p:self.p - 2 * n]; self.p += -2 * n
        return s.decode("utf-16-be" if self.e == ">" else "utf-16-le").rstrip("\0")

    def fname(self):
        idx, num = self.i32(), self.i32()
        base = self.names[idx] if 0 <= idx < len(self.names) else f"<bad {idx}>"
        return base if num == 0 else f"{base}_{num - 1}"

    # tables
    def _summary(self):
        if self.u32() != 0x9E2A83C1:
            raise SystemExit("not an uncompressed UE3 package (decompress it first)")
        v = self.u32()
        self.ver, self.lic = v & 0xFFFF, v >> 16
        self.i32()  # header size
        self.fstring()  # folder
        self.flags = self.u32()
        self.name_count, self.name_off = self.i32(), self.i32()
        self.export_count, self.export_off = self.i32(), self.i32()
        self.import_count, self.import_off = self.i32(), self.i32()

    def _names(self):
        self.p = self.name_off
        self.names = []
        for _ in range(self.name_count):
            self.names.append(self.fstring())
            self.u64()

    def _imports(self):
        self.p = self.import_off
        self.imports = []
        for _ in range(self.import_count):
            cpkg, cname = self.fname(), self.fname()
            outer = self.i32()
            name = self.fname()
            self.imports.append({"class_pkg": cpkg, "class": cname, "outer": outer, "name": name})

    def _exports(self):
        self.p = self.export_off
        self.exports = []
        for _ in range(self.export_count):
            cls, sup, outer = self.i32(), self.i32(), self.i32()
            name = self.fname()
            arch = self.i32()
            oflags = self.u64()
            size = self.i32()
            off = self.i32()
            eflags = self.u32()
            n = self.i32()
            self.p += 4 * n  # NetObjectCount
            self.p += 16     # Guid
            self.i32()       # package flags
            self.exports.append({"class": cls, "super": sup, "outer": outer, "name": name,
                                 "arch": arch, "flags": oflags, "size": size, "offset": off})

    # helpers
    def obj_name(self, ref):
        if ref > 0:
            return self.exports[ref - 1]["name"]
        if ref < 0:
            return self.imports[-ref - 1]["name"]
        return "Class"

    def path(self, i):
        """Full dotted path of export i (0-based)."""
        parts, ref = [], i + 1
        while ref:
            if ref > 0:
                e = self.exports[ref - 1]; parts.append(e["name"]); ref = e["outer"]
            else:
                m = self.imports[-ref - 1]; parts.append(m["name"]); ref = m["outer"]
        return ".".join(reversed(parts))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("package")
    ap.add_argument("--class", dest="cls", help="only exports of this class (e.g. Class, Function, State)")
    ap.add_argument("--grep", help="regex on the full object path (case-insensitive)")
    ap.add_argument("--tsv", help="write all exports to a TSV file")
    ap.add_argument("--summary", action="store_true", help="counts per class only")
    a = ap.parse_args()
    pk = Pkg(a.package)
    print(f"version {pk.ver}/{pk.lic}  names {len(pk.names)}  imports {len(pk.imports)}  exports {len(pk.exports)}",
          file=sys.stderr)
    rx = re.compile(a.grep, re.I) if a.grep else None
    rows = []
    for i, e in enumerate(pk.exports):
        cls = pk.obj_name(e["class"])
        if a.cls and cls != a.cls:
            continue
        path = pk.path(i)
        if rx and not rx.search(path):
            continue
        rows.append((i + 1, cls, path, pk.obj_name(e["super"]) if e["super"] else "", e["size"], e["offset"]))
    if a.summary:
        from collections import Counter
        for c, n in Counter(r[1] for r in rows).most_common(40):
            print(f"{n:7d}  {c}")
        return
    if a.tsv:
        with open(a.tsv, "w") as f:
            f.write("index\tclass\tpath\tsuper\tsize\toffset\n")
            for r in rows:
                f.write("\t".join(map(str, r)) + "\n")
        print(f"wrote {len(rows)} rows to {a.tsv}", file=sys.stderr)
    else:
        for r in rows:
            print(f"{r[0]:7d}  {r[1]:<22} {r[2]}" + (f"  : {r[3]}" if r[3] else ""))


if __name__ == "__main__":
    main()
