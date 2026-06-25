#!/usr/bin/env python3
import glob
import json
import os
import sqlite3

home = os.environ.get("CODEX_HOME") or os.path.join(os.environ["USERPROFILE"], ".codex")
db = os.path.join(home, "state_5.sqlite")
print("home", home)
print("db exists", os.path.isfile(db))
conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
cur = conn.cursor()
cur.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
print("tables", [r[0] for r in cur.fetchall()])
cur.execute("PRAGMA table_info(threads)")
print("threads cols", [r[1] for r in cur.fetchall()])
cur.execute(
    "SELECT id, title, cwd, model, rollout_path, source, created_at, updated_at "
    "FROM threads ORDER BY updated_at DESC LIMIT 3"
)
rows = cur.fetchall()
for r in rows:
    print("thread", r)
if rows:
    p = rows[0][4]
    if p and not os.path.isabs(p):
        p = os.path.join(home, p)
    print("rollout", p, os.path.isfile(p) if p else None)
    if p and os.path.isfile(p):
        with open(p, encoding="utf-8") as f:
            for i, line in enumerate(f):
                if i >= 10:
                    break
                print(line[:400].rstrip())
