"""Two-column newswire render profile — features (E) + open wire (W)."""

from __future__ import annotations

import base64
import hashlib
from pathlib import Path
import re
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

try:
    import newswire as nw
except ModuleNotFoundError:
    nw = None


class NewswireRenderTest(unittest.TestCase):
    def setUp(self) -> None:
        self.assertIsNotNone(nw, "the newswire render profile does not exist")
        self.view = nw.sample_view()
        self.page = nw.render_newswire(self.view)

    def test_sample_view_is_flagged_demo_not_real_signed_content(self) -> None:
        # Honest boundary: this profile awaits real E/W content. The sample must
        # never be mistaken for signed data from a namespace.
        self.assertTrue(self.view.sample)
        self.assertIn("sample", self.page.lower())

    def test_renders_editorial_and_open_wire_as_separate_sections(self) -> None:
        self.assertIn("editorial", self.page.lower())
        self.assertRegex(self.page, r'class="[^"]*\bwire\b')
        # Every editorial article and every wire post makes it onto the page.
        for entry in self.view.editorial:
            self.assertIn(entry.title, self.page)
        for post in self.view.wire:
            self.assertIn(post.body, self.page)

    def test_trust_tiers_are_legible_editorial_verified_wire_open(self) -> None:
        # The security-legible distinction (composite spec §6): verified editorial
        # must never look like an anonymous open post.
        self.assertIn("verified", self.page.lower())
        self.assertIn("unverified", self.page.lower())

    def test_carries_the_open_in_riot_deep_link_not_web_as_truth(self) -> None:
        self.assertIn(f"riot://open?namespace={self.view.namespace}", self.page)

    def test_csp_is_baked_in_and_style_hash_matches_the_stylesheet(self) -> None:
        styles = re.findall(r"<style>(.*?)</style>", self.page, re.S)
        self.assertEqual(len(styles), 1)
        digest = base64.b64encode(hashlib.sha256(styles[0].encode("utf-8")).digest()).decode("ascii")
        self.assertIn(f"style-src 'sha256-{digest}'", self.page)
        self.assertIn("connect-src 'none'", self.page)
        self.assertIn("script-src 'none'", self.page)

    def test_no_external_fetch_references(self) -> None:
        # xmlns / riot:// are identifiers, not fetches; nothing else may reach out.
        self.assertNotRegex(self.page, r'(src|href)="https?://(?!www\.w3\.org)')


if __name__ == "__main__":
    unittest.main()
