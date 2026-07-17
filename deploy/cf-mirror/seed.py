#!/usr/bin/env python3
"""Seed: project the newswire and push it into the worker's KV store.

This is the dynamic tier's ingest step. Today it projects from the local export
(build.py) and bulk-writes the rendered pages into CF KV; the worker reads KV
per request, so re-running this updates the live site with NO redeploy. When the
iroh transport lands, the "project" step gains a p2p sync in front of it — the
push-to-KV stays identical.

Run: python3 seed.py   (needs your Cloudflare login for the KV write)
"""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
NAMESPACE_ID = "94646e705b474307a38f61b169ab9d5e"
PUSH_SUFFIXES = {".html", ".json", ".svg"}


def main() -> None:
    # Project: render the current newswire into dist/.
    subprocess.run([sys.executable, "build.py"], cwd=HERE, check=True)

    dist = HERE / "dist"
    entries = [
        {"key": str(path.relative_to(dist)), "value": path.read_text(encoding="utf-8")}
        for path in sorted(dist.rglob("*"))
        if path.is_file() and path.suffix in PUSH_SUFFIXES
    ]
    bulk = HERE / "kv-bulk.json"
    bulk.write_text(json.dumps(entries), encoding="utf-8")

    print(f"seeding {len(entries)} keys → KV {NAMESPACE_ID}")
    # --remote: write the REAL edge KV the deployed worker reads (wrangler v4
    # defaults kv writes to local simulation, which the worker never sees).
    subprocess.run(
        ["npx", "wrangler", "kv", "bulk", "put", str(bulk), "--namespace-id", NAMESPACE_ID, "--remote"],
        cwd=HERE,
        check=True,
    )
    print("seeded — the live site now reflects the store (no redeploy needed).")


if __name__ == "__main__":
    main()
