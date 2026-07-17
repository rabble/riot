"""build.py smoke test: the cf-mirror freezes the newswire site from the /2 export."""

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
        self.raw = json.loads(EXPORT.read_text(encoding="utf-8"))
        self._tmp = tempfile.TemporaryDirectory()
        self.dist = Path(self._tmp.name)
        build.build_newswire(self.dist)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_home_publish_about_render_from_the_v2_export(self) -> None:
        home = (self.dist / "index.html").read_text(encoding="utf-8")
        self.assertIn("Port workers walk out", home)  # a real /2 headline
        self.assertIn("RIOT Editorial Desk", home)  # organizer display name
        self.assertNotIn("descriptor=", home)  # /1 deep-link form gone
        self.assertTrue((self.dist / "publish" / "index.html").exists())
        self.assertTrue((self.dist / "about" / "index.html").exists())

    def test_a_post_and_author_page_are_emitted(self) -> None:
        entry_id = self.raw["entries"][0]["entry_id"]
        self.assertTrue((self.dist / "post" / entry_id / "index.html").exists())
        editor = next(c for c in self.raw["contributors"] if not c["is_organizer"])
        author_page = self.dist / "author" / editor["author_id"] / "index.html"
        self.assertTrue(author_page.exists())
        self.assertIn(editor["display_name"], author_page.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
