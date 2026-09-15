#!/usr/bin/env python3
"""Render app-icon.svg to the PNG sizes Tauri expects.

Uses ImageMagick (magick) if available, else falls back to macOS qlmanage.
"""
import os, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SVG = os.path.join(HERE, "app-icon.svg")

# sizes used by the .icns iconset (name, size in px)
ICONSET = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
]


def render_with_magick(out, size):
    # ImageMagick's SVG delegate always paints a white canvas (ignores
    # -background none). The Blogger icon is a full circle, so we render it and
    # then cut the corners off with a circular alpha mask. The white "B" glyph
    # stays untouched because the mask is geometry-based, not colour-based.
    tmp = str(out) + ".render.png"
    subprocess.run(
        [
            "magick",
            SVG,
            "-resize",
            f"{size}x{size}",
            "-depth",
            "8",
            "-type",
            "TrueColorAlpha",
            tmp,
        ],
        check=True,
    )
    with tempfile.TemporaryDirectory() as td:
        mask = os.path.join(td, "mask.png")
        # circle inscribed in the square: center (c,c), radius c where c=size/2
        c = size / 2
        r = c - 0.5  # keep 0.5px inside so the tangent edge stays opaque
        subprocess.run(
            [
                "magick",
                "-size",
                f"{size}x{size}",
                "xc:none",
                "-fill",
                "white",
                "-draw",
                f"circle {c},{c} {r},0",
                "-depth",
                "8",
                mask,
            ],
            check=True,
        )
        subprocess.run(
            [
                "magick",
                tmp,
                mask,
                "-alpha",
                "off",
                "-compose",
                "CopyOpacity",
                "-composite",
                "-colorspace",
                "sRGB",
                "-type",
                "TrueColorAlpha",
                "-define",
                "png:color-type=6",
                out,
            ],
            check=True,
        )
    os.remove(tmp)


def render_with_qlmanage(out, size):
    ql = subprocess.run(
        ["qlmanage", "-t", "-s", str(size), "-o", os.path.dirname(out), SVG],
        capture_output=True,
        text=True,
    )
    if ql.returncode != 0:
        raise RuntimeError(ql.stderr)
    # qlmanage writes "<name>.svg.png"
    src = SVG + ".png"
    if not os.path.exists(src):
        raise RuntimeError("qlmanage did not produce an output png")
    os.replace(src, out)


def make_icns(out_dir, use_magick):
    if sys.platform != "darwin" or shutil.which("iconutil") is None:
        return
    icon_set = out_dir / "AppIcon.iconset"
    shutil.rmtree(icon_set, ignore_errors=True)
    icon_set.mkdir(parents=True, exist_ok=True)
    for name, size in ICONSET:
        tmp = icon_set / name
        if use_magick:
            render_with_magick(tmp, size)
        else:
            render_with_qlmanage(tmp, size)
    icns = out_dir / "icon.icns"
    subprocess.run(
        ["iconutil", "-c", "icns", str(icon_set), "-o", str(icns)],
        check=True,
    )
    shutil.rmtree(icon_set, ignore_errors=True)
    print("wrote", icns)


def make_ico(out_dir, use_magick):
    if not use_magick:
        return
    sizes = [16, 24, 32, 48, 64, 128, 256]
    files = []
    for s in sizes:
        tmp = out_dir / f"_ico_{s}.png"
        render_with_magick(tmp, s)
        files.append(str(tmp))
    ico = out_dir / "icon.ico"
    subprocess.run(["magick"] + files + [str(ico)], check=True)
    for f in files:
        os.remove(f)
    print("wrote", ico)


def main():
    from pathlib import Path

    default = os.path.join(HERE, "..", "src-tauri", "icons")
    out_dir = Path(sys.argv[1] if len(sys.argv) > 1 else default)
    out_dir.mkdir(parents=True, exist_ok=True)

    use_magick = shutil.which("magick") is not None
    for size in (32, 128, 256, 512):
        out = out_dir / f"icon-{size}.png"
        if use_magick:
            render_with_magick(out, size)
        else:
            render_with_qlmanage(out, size)
        print("wrote", out)

    icon_png = out_dir / "icon.png"
    if use_magick:
        render_with_magick(icon_png, 512)
    else:
        render_with_qlmanage(icon_png, 512)
    print("wrote", icon_png)

    make_icns(out_dir, use_magick)
    make_ico(out_dir, use_magick)


if __name__ == "__main__":
    main()