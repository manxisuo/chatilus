"""Compare legacy vs Manifest v1 ChatGPT export directories."""
from __future__ import annotations

import json
import os
from collections import Counter
from pathlib import Path

OLD = Path(r"D:\Personal\ChatGPT数据下载\2026-05-16-12-08-35")
NEW = Path(r"D:\Personal\ChatGPT数据下载\2026-06-26")


def scan(root: Path) -> dict:
    if not root.is_dir():
        return {"path": str(root), "exists": False}

    ext_bytes: Counter[str] = Counter()
    ext_count: Counter[str] = Counter()
    total = 0
    files = 0
    dirs = 0
    max_depth = 0
    conv_count = 0

    for dirpath, dirnames, filenames in os.walk(root):
        rel = Path(dirpath).relative_to(root)
        depth = 0 if str(rel) == "." else len(rel.parts)
        max_depth = max(max_depth, depth)
        dirs += len(dirnames)
        for fn in filenames:
            p = Path(dirpath) / fn
            try:
                sz = p.stat().st_size
            except OSError:
                continue
            total += sz
            files += 1
            ext = p.suffix.lower() or "(no ext)"
            ext_bytes[ext] += sz
            ext_count[ext] += 1
            if fn.startswith("conversations") and fn.endswith(".json"):
                try:
                    conv_count += len(json.loads(p.read_text(encoding="utf-8")))
                except Exception:
                    pass

    top_level = sorted(
        [(p.name, p.stat().st_size) for p in root.iterdir() if p.is_file()],
        key=lambda x: -x[1],
    )[:8]

    return {
        "path": str(root),
        "exists": True,
        "total_gb": total / 1024**3,
        "total_mb": total / 1024**2,
        "files": files,
        "dirs": dirs,
        "max_depth": max_depth,
        "conv_count": conv_count,
        "ext_bytes": ext_bytes.most_common(12),
        "ext_count": ext_count.most_common(12),
        "top_files": top_level,
        "has_manifest": (root / "export_manifest.json").is_file(),
    }


def image_pointers(root: Path) -> tuple[int, int]:
    resolved = missing = 0
    if not root.is_dir():
        return 0, 0
    for cf in root.glob("conversations*.json"):
        try:
            convs = json.loads(cf.read_text(encoding="utf-8"))
        except Exception:
            continue
        for c in convs:
            for node in (c.get("mapping") or {}).values():
                msg = (node or {}).get("message")
                if not msg:
                    continue
                for p in (msg.get("content") or {}).get("parts") or []:
                    if not isinstance(p, dict) or not p.get("asset_pointer"):
                        continue
                    ptr = p["asset_pointer"]
                    key = ptr.replace("file-service://", "").replace("sediment://", "")
                    candidates = [
                        root / f"{key}.dat",
                        root / f"{key}.jpg",
                        root / f"{key}.png",
                    ]
                    if any(c.is_file() for c in candidates):
                        resolved += 1
                    else:
                        # search nested (legacy)
                        found = any(root.rglob(f"{Path(key).name}*"))
                        if found:
                            resolved += 1
                        else:
                            missing += 1
    return resolved, missing


def print_report(label: str, data: dict) -> None:
    print(f"\n{'=' * 60}")
    print(f"{label}: {data['path']}")
    if not data.get("exists"):
        print("  (目录不存在)")
        return
    print(f"  总大小: {data['total_gb']:.2f} GB ({data['total_mb']:.0f} MB)")
    print(f"  文件数: {data['files']}, 子目录数: {data['dirs']}, 最大深度: {data['max_depth']}")
    print(f"  会话数(conversations*.json): {data['conv_count']}")
    print(f"  export_manifest.json: {'有' if data['has_manifest'] else '无'}")
    count_map = dict(data["ext_count"])
    print("  按扩展名体积 / 数量:")
    for ext, sz in data["ext_bytes"]:
        print(f"    {ext:10s} {sz / 1024**2:8.1f} MB  ({count_map.get(ext, 0)} files)")
    print("  根目录最大文件:")
    for name, sz in data["top_files"]:
        print(f"    {name:40s} {sz / 1024**2:8.1f} MB")


def main() -> None:
    old = scan(OLD)
    new = scan(NEW)
    print_report("旧导出 (legacy)", old)
    print_report("新导出 (Manifest v1)", new)

    if old.get("exists") and new.get("exists"):
        print(f"\n{'=' * 60}")
        print("对比摘要:")
        print(f"  体积比: 旧 {old['total_gb']:.2f} GB / 新 {new['total_gb']:.2f} GB = {old['total_gb']/new['total_gb']:.1f}x")
        print(f"  会话数: 旧 {old['conv_count']} / 新 {new['conv_count']} (差 {old['conv_count'] - new['conv_count']:+d})")

        old_img = image_pointers(OLD)
        new_img = image_pointers(NEW)
        print(f"  图片指针可解析: 旧 {old_img[0]}/{old_img[0]+old_img[1]}, 新 {new_img[0]}/{new_img[0]+new_img[1]}")


if __name__ == "__main__":
    main()
