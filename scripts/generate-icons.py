#!/usr/bin/env python3
"""从带透明通道的 PNG 生成 ChatLens 桌面图标（保留 RGBA，不填黑底）。"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "src-tauri" / "icons"
DEFAULT_SRC = ROOT / "assets" / "chatlens-original.png"
TARGET_SIZE = 1024


def prepare_source(src: Path, dest: Path) -> None:
    img = Image.open(src).convert("RGBA")
    scale = TARGET_SIZE / max(img.size)
    nw, nh = (int(img.width * scale), int(img.height * scale))
    resized = img.resize((nw, nh), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (TARGET_SIZE, TARGET_SIZE), (0, 0, 0, 0))
    canvas.paste(resized, ((TARGET_SIZE - nw) // 2, (TARGET_SIZE - nh) // 2), resized)
    dest.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(dest, "PNG")


def main() -> int:
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_SRC
    if not src.is_file():
        print(f"源图不存在: {src}", file=sys.stderr)
        return 1

    square = ICONS / "chatlens-source-square.png"
    prepare_source(src, square)

    subprocess.run(
        ["npm", "run", "tauri", "icon", str(square.relative_to(ROOT)).replace("\\", "/")],
        cwd=ROOT,
        check=True,
    )

    for mobile in (ICONS / "android", ICONS / "ios"):
        if mobile.exists():
            shutil.rmtree(mobile)

    public = ROOT / "public"
    shutil.copy2(ICONS / "128x128.png", public / "favicon.png")
    shutil.copy2(ICONS / "32x32.png", public / "favicon-32.png")

    print(f"已生成桌面图标，源图: {src}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
