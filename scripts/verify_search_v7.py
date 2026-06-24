#!/usr/bin/env python3
"""Verify schema v7 migration and source-list / bm25 search optimizations."""

from __future__ import annotations

import os
import sqlite3
import sys
from pathlib import Path


def default_db_path() -> Path:
    appdata = os.environ.get("APPDATA")
    if not appdata:
        raise SystemExit("APPDATA not set; pass db path as first argument")
    return Path(appdata) / "com.manxi.chatlens" / "chatlens.db"


def explain(conn: sqlite3.Connection, title: str, sql: str, params: tuple = ()) -> None:
    print(f"\n--- {title} ---")
    for row in conn.execute(f"EXPLAIN QUERY PLAN {sql}", params):
        print(row)


def main() -> None:
    db_path = Path(sys.argv[1]) if len(sys.argv) > 1 else default_db_path()
    if not db_path.exists():
        raise SystemExit(f"Database not found: {db_path}")

    print(f"DB: {db_path}")
    conn = sqlite3.connect(db_path)

    version = conn.execute(
        "SELECT value FROM meta WHERE key = 'schema_version'"
    ).fetchone()
    print(f"schema_version: {version[0] if version else 'N/A'}")

    indexes = [
        row[0]
        for row in conn.execute(
            """
            SELECT name FROM sqlite_master
            WHERE type = 'index' AND name LIKE 'idx_conversations%'
            ORDER BY name
            """
        )
    ]
    print("conversation indexes:", ", ".join(indexes) or "(none)")

    has_v7_index = "idx_conversations_source_starred_update" in indexes
    print("v7 index present:", has_v7_index)

    explain(
        conn,
        "Source list filter (cursor)",
        """
        SELECT c.id
        FROM conversations c
        WHERE c.source = ?
        ORDER BY c.is_starred DESC, COALESCE(c.update_time, c.create_time, 0) DESC
        LIMIT 50
        """,
        ("cursor",),
    )

    explain(
        conn,
        "FTS search ordered by bm25",
        """
        SELECT f.message_id
        FROM messages_fts f
        JOIN messages m ON m.id = f.message_id
        JOIN conversations c ON c.id = f.conversation_id
        WHERE messages_fts MATCH ?
        ORDER BY bm25(messages_fts)
        LIMIT 10
        """,
        ('"test"',),
    )

    null_sources = conn.execute(
        "SELECT COUNT(*) FROM conversations WHERE source IS NULL"
    ).fetchone()[0]
    print(f"\nconversations with NULL source: {null_sources}")

    conn.close()


if __name__ == "__main__":
    main()
