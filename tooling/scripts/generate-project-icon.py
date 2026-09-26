"""Generate the Windows-compatible CPROJ icon from the checked-in artwork."""

from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "src-tauri" / "icons" / "project-source.png"
TARGET = ROOT / "src-tauri" / "icons" / "project.ico"
SIZES = [(size, size) for size in (16, 24, 32, 48, 64, 128, 256)]

with Image.open(SOURCE) as artwork:
    artwork.convert("RGBA").save(
        TARGET, format="ICO", sizes=SIZES, bitmap_format="bmp"
    )

print(f"Generated {TARGET}")
