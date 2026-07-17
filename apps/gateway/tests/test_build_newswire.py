"""build.py smoke test: the cf-mirror freezes the REAL newswire export."""

from __future__ import annotations

import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]  # apps/gateway
REPO_ROOT = ROOT.parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(REPO_ROOT / "deploy" / "cf-mirror"))

try:
    import build
except ModuleNotFoundError:
    build = None

EXPORT = REPO_ROOT / "fixtures" / "newswire" / "gateway-space" / "public-export-v1.json"


@unittest.skipIf(build is None, "cf-mirror build module missing")
class BuildNewswireTest(unittest.TestCase):
    def setUp(self) -> None:
        self.export = json.loads(EXPORT.read_text(encoding="utf-8"))
        self._tmp = tempfile.TemporaryDirectory()
        self.dist = Path(self._tmp.name)
        build.build_newswire(self.dist, EXPORT)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_index_is_the_real_export_not_the_demo(self) -> None:
        home = (self.dist / "index.html").read_text(encoding="utf-8")
        # A real headline from the committed golden, the organizer's display name,
        # and NOT the demo sentinel.
        self.assertIn("Port workers walk out", home)
        self.assertIn("RIOT Editorial Desk", home)
        self.assertNotIn("demo · sample content", home)

    def test_demo_route_keeps_the_flagged_sample(self) -> None:
        demo = (self.dist / "demo" / "index.html").read_text(encoding="utf-8")
        self.assertIn("demo · sample content", demo)

    def test_a_post_page_is_emitted_per_entry(self) -> None:
        entry_id = self.export["entries"][0]["entry_id"]
        page = self.dist / "post" / entry_id / "index.html"
        self.assertTrue(page.exists(), "each post has its own page")

    def test_a_named_contributor_gets_an_author_page(self) -> None:
        editor = next(c for c in self.export["contributors"] if not c["is_organizer"])
        page = self.dist / "author" / editor["author_id"] / "index.html"
        self.assertTrue(page.exists(), "the named editor has an author page")
        self.assertIn(editor["display_name"], page.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
