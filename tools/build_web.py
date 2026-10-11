#!/usr/bin/env python3
"""Inline web/gears_core.js into the test range pages -> web/dist/.

  web/range3d.html  -> web/dist/libgears_range3d.html   (third-person 3D, the main one)
  web/sandbox.html  -> web/dist/libgears_sandbox.html   (top-down 2D)

Each page must be a single self-contained file (the 3D page also loads three.js r128
from cdnjs). Run tools/parity.js first to confirm the JS port still matches Rust.
"""
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MARKER = "<script>/*__GEARS_CORE__*/</script>"
PAGES = {"range3d.html": "libgears_range3d.html", "sandbox.html": "libgears_sandbox.html"}

core = open(os.path.join(ROOT, "web", "gears_core.js"), encoding="utf-8").read()
os.makedirs(os.path.join(ROOT, "web", "dist"), exist_ok=True)
for src, dst in PAGES.items():
    page = open(os.path.join(ROOT, "web", src), encoding="utf-8").read()
    assert MARKER in page, f"core placeholder missing in {src}"
    out = page.replace(MARKER, "<script>\n" + core + "\n</script>")
    path = os.path.join(ROOT, "web", "dist", dst)
    with open(path, "w", encoding="utf-8") as f:
        f.write(out)
    print(f"wrote {path} ({len(out)} bytes)")
