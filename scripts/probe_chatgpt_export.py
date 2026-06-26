import json
import re
from pathlib import Path

root = Path(r"D:\Personal\ChatGPT数据下载\2026-06-26")


def pointer_key_from_filename(name: str) -> str | None:
    base = name.split("#")[0]
    if base.startswith("file-"):
        prefix = "file-"
    elif base.startswith("file_"):
        prefix = "file_"
    else:
        return None
    rest = base[len(prefix) :]
    id_end = rest.find("-")
    if id_end == -1:
        id_end = len(rest)
    file_id = rest[:id_end]
    return f"{prefix}{file_id}" if file_id else None


print("=== pointer_key_from_filename ===")
for name in [
    "file-UZjt2wbMlSFQEdh6nzVVybgI.dat",
    "file-UZjt2wbMlSFQEdh6nzVVybgI-1000036297.jpg",
    "file_0000000099e8722f88e5ee3819ccc378-1000018748.jpg",
]:
    print(name, "->", pointer_key_from_filename(name))

asset_map = json.loads((root / "conversation_asset_file_names.json").read_text(encoding="utf-8"))
resolved = missing = 0
samples_missing = []
for cf in root.glob("conversations-*.json"):
    convs = json.loads(cf.read_text(encoding="utf-8"))
    for c in convs:
        for node in (c.get("mapping") or {}).values():
            msg = (node or {}).get("message")
            if not msg:
                continue
            content = msg.get("content") or {}
            for p in content.get("parts") or []:
                if isinstance(p, dict) and p.get("asset_pointer"):
                    key = p["asset_pointer"].replace("file-service://", "")
                    if (root / f"{key}.dat").exists():
                        resolved += 1
                    else:
                        missing += 1
                        if len(samples_missing) < 5:
                            samples_missing.append((key, asset_map.get(f"{key}.dat")))

print(f"\nimage pointers: resolved={resolved} missing={missing}")
print("missing samples:", samples_missing)

sediment_samples = []
for cf in sorted(root.glob("conversations-*.json"))[:3]:
    text = cf.read_text(encoding="utf-8")
    sediment_samples.extend(re.findall(r"sediment://[^\"\\]+", text)[:2])
print("\nsediment samples:", sediment_samples[:4])

manifest = json.loads((root / "export_manifest.json").read_text(encoding="utf-8"))
print("\nmanifest version:", manifest.get("version"))
print("sharded conversations:", manifest["logical_files"]["conversations.json"])
