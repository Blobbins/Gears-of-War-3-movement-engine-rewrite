#!/usr/bin/env python3
"""Decode UnrealScript bytecode from uncompressed UE3 packages (version 828,
Gears of War 3, Xbox 360, big-endian) into readable pseudo-code.

Our own implementation. Token layouts follow the public UE3 format as documented by
UELib (github.com/EliotVU/Unreal-Library, MIT) and UE Viewer. The *output* is
decompiled game code: write it only under _local/ (never commit it).

Output is flat pseudo-code with jump labels (no if/else rebuilding), which is enough
to read the logic. Each line starts with the memory offset used by jumps.

Usage:
  python tools/ue3_bytecode.py --pkg GearGame.xxx --lib Core.xxx Engine.xxx GameFramework.xxx \
      --func GSM_Run2Cover.CanChainMove        # one function (or a class prefix)
  python tools/ue3_bytecode.py ... --class GSM_Run2Cover --out _local/re/gears3/script
  python tools/ue3_bytecode.py ... --selftest  # decode every function, report failures
"""
import argparse
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ue3_tables import Pkg  # noqa: E402

FUNC_FLAGS = {
    0x1: "final", 0x4: "iterator", 0x8: "latent", 0x10: "preoperator", 0x20: "singular",
    0x40: "net", 0x100: "simulated", 0x200: "exec", 0x400: "native", 0x800: "event",
    0x1000: "operator", 0x2000: "static", 0x8000: "const", 0x40000: "private",
    0x80000: "protected", 0x100000: "delegate", 0x200000: "server", 0x1000000: "client",
}

OPERATOR_SYMBOLS = [
    ("AddAdd", "++"), ("SubtractSubtract", "--"), ("AddEqual", "+="), ("SubtractEqual", "-="),
    ("MultiplyEqual", "*="), ("DivideEqual", "/="), ("ConcatEqual", "$="), ("AtEqual", "@="),
    ("MultiplyMultiply", "**"), ("EqualEqual", "=="), ("NotEqual", "!="), ("LessEqual", "<="),
    ("GreaterEqual", ">="), ("LessLess", "<<"), ("GreaterGreaterGreater", ">>>"),
    ("GreaterGreater", ">>"), ("AndAnd", "&&"), ("OrOr", "||"), ("XorXor", "^^"),
    ("ComplementEqual", "~="), ("Add", "+"), ("Subtract", "-"), ("Multiply", "*"),
    ("Divide", "/"), ("Percent", "%"), ("Less", "<"), ("Greater", ">"), ("Not", "!"),
    ("Complement", "~"), ("And", "&"), ("Or", "|"), ("Xor", "^"), ("Concat", "$"), ("At", "@"),
    ("Dot", "Dot"), ("Cross", "Cross"), ("ClockwiseFrom", "ClockwiseFrom"),
]

PRIMITIVE_CASTS = {  # UE3 cast codes (per UELib UnrealTokens.cs CastToken)
    0x36: "InterfaceToObject", 0x37: "InterfaceToString", 0x38: "InterfaceToBool",
    0x39: "RotatorToVector", 0x3A: "ByteToInt", 0x3B: "ByteToBool", 0x3C: "ByteToFloat",
    0x3D: "IntToByte", 0x3E: "IntToBool", 0x3F: "IntToFloat", 0x40: "BoolToByte",
    0x41: "BoolToInt", 0x42: "BoolToFloat", 0x43: "FloatToByte", 0x44: "FloatToInt",
    0x45: "FloatToBool", 0x46: "ObjectToInterface", 0x47: "ObjectToBool",
    0x48: "NameToBool", 0x49: "StringToByte", 0x4A: "StringToInt", 0x4B: "StringToBool",
    0x4C: "StringToFloat", 0x4D: "StringToVector", 0x4E: "StringToRotator",
    0x4F: "VectorToBool", 0x50: "VectorToRotator", 0x51: "RotatorToBool",
    0x52: "ByteToString", 0x53: "IntToString", 0x54: "BoolToString",
    0x55: "FloatToString", 0x56: "ObjectToString", 0x57: "NameToString",
    0x58: "VectorToString", 0x59: "RotatorToString", 0x5A: "DelegateToString",
    0x60: "StringToName",
}

CAST_TO = {k: v.split("To")[1].lower() for k, v in PRIMITIVE_CASTS.items()}


class Decoder:
    def __init__(self, pkg, natives):
        self.pk = pkg
        self.natives = natives
        self.e = pkg.e

    # --- stream helpers (storage position p, memory position m) -------------
    def u8(self):
        v = self.d[self.p]; self.p += 1; self.m += 1; return v

    def u16(self):
        v = struct.unpack_from(self.e + "H", self.d, self.p)[0]; self.p += 2; self.m += 2; return v

    def i32(self):
        v = struct.unpack_from(self.e + "i", self.d, self.p)[0]; self.p += 4; self.m += 4; return v

    def f32(self):
        v = struct.unpack_from(self.e + "f", self.d, self.p)[0]; self.p += 4; self.m += 4; return v

    def obj(self):
        v = struct.unpack_from(self.e + "i", self.d, self.p)[0]; self.p += 4; self.m += 8
        return self.pk.obj_name(v) if v else "None"

    def name(self):
        idx, num = struct.unpack_from(self.e + "ii", self.d, self.p); self.p += 8; self.m += 8
        n = self.pk.names[idx] if 0 <= idx < len(self.pk.names) else f"<name{idx}>"
        return n if num == 0 else f"{n}_{num - 1}"

    def zstr(self, wide=False):
        if wide:
            out = []
            while True:
                c = struct.unpack_from(self.e + "H", self.d, self.p)[0]; self.p += 2; self.m += 2
                if c == 0:
                    return "".join(out)
                out.append(chr(c))
        end = self.d.index(b"\0", self.p)
        s = self.d[self.p:end].decode("latin-1")
        n = end - self.p + 1
        self.p += n; self.m += n
        return s

    # --- expressions ---------------------------------------------------------
    def params(self):
        args = []
        while True:
            if self.d[self.p] == 0x16:  # EndFunctionParms
                self.u8(); break
            args.append(self.expr())
        while args and args[-1] == "":
            args.pop()
        return args

    def call(self, fname, args, flags=0, prec=0):
        if flags & 0x1000 and len(args) == 2:  # binary operator
            sym = op_symbol(fname)
            return f"({args[0]} {sym} {args[1]})"
        if (flags & 0x10 or flags & 0x1000) and len(args) == 1:
            sym = op_symbol(fname)
            if fname.endswith("_Post") or "Post" in fname.split("_")[-1]:
                return f"({args[0]}{sym})"
            return f"({sym}{args[0]})"
        return f"{fname}({', '.join(args)})"

    def expr(self):
        t = self.u8()
        if t >= 0x70 or 0x60 <= t <= 0x6F:
            idx = t if t >= 0x70 else ((t - 0x60) << 8) | self.u8()
            nat = self.natives.get(idx, (f"native{idx}", 0, 0))
            return self.call(nat[0], self.params(), nat[1], nat[2])
        h = HANDLERS.get(t)
        if h is None:
            raise ValueError(f"unknown token 0x{t:02X} at storage {self.p - 1}")
        return h(self)

    def decode(self, data, start):
        """Decode one script blob: list of (memory_offset, text)."""
        self.d, self.p, self.m = data, start, 0
        end = len(data)
        out = []
        while self.p < end:
            at = self.m
            if self.d[self.p] == 0x53:
                self.u8(); out.append((at, "// end of script")); break
            out.append((at, self.expr()))
        return out


def op_symbol(fname):
    base = fname.split("_")[0]
    for k, v in OPERATOR_SYMBOLS:
        if base == k:
            return v
    return fname


def _ctx(d, sep="."):
    left = d.expr(); d.u16(); d.obj(); d.u8(); right = d.expr()
    return f"{left}{sep}{right}"


def _switch(d):
    d.obj(); d.u8()
    return f"switch ({d.expr()})"


def _case(d):
    nxt = d.u16()
    if nxt == 0xFFFF:
        return "default:"
    return f"case {d.expr()}:   // next case @{nxt:04X}"


def _jumpifnot(d):
    target = d.u16()
    return f"if (!{d.expr()}) goto L{target:04X}"


def _dynarray_find(d):
    arr = d.expr(); d.u16(); val = d.expr(); d.u8()
    return f"{arr}.Find({val})"


def _dynarray_find_struct(d):
    arr = d.expr(); d.u16(); a = d.expr(); b = d.expr(); d.u8()
    return f"{arr}.Find({a}, {b})"


def _dynarray_item(name, n):
    def f(d):
        arr = d.expr(); d.u16(); args = [d.expr() for _ in range(n)]; d.u8()
        return f"{arr}.{name}({', '.join(args)})"
    return f


def _dynarray_plain(name, n):
    def f(d):
        arr = d.expr(); args = [d.expr() for _ in range(n)]; d.u8()
        return f"{arr}.{name}({', '.join(args)})"
    return f


def _iterator(d):
    e = d.expr(); end = d.u16()
    return f"foreach {e}   // ends @{end:04X}"


def _dynarray_iter(d):
    arr = d.expr(); item = d.expr(); has_index = d.u8(); index = d.expr(); end = d.u16()
    return f"foreach {arr}({item}{', ' + index if has_index else ''})   // ends @{end:04X}"


def _conditional(d):
    c = d.expr(); d.u16(); a = d.expr(); d.u16(); b = d.expr()
    return f"({c} ? {a} : {b})"


def _virtual(d):
    n = d.name()
    return d.call(n, d.params())


def _final(d):
    n = d.obj()
    return d.call(n, d.params())


def _global(d):
    n = d.name()
    return d.call("global." + n, d.params())


def _delegate_function(d):
    d.u8(); prop = d.obj(); n = d.name()
    return d.call(f"{prop}<delegate {n}>", d.params())


def _default_parm(d):
    d.u16(); v = d.expr(); d.u8()
    return f"/* default = {v} */"


def _new(d):
    a = [d.expr() for _ in range(4)]
    return f"new({a[0]}, {a[1]}, {a[2]}) {a[3]}"


def _struct_member(d):
    prop = d.obj(); d.obj(); d.u8(); d.u8()
    return f"{d.expr()}.{prop}"


def _primitive_cast(d):
    c = d.u8()
    return f"{CAST_TO.get(c, f'cast{c:02X}')}({d.expr()})"


def _debug_info(d):
    d.i32(); line = d.i32(); d.i32(); d.u8()
    return f"// line {line}"


HANDLERS = {
    0x00: lambda d: d.obj(),                                   # LocalVariable
    0x01: lambda d: d.obj(),                                   # InstanceVariable
    0x02: lambda d: "default." + d.obj(),                      # DefaultVariable
    0x03: lambda d: d.obj(),                                   # StateVariable
    0x04: lambda d: (lambda v: "return" if v in ("", "return") else f"return {v}")(d.expr()),  # Return
    0x05: _switch,
    0x06: lambda d: f"goto L{d.u16():04X}",                    # Jump
    0x07: _jumpifnot,
    0x08: lambda d: "stop",
    0x09: lambda d: (d.u16(), d.u8(), f"assert({d.expr()})")[2],
    0x0A: _case,
    0x0B: lambda d: "",                                        # Nothing
    0x0D: lambda d: f"goto label {d.expr()}",
    0x0E: lambda d: (d.obj(), d.expr())[1],                    # EatReturnValue
    0x0F: lambda d: f"{d.expr()} = {d.expr()}",                # Let
    0x10: lambda d: (lambda i, a: f"{a}[{i}]")(d.expr(), d.expr()),  # DynArrayElement
    0x11: _new,
    0x12: lambda d: _ctx(d, ".static."),                       # ClassContext
    0x13: lambda d: f"class<{d.obj()}>({d.expr()})",           # MetaCast
    0x14: lambda d: f"{d.expr()} = {d.expr()}",                # LetBool
    0x15: lambda d: "",                                        # EndParmValue
    0x16: lambda d: "",                                        # EndFunctionParms
    0x17: lambda d: "self",
    0x18: lambda d: (d.u16(), d.expr())[1],                    # Skip
    0x19: _ctx,                                                # Context
    0x1A: lambda d: (lambda i, a: f"{a}[{i}]")(d.expr(), d.expr()),  # ArrayElement
    0x1B: _virtual,
    0x1C: _final,
    0x1D: lambda d: str(d.i32()),
    0x1E: lambda d: f"{d.f32():g}",
    0x1F: lambda d: '"' + d.zstr() + '"',
    0x20: lambda d: d.obj(),
    0x21: lambda d: "'" + d.name() + "'",
    0x22: lambda d: f"rot({d.i32()}, {d.i32()}, {d.i32()})",
    0x23: lambda d: f"vect({d.f32():g}, {d.f32():g}, {d.f32():g})",
    0x24: lambda d: str(d.u8()),
    0x25: lambda d: "0",
    0x26: lambda d: "1",
    0x27: lambda d: "true",
    0x28: lambda d: "false",
    0x29: lambda d: d.obj(),                                   # NativeParm
    0x2A: lambda d: "None",
    0x2C: lambda d: str(d.u8()),                               # IntConstByte
    0x2D: lambda d: d.expr(),                                  # BoolVariable
    0x2E: lambda d: f"{d.obj()}({d.expr()})",                  # DynamicCast
    0x2F: _iterator,
    0x30: lambda d: "iterator pop",
    0x31: lambda d: "iterator next",
    0x32: lambda d: (d.obj(), f"({d.expr()} == {d.expr()})")[1],
    0x33: lambda d: (d.obj(), f"({d.expr()} != {d.expr()})")[1],
    0x34: lambda d: '"' + d.zstr(wide=True) + '"',
    0x35: _struct_member,
    0x36: lambda d: f"{d.expr()}.Length",
    0x37: _global,
    0x38: _primitive_cast,
    0x39: _dynarray_plain("Insert", 2),
    0x3A: lambda d: (d.obj(), "return")[1],                    # ReturnNothing
    0x3B: lambda d: f"({d.expr()} == {d.expr()})",
    0x3C: lambda d: f"({d.expr()} != {d.expr()})",
    0x3D: lambda d: f"({d.expr()} == {d.expr()})",
    0x3E: lambda d: f"({d.expr()} != {d.expr()})",
    0x3F: lambda d: "None",                                    # EmptyDelegate
    0x40: _dynarray_plain("Remove", 2),
    0x41: _debug_info,
    0x42: _delegate_function,
    0x43: lambda d: (d.name(), d.obj())[0],                    # DelegateProperty
    0x44: lambda d: f"{d.expr()} = {d.expr()}",                # LetDelegate
    0x45: _conditional,
    0x46: _dynarray_find,
    0x47: _dynarray_find_struct,
    0x48: lambda d: d.obj(),                                   # OutVariable
    0x49: _default_parm,
    0x4A: lambda d: "",                                        # EmptyParmValue
    0x4B: lambda d: d.name(),                                  # InstanceDelegate
    0x51: lambda d: d.expr(),                                  # InterfaceContext
    0x52: lambda d: f"{d.obj()}({d.expr()})",                  # InterfaceCast
    0x54: _dynarray_plain("Add", 1),
    0x55: _dynarray_item("AddItem", 1),
    0x56: _dynarray_item("RemoveItem", 1),
    0x57: _dynarray_item("InsertItem", 2),
    0x58: _dynarray_iter,
    0x59: _dynarray_item("Sort", 1),
}


# --- function headers ---------------------------------------------------------

def read_struct_header(pk, e):
    """Return (data, script_start, storage_size, memory_size, after_script) for a UStruct export."""
    d = pk.d[e["offset"]:e["offset"] + e["size"]]
    p = 4                      # NetIndex
    if e["flags"] & 0x02000000:  # HasStack: state frame (classes/states) — not used for functions
        raise ValueError("HasStack objects not supported here")
    p += 8                     # property list terminator: 'None'
    p += 4                     # UField.Next
    p += 4                     # UStruct.Super
    p += 4                     # UStruct.Children
    mem, store = struct.unpack_from(pk.e + "ii", d, p); p += 8
    return d, p, store, mem, p + store


def function_meta(pk, e):
    d, start, store, mem, after = read_struct_header(pk, e)
    inative = struct.unpack_from(pk.e + "H", d, after)[0]
    prec = d[after + 2]
    flags = struct.unpack_from(pk.e + "I", d, after + 3)[0]
    return d, start, store, mem, inative, prec, flags


def build_natives(pkgs):
    table = {}
    for pk in pkgs:
        for e in pk.exports:
            if pk.obj_name(e["class"]) != "Function":
                continue
            try:
                _, _, _, _, inat, prec, flags = function_meta(pk, e)
            except Exception:
                continue
            if inat:
                table[inat] = (e["name"], flags, prec)
    return table


def params_of(pk, index):
    """Names of a function's parameters and locals (its child properties)."""
    out = []
    for i, e in enumerate(pk.exports):
        if e["outer"] == index + 1 and pk.obj_name(e["class"]).endswith("Property"):
            out.append((pk.obj_name(e["class"]), e["name"]))
    return out


def render(pk, dec, index):
    e = pk.exports[index]
    d, start, store, mem, inat, prec, flags = function_meta(pk, e)
    words = [w for bit, w in FUNC_FLAGS.items() if flags & bit]
    head = f"{' '.join(words)} function {pk.path(index)}".strip()
    lines = [head]
    for cls, n in params_of(pk, index):
        lines.append(f"    // {cls.replace('Property', '').lower()} {n}")
    if flags & 0x400 and store <= 2:
        lines.append("    // native: body is C++ in the executable")
        return "\n".join(lines)
    for at, text in dec.decode(d[:start + store], start):
        if text:
            lines.append(f"  {at:04X}: {text}")
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pkg", required=True)
    ap.add_argument("--lib", nargs="*", default=[], help="packages that define native operators (Core, Engine...)")
    ap.add_argument("--func", help="exact Class.Function path")
    ap.add_argument("--class", dest="cls", help="decode every function of this class (and its states)")
    ap.add_argument("--out", help="write <Class>.uc.txt here instead of stdout")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()

    pk = Pkg(a.pkg)
    libs = [Pkg(p) for p in a.lib]
    dec = Decoder(pk, build_natives(libs + [pk]))

    funcs = [i for i, e in enumerate(pk.exports) if pk.obj_name(e["class"]) == "Function"]

    if a.selftest:
        ok = native = bad = 0
        fails = {}
        for i in funcs:
            e = pk.exports[i]
            try:
                d, start, store, mem, inat, prec, flags = function_meta(pk, e)
                if flags & 0x400 and store <= 2:
                    native += 1; continue
                dec.decode(d[:start + store], start)
                if dec.p != start + store or d[start + store - 1] != 0x53 or dec.m != mem:
                    raise ValueError(f"consumed {dec.p - start}/{store} storage, {dec.m}/{mem} memory")
                ok += 1
            except Exception as ex:
                bad += 1
                key = str(ex).split(" at ")[0]
                fails.setdefault(key, []).append(pk.path(i))
        print(f"decoded OK: {ok}   native-only: {native}   failed: {bad}")
        for k, v in sorted(fails.items(), key=lambda x: -len(x[1]))[:15]:
            print(f"  {len(v):5d}  {k}   e.g. {v[0]}")
        return

    sel = []
    for i in funcs:
        path = pk.path(i)
        if a.func and path == a.func:
            sel.append(i)
        elif a.cls and (path.startswith(a.cls + ".")):
            sel.append(i)
    if not sel:
        sys.exit("nothing matched")
    text = "\n\n".join(render(pk, dec, i) for i in sel) + "\n"
    if a.out:
        os.makedirs(a.out, exist_ok=True)
        name = (a.cls or a.func).replace(".", "_") + ".uc.txt"
        with open(os.path.join(a.out, name), "w") as f:
            f.write(text)
        print(f"wrote {len(sel)} functions -> {os.path.join(a.out, name)}")
    else:
        print(text)


if __name__ == "__main__":
    main()
