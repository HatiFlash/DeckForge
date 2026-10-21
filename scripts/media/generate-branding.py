#!/usr/bin/env python3
"""Generate transparent NovyDeck logo assets from res/branding/novydeck-source.png."""
import argparse
import shutil
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
BRANDING = ROOT / "res" / "branding"
SIZES = (32, 64, 256)
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)


def validate_transparency(image: Image.Image, label: str) -> None:
    rgba = image.convert("RGBA")
    alpha = rgba.getchannel("A")
    histogram = alpha.histogram()
    if not alpha.getbbox() or histogram[0] < rgba.width * rgba.height // 5:
        raise ValueError(f"{label}: visible artwork and transparent exterior required")
    for xy in ((0, 0), (rgba.width - 1, 0), (0, rgba.height - 1), (rgba.width - 1, rgba.height - 1)):
        if alpha.getpixel(xy) != 0:
            raise ValueError(f"{label}: opaque corner at {xy}")
    if sum(histogram[1:255]) == 0:
        raise ValueError(f"{label}: antialiased alpha was lost")


def generate(preview: Path | None = None) -> None:
    source_path = BRANDING / "novydeck-source.png"
    with Image.open(source_path) as original:
        source = original.convert("RGBA")
    if source.size != (1536, 1024):
        raise ValueError("source must be the supplied 1536x1024 PNG")
    alpha = source.getchannel("A")
    low, high = alpha.getextrema()
    if low == 255 or high == 0:
        raise ValueError("source alpha must contain transparency and non-empty pixels")
    cropped = source.crop((240, 120, 1360, 952))
    canvas = Image.new("RGBA", (1120, 1120), (0, 0, 0, 0))
    canvas.alpha_composite(cropped, (0, (1120 - cropped.height) // 2))
    logo = canvas.resize((1024, 1024), Image.Resampling.LANCZOS)
    validate_transparency(logo, "1024px logo")
    logo.save(BRANDING / "novydeck-logo.png")
    variants = {}
    for size in SIZES:
        image = logo.resize((size, size), Image.Resampling.LANCZOS)
        validate_transparency(image, f"{size}px logo")
        image.save(BRANDING / f"novydeck-{size}.png")
        variants[size] = image
    logo.save(BRANDING / "novydeck.ico", format="ICO", sizes=[(s, s) for s in ICO_SIZES])
    (ROOT / "res" / "novydeck.ico").write_bytes((BRANDING / "novydeck.ico").read_bytes())
    with Image.open(BRANDING / "novydeck.ico") as ico:
        for size in ICO_SIZES:
            validate_transparency(ico.ico.getimage((size, size)), f"ICO {size}px")
    for destination in (ROOT / "scripts/media/remotion/public/logo.png", ROOT / "scripts/media/promo/assets/novydeck-logo.png"):
        if destination.parent.is_dir():
            logo.save(destination)
    shutil.copyfile(source_path, ROOT / "scripts/media/remotion/public/logo-original.png")
    if preview is not None:
        preview.mkdir(parents=True, exist_ok=True)
        for name, color in (("white", (255, 255, 255)), ("gray", (128, 128, 128)), ("dark", (24, 24, 24))):
            background = Image.new("RGBA", logo.size, (*color, 255))
            background.alpha_composite(logo)
            background.convert("RGB").save(preview / f"logo-{name}.png")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--preview", type=Path, help="optional control-preview output directory")
    args = parser.parse_args()
    generate(args.preview)
