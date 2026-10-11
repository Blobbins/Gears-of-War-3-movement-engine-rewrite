#!/usr/bin/env python3
"""Read enums and class default properties from an uncompressed UE3 v828 package.

  python tools/ue3_props.py <pkg> --enum ESpecialMove
  python tools/ue3_props.py <pkg> --defaults GSM_Run2Cover
  python tools/ue3_props.py <pkg> --find-enum-value SM_Run2Cover

Our own reader (format per public UE3 docs / UELib). Output is game data: keep it in
_local/ or quote individual numbers with their source in docs/GEARS_VALUES.md.
"""
import argparse
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ue3_tables import Pkg  # noqa: E402

# Native structs that are serialized raw (immutable) rather than as tagged properties.
RAW_STRUCTS = {
    "Vector": "fff", "Rotator": "iii", "Vector2D": "ff", "Color": "BBBB",
    "LinearColor": "ffff", "Plane": "ffff", "Quat": "ffff", "Guid": "IIII",
    "IntPoint": "ii", "Box": None, "Matrix": None, "Sphere": "ffff", "TwoVectors": "ffffff",
}


class Reader:
    def __init__(self, pk, data):
        self.pk, self.d, self.p, self.e = pk, data, 0, pk.e

    def i32(self):
        v = struct.unpack_from(self.e + "i", self.d, self.p)[0]; self.p += 4; return v

    def u8(self):
        v = self.d[self.p]; self.p += 1; return v

    def f32(self):
        v = struct.unpack_from(self.e + "f", self.d, self.p)[0]; self.p += 4; return v

    def name(self):
        idx, num = struct.unpack_from(self.e + "ii", self.d, self.p); self.p += 8
        n = self.pk.names[idx] if 0 <= idx < len(self.pk.names) else f"<name{idx}>"
        return n if num == 0 else f"{n}_{num - 1}"

    def fstring(self):
        n = self.i32()
        if n == 0:
            return ""
        if n > 0:
            s = self.d[self.p:self.p + n]; self.p += n
            return s.rstrip(b"\0").decode("latin-1")
        s = self.d[self.p:self.p - 2 * n]; self.p += -2 * n
        return s.decode("utf-16-be" if self.e == ">" else "utf-16-le").rstrip("\0")

    def obj(self):
        v = self.i32()
        return self.pk.obj_name(v) if v else "None"

    def props(self, depth=0):
        """Tagged property list until 'None'. Returns list of (name, index, value)."""
        out = []
        while True:
            name = self.name()
            if name == "None":
                return out
            typ = self.name()
            size = self.i32()
            index = self.i32()
            extra = None
            if typ == "StructProperty":
                extra = self.name()
            elif typ == "BoolProperty":
                out.append((name, index, bool(self.u8())))
                continue
            elif typ == "ByteProperty":
                extra = self.name()
            start = self.p
            try:
                val = self.value(typ, size, extra, depth)
            except Exception as ex:  # unknown layout: keep raw bytes, stay aligned
                val = f"<{typ} {size}B {ex}>"
            self.p = start + size
            out.append((name, index, val))

    def value(self, typ, size, extra, depth):
        if typ == "IntProperty":
            return self.i32()
        if typ == "FloatProperty":
            return round(self.f32(), 6)
        if typ == "NameProperty":
            return self.name()
        if typ in ("ObjectProperty", "ComponentProperty", "ClassProperty", "InterfaceProperty"):
            return self.obj()
        if typ == "ByteProperty":
            return self.name() if size == 8 else self.u8()
        if typ == "StrProperty":
            return self.fstring()
        if typ == "StructProperty":
            fmt = RAW_STRUCTS.get(extra, "?")
            if fmt and fmt != "?" and struct.calcsize(fmt) == size:
                vals = struct.unpack_from(self.e + fmt, self.d, self.p)
                return f"{extra}({', '.join(f'{v:g}' if isinstance(v, float) else str(v) for v in vals)})"
            if fmt == "?" and depth < 4:
                return {"struct": extra, "props": self.props(depth + 1)}
            return f"<{extra} raw {self.d[self.p:self.p + size].hex()}>"
        if typ == "ArrayProperty":
            n = self.i32()
            return f"<array of {n}, {size - 4} bytes: {self.d[self.p:self.p + min(size - 4, int(os.environ.get("UE3_ARRAY_BYTES", "48")))].hex()}>"
        if typ == "DelegateProperty":
            return f"{self.obj()}.{self.name()}"
        return f"<{typ} {self.d[self.p:self.p + min(size, 32)].hex()}>"


def find_export(pk, path):
    for i, e in enumerate(pk.exports):
        if pk.path(i) == path or e["name"] == path:
            return i, e
    return None, None


def read_enum(pk, e):
    r = Reader(pk, pk.d[e["offset"]:e["offset"] + e["size"]])
    r.i32()          # NetIndex
    r.props()        # (empty) property list
    r.i32()          # UField.Next
    n = r.i32()
    return [r.name() for _ in range(n)]


def read_defaults(pk, e):
    r = Reader(pk, pk.d[e["offset"]:e["offset"] + e["size"]])
    r.i32()          # NetIndex
    return r.props()


def fmt(props, indent="  "):
    lines = []
    for name, idx, val in props:
        key = f"{name}[{idx}]" if idx else name
        if isinstance(val, dict):
            lines.append(f"{indent}{key} = {val['struct']}(")
            lines += fmt(val["props"], indent + "    ")
            lines.append(f"{indent})")
        else:
            lines.append(f"{indent}{key} = {val}")
    return lines


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("pkg")
    ap.add_argument("--enum")
    ap.add_argument("--defaults", help="class name; reads Default__<class>")
    ap.add_argument("--find-enum-value", help="which enum contains this value name")
    ap.add_argument("--object", help="full object path (or name): print its properties")
    a = ap.parse_args()
    pk = Pkg(a.pkg)
    if a.enum:
        for i, e in enumerate(pk.exports):
            if e["name"] == a.enum and pk.obj_name(e["class"]) == "Enum":
                for n, v in enumerate(read_enum(pk, e)):
                    print(f"{n:4d}  {v}")
                print(f"({pk.path(i)})")
    if a.find_enum_value:
        for i, e in enumerate(pk.exports):
            if pk.obj_name(e["class"]) == "Enum":
                vals = read_enum(pk, e)
                if a.find_enum_value in vals:
                    print(pk.path(i), vals.index(a.find_enum_value))
    if a.object:
        i, e = find_export(pk, a.object)
        if e is None:
            sys.exit("no such object")
        print(f"{pk.path(i)} ({pk.obj_name(e['class'])})")
        print("\n".join(fmt(read_defaults(pk, e))))
    if a.defaults:
        i, e = find_export(pk, "Default__" + a.defaults)
        if e is None:
            sys.exit("no defaults object found")
        print(f"Default__{a.defaults} ({pk.obj_name(e['class'])})")
        print("\n".join(fmt(read_defaults(pk, e))))


if __name__ == "__main__":
    main()
