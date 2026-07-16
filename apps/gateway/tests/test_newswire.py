"""Newswire renderer: projects a REAL signed-record export into HTML."""

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


def _post(entry_id, headline, body, author_id="aaaa", rendered="Ana · aaaa",
          verified=False, treatment="Ordinary"):
    return {
        "entry_id": entry_id,
        "author": {"id": author_id, "display_name": "Ana", "tag": "aaaa", "rendered": rendered},
        "tai_j2000_micros": 1,
        "headline": headline,
        "body": body,
        "ai_assisted": False,
        "verified": verified,
        "treatment": treatment,
    }


def _export(front_page, open_wire, contributors):
    return {
        "schema": "riot.newswire.export/1",
        "space": {"name": "RIOT · Newswire", "descriptor_entry_id": "d" * 64},
        "front_page": front_page,
        "open_wire": open_wire,
        "contributors": contributors,
    }


class NewswireRenderTest(unittest.TestCase):
    def setUp(self) -> None:
        self.assertIsNotNone(nw, "the newswire renderer does not exist")
        feat = _post("e1" * 32, "Rent strike jumps three more blocks", "Four hundred households…", verified=True)
        wire = _post("e2" * 32, "Medic station open at the old library", "West entrance staffed.", verified=False)
        self.export = _export([feat], [feat, wire], [
            {"id": "aaaa", "display_name": "Ana", "rendered": "Ana · aaaa", "is_organizer": True, "contribution_count": 3},
        ])
        self.page = nw.render_newswire(self.export)

    def test_real_export_fixture_loads_if_present(self) -> None:
        path = ROOT.parents[1] / "fixtures" / "newswire" / "newswire-export-v1.json"
        if not path.is_file():
            self.skipTest("run the riot-ffi generator to produce the real export")
        export = nw.load_export(path)
        self.assertIn("front_page", export)
        self.assertIn("open_wire", export)
        # entry ids are real content hashes (64 hex chars).
        self.assertRegex(export["open_wire"][0]["entry_id"], r"^[0-9a-f]{64}$")

    def test_front_page_and_open_wire_render_from_the_projection(self) -> None:
        self.assertIn("Rent strike jumps three more blocks", self.page)   # featured
        self.assertIn("Medic station open", self.page)                    # open wire

    def test_verified_flag_comes_from_the_record_not_the_renderer(self) -> None:
        self.assertIn("verified", self.page.lower())
        # the unverified wire post is marked open/unverified
        self.assertIn("unverified", self.page.lower())

    def test_headlines_and_authors_link(self) -> None:
        self.assertIn(f'/post/{"e1" * 32}/', self.page)   # headline permalink = entry_id
        self.assertIn('/author/aaaa/', self.page)          # byline → author profile

    def test_post_permalink_shows_provenance(self) -> None:
        post = self.export["front_page"][0]
        page = nw.render_post(self.export, post)
        self.assertIn(post["entry_id"], page)               # the real entry id
        self.assertIn(post["author"]["id"], page)           # the real signer id
        self.assertIn("Four hundred households", page)       # full body

    def test_author_page_aggregates_that_signers_posts(self) -> None:
        page = nw.render_author(self.export, "aaaa")
        self.assertIn("Rent strike jumps three more blocks", page)
        self.assertIn("recognized organizer", page)

    def test_moderation_hidden_and_tombstoned_posts_do_not_render(self) -> None:
        hidden = _post("f0" * 32, "Should be hidden", "secret", treatment="Hidden")
        tomb = _post("f1" * 32, "Should be tombstoned", "gone", treatment="Tombstoned")
        ok = _post("f2" * 32, "Visible report", "shown")
        export = _export([], [hidden, tomb, ok], [])
        page = nw.render_newswire(export)
        self.assertIn("Visible report", page)
        self.assertNotIn("Should be hidden", page)
        self.assertNotIn("Should be tombstoned", page)

    def test_csp_baked_in_and_no_external_fetches(self) -> None:
        styles = re.findall(r"<style>(.*?)</style>", self.page, re.S)
        digest = base64.b64encode(hashlib.sha256(styles[0].encode("utf-8")).digest()).decode("ascii")
        self.assertIn(f"style-src 'sha256-{digest}'", self.page)
        self.assertIn("connect-src 'none'", self.page)
        self.assertNotRegex(self.page, r'(src|href)="https?://(?!www\.w3\.org)')


if __name__ == "__main__":
    unittest.main()
