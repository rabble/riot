"""Newswire renderer: projects the RIGOROUS signed /2 export into HTML."""

from __future__ import annotations

import base64
import hashlib
import json
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

REAL_EXPORT = ROOT.parents[1] / "fixtures" / "newswire" / "gateway-space" / "public-export-v1.json"


def _v2_entry(entry_id, title, body, signer="s" * 64, featured=False, verified=False,
              ai_assisted=False, verification_status="signature_verified", tai=1):
    return {
        "entry_id": entry_id,
        "signer": signer,
        "kind": "post",
        "title": title,
        "body": body,
        "ai_assisted": ai_assisted,
        "tai_j2000_micros": tai,
        "featured": featured,
        "editorially_verified": verified,
        "verification_status": verification_status,
    }


def _v2_export(entries, contributors=None, title="RIOT · Newswire", namespace="n" * 64):
    return {
        "schema": "riot-public-gateway-export/2",
        "title": title,
        "namespace": namespace,
        "generated_at": "2026-07-17T00:00:00Z",
        "entries": entries,
        "contributors": contributors or [],
    }


def _text(html_fragment: str) -> str:
    return re.sub(r"<[^>]+>", "", html_fragment)


@unittest.skipIf(nw is None, "newswire module missing")
class NewswireV2MappingTest(unittest.TestCase):
    def test_featured_go_to_front_page_and_all_valid_to_open_wire(self) -> None:
        internal = nw._from_v2(
            _v2_export([
                _v2_entry("f" * 64, "Lead", "b", featured=True),
                _v2_entry("w" * 64, "Wire", "b", featured=False),
            ])
        )
        self.assertEqual([p["entry_id"] for p in internal["front_page"]], ["f" * 64])
        self.assertEqual({p["entry_id"] for p in internal["open_wire"]}, {"f" * 64, "w" * 64})

    def test_signature_invalid_entries_are_dropped(self) -> None:
        internal = nw._from_v2(
            _v2_export([
                _v2_entry("g" * 64, "Good", "b"),
                _v2_entry("b" * 64, "Bad", "b", verification_status="signature_invalid"),
            ])
        )
        ids = {p["entry_id"] for p in internal["open_wire"]}
        self.assertEqual(ids, {"g" * 64})

    def test_byline_is_the_display_name_only_no_key_tag(self) -> None:
        signer = "c" * 64
        internal = nw._from_v2(
            _v2_export(
                [_v2_entry("k" * 64, "t", "b", signer=signer, featured=True)],
                contributors=[{"author_id": signer, "display_name": "Harbor Desk",
                               "is_organizer": False, "contribution_count": 1}],
            )
        )
        author = internal["front_page"][0]["author"]
        self.assertEqual(author["rendered"], "Harbor Desk")  # name only, no "·tag"
        self.assertEqual(author["display_name"], "Harbor Desk")

    def test_a_card_less_author_is_a_nameless_open_contributor(self) -> None:
        internal = nw._from_v2(_v2_export([_v2_entry("z" * 64, "t", "b", signer="0" * 64)]))
        self.assertEqual(internal["open_wire"][0]["author"]["rendered"], "Open contributor")


@unittest.skipIf(nw is None, "newswire module missing")
class NewswireRenderTest(unittest.TestCase):
    def setUp(self) -> None:
        signer = "a" * 64
        self.export = nw._from_v2(
            _v2_export(
                [
                    _v2_entry("e1" * 32, "Rent strike jumps three more blocks",
                              "Four hundred households…", signer=signer, featured=True, verified=True),
                    _v2_entry("e2" * 32, "Medic station open at the old library",
                              "West entrance staffed.", signer=signer, featured=False),
                ],
                contributors=[{"author_id": signer, "display_name": "Ana",
                               "is_organizer": True, "contribution_count": 3}],
            )
        )
        self.page = nw.render_newswire(self.export)

    def test_front_page_and_open_wire_render(self) -> None:
        self.assertIn("Rent strike jumps three more blocks", self.page)
        self.assertIn("Medic station open", self.page)

    def test_verified_and_unverified_are_legible(self) -> None:
        self.assertIn("verified", self.page.lower())
        self.assertIn("unverified", self.page.lower())

    def test_headlines_and_authors_link(self) -> None:
        self.assertIn(f'/post/{"e1" * 32}/', self.page)
        self.assertIn(f'/author/{"a" * 64}/', self.page)

    def test_deep_link_is_namespace_not_descriptor(self) -> None:
        self.assertIn("riot://open?namespace=", self.page)
        self.assertNotIn("descriptor=", self.page)

    def test_post_permalink_shows_provenance_and_a_namespace_entry_link(self) -> None:
        post = self.export["front_page"][0]
        page = nw.render_post(self.export, post)
        self.assertIn(post["entry_id"], page)
        self.assertIn(post["author"]["id"], page)  # signer disclosed as provenance
        self.assertIn("Four hundred households", page)
        self.assertIn(f"riot://open?namespace={self.export['space']['namespace']}&entry={post['entry_id']}", page)

    def test_author_page_aggregates_that_signers_posts(self) -> None:
        page = nw.render_author(self.export, "a" * 64)
        self.assertIn("Rent strike jumps three more blocks", page)
        self.assertIn("recognized organizer", page)
        self.assertIn("Ana", page)

    def test_publish_page_explains_publishing_from_the_app(self) -> None:
        page = nw.render_publish(self.export)
        self.assertIn("Publish from the Riot app", page)
        self.assertIn("read-only", page.lower())
        self.assertIn("riot://open?namespace=", page)
        self.assertIn("connect-src 'none'", page)

    def test_about_page_covers_the_collective_and_censorship_model(self) -> None:
        page = nw.render_about(self.export)
        self.assertIn("Many mirrors", page)
        self.assertIn("Signed, not trusted", page)
        self.assertIn(f'/author/{"a" * 64}/', page)
        self.assertIn("connect-src 'none'", page)

    def test_footer_links_to_about(self) -> None:
        self.assertIn('href="/about/"', self.page)

    def test_csp_baked_in_and_no_external_fetches(self) -> None:
        styles = re.findall(r"<style>(.*?)</style>", self.page, re.S)
        digest = base64.b64encode(hashlib.sha256(styles[0].encode("utf-8")).digest()).decode("ascii")
        self.assertIn(f"style-src 'sha256-{digest}'", self.page)
        self.assertIn("connect-src 'none'", self.page)
        self.assertNotRegex(self.page, r'(src|href)="https?://(?!www\.w3\.org)')


@unittest.skipIf(nw is None, "newswire module missing")
class NewswireRealGoldenRoundTripTest(unittest.TestCase):
    """Ratification anchor: EVERY page type renders from the REAL committed /2
    golden with #19's richness intact, display-name bylines, and no /1 artifact."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.raw = json.loads(REAL_EXPORT.read_text(encoding="utf-8"))
        cls.export = nw.load_export(REAL_EXPORT)
        cls.signers = {e["signer"] for e in cls.raw["entries"]}

    def test_home_renders_the_real_signed_content_no_v1_artifact(self) -> None:
        page = nw.render_newswire(self.export)
        self.assertIn("Port workers walk out", page)  # a real golden headline
        self.assertIn("RIOT Editorial Desk", page)  # the organizer's display name
        self.assertIn("riot://open?namespace=", page)
        self.assertNotIn("descriptor=", page)  # /1 deep-link form gone
        # No raw signer key in any visible byline (display name only).
        for byline in re.findall(r'class="byline".*?</div>', page, re.S):
            for signer in self.signers:
                self.assertNotIn(signer, _text(byline), "a key leaked into a visible byline")

    def test_every_page_type_survives_on_v2(self) -> None:
        post = nw.all_posts(self.export)[0]
        post_page = nw.render_post(self.export, post)
        self.assertIn(post["entry_id"], post_page)
        self.assertIn("riot://open?namespace=", post_page)
        editor = next(c for c in self.raw["contributors"] if not c["is_organizer"])
        author_page = nw.render_author(self.export, editor["author_id"])
        self.assertIn(editor["display_name"], author_page)  # "Harbor Desk"
        self.assertIn("Publish from the Riot app", nw.render_publish(self.export))
        self.assertIn("Many mirrors", nw.render_about(self.export))

    def test_signature_invalid_would_be_dropped(self) -> None:
        # The golden has none invalid; prove the drop on a synthetic /2.
        internal = nw._from_v2(
            _v2_export([_v2_entry("q" * 64, "Bad", "b", verification_status="signature_invalid")])
        )
        self.assertEqual(internal["open_wire"], [])


if __name__ == "__main__":
    unittest.main()
