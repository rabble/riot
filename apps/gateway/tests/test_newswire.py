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


def _entry(**kw) -> dict:
    base = {
        "entry_id": "a" * 64,
        "signer": "s" * 64,
        "kind": "post",
        "title": "A headline",
        "body": "Body text.",
        "ai_assisted": False,
        "tai_j2000_micros": 837_425_926_000_000,
        "featured": False,
        "editorially_verified": False,
        "verification_status": "signature_verified",
    }
    base.update(kw)
    return base


def _export(entries, contributors=None, **kw) -> dict:
    base = {
        "schema": "riot-public-gateway-export/2",
        "title": "RIOT · Independent Newswire",
        "namespace": "n" * 64,
        "generated_at": "2026-07-17T00:00:00Z",
        "entries": entries,
        "contributors": contributors or [],
    }
    base.update(kw)
    return base


@unittest.skipIf(nw is None, "newswire module missing")
class NewswireExportViewTest(unittest.TestCase):
    def test_featured_entries_become_editorial_and_the_rest_open_wire(self) -> None:
        export = _export(
            [
                _entry(entry_id="f" * 64, featured=True, title="Featured lead"),
                _entry(entry_id="w" * 64, featured=False, body="Open wire body"),
            ]
        )
        view = nw.newswire_view_from_export(export)
        self.assertEqual([e.title for e in view.editorial], ["Featured lead"])
        self.assertEqual([p.body for p in view.wire], ["Open wire body"])
        self.assertFalse(view.sample)

    def test_all_featured_is_editorial_only_and_none_featured_is_wire_only(self) -> None:
        e_only = nw.newswire_view_from_export(
            _export([_entry(entry_id="1" * 64, featured=True), _entry(entry_id="2" * 64, featured=True)])
        )
        self.assertEqual(len(e_only.editorial), 2)
        self.assertEqual(len(e_only.wire), 0)
        w_only = nw.newswire_view_from_export(
            _export([_entry(entry_id="3" * 64), _entry(entry_id="4" * 64)])
        )
        self.assertEqual(len(w_only.editorial), 0)
        self.assertEqual(len(w_only.wire), 2)

    def test_a_duplicate_entry_id_is_rendered_once(self) -> None:
        view = nw.newswire_view_from_export(
            _export([_entry(entry_id="d" * 64, featured=True), _entry(entry_id="d" * 64, featured=False)])
        )
        self.assertEqual(len(view.editorial) + len(view.wire), 1)

    def test_editorially_verified_drives_the_editorial_verified_flag(self) -> None:
        verified = nw.newswire_view_from_export(
            _export([_entry(entry_id="v" * 64, featured=True, editorially_verified=True)])
        )
        self.assertTrue(verified.editorial[0].verified)
        unverified = nw.newswire_view_from_export(
            _export([_entry(entry_id="u" * 64, featured=True, editorially_verified=False)])
        )
        self.assertFalse(unverified.editorial[0].verified)

    def test_signature_invalid_entries_are_dropped_entirely(self) -> None:
        view = nw.newswire_view_from_export(
            _export(
                [
                    _entry(entry_id="g" * 64, verification_status="signature_verified", body="good"),
                    _entry(entry_id="b" * 64, verification_status="signature_invalid", body="bad"),
                ]
            )
        )
        bodies = [p.body for p in view.wire]
        self.assertIn("good", bodies)
        self.assertNotIn("bad", bodies)

    def test_an_empty_export_renders_the_empty_state_without_crashing(self) -> None:
        view = nw.newswire_view_from_export(_export([]))
        self.assertEqual(view.editorial, ())
        self.assertEqual(view.wire, ())
        # The renderer must tolerate an empty view.
        page = nw.render_newswire(view)
        self.assertIn("Open Newswire", page)

    def test_byline_uses_the_display_name_and_falls_back_for_the_nameless(self) -> None:
        signer = "c" * 64
        export = _export(
            [
                _entry(entry_id="k" * 64, featured=True, signer=signer),
                _entry(entry_id="z" * 64, featured=False, signer="0" * 64),  # no card
            ],
            contributors=[{"author_id": signer, "display_name": "Harbor Desk", "is_organizer": False, "contribution_count": 1}],
        )
        view = nw.newswire_view_from_export(export)
        self.assertEqual(view.editorial[0].author, "Harbor Desk")
        self.assertEqual(view.wire[0].handle, "Open contributor")
        # And no raw signer hex leaks into a byline.
        page = nw.render_newswire(view)
        self.assertNotIn(signer, page)

    def test_ai_assisted_entries_carry_a_visible_marker(self) -> None:
        view = nw.newswire_view_from_export(
            _export([_entry(entry_id="i" * 64, featured=True, ai_assisted=True)])
        )
        self.assertTrue(view.editorial[0].ai_assisted)
        self.assertIn("AI-assisted", nw.render_newswire(view))

    def test_j2000_timestamp_formats_to_a_utc_string(self) -> None:
        # 837425926000000 µs after J2000 → a fixed UTC minute.
        self.assertEqual(nw._format_j2000(837_425_926_000_000), "2026-07-15 22:18 UTC")

    def test_home_headlines_link_to_their_post_pages(self) -> None:
        view = nw.newswire_view_from_export(
            _export([_entry(entry_id="p" * 64, featured=True, title="Linked lead")])
        )
        page = nw.render_newswire(view)
        self.assertIn(f'href="/post/{"p" * 64}/"', page)


@unittest.skipIf(nw is None, "newswire module missing")
class NewswirePostAndAuthorPageTest(unittest.TestCase):
    SIGNER = "e" * 64

    def _export(self) -> dict:
        return _export(
            [
                _entry(entry_id="f" * 64, featured=True, title="Featured lead",
                       signer=self.SIGNER, editorially_verified=True),
                _entry(entry_id="w" * 64, featured=False, body="wire body",
                       signer="0" * 64),
                _entry(entry_id="x" * 64, signer=self.SIGNER,
                       verification_status="signature_invalid", title="bad"),
            ],
            contributors=[
                {"author_id": self.SIGNER, "display_name": "Harbor Desk",
                 "is_organizer": True, "contribution_count": 2},
            ],
        )

    def test_post_page_shows_headline_and_a_display_name_byline_without_the_key(self) -> None:
        post = nw.post_view_from_export(self._export(), "f" * 64)
        self.assertIsNotNone(post)
        page = nw.render_post(post)
        self.assertIn("Featured lead", page)
        byline = re.search(r'class="byline".*?</div>', page, re.S).group(0)
        self.assertIn("Harbor Desk", byline)
        self.assertNotIn(self.SIGNER, byline)  # no key tag in the byline
        # The key is disclosed as provenance, not as identity.
        self.assertIn(self.SIGNER, page)

    def test_a_signature_invalid_entry_has_no_post_page(self) -> None:
        self.assertIsNone(nw.post_view_from_export(self._export(), "x" * 64))
        self.assertNotIn("x" * 64, nw.all_post_ids(self._export()))

    def test_all_post_ids_lists_the_renderable_posts_only(self) -> None:
        self.assertEqual(set(nw.all_post_ids(self._export())), {"f" * 64, "w" * 64})

    def test_author_page_is_emitted_only_for_carded_contributors(self) -> None:
        self.assertIsNone(nw.author_view_from_export(self._export(), "0" * 64))  # nameless
        author = nw.author_view_from_export(self._export(), self.SIGNER)
        self.assertIsNotNone(author)
        self.assertEqual(author.display_name, "Harbor Desk")
        self.assertTrue(author.is_organizer)

    def test_author_page_lists_only_that_authors_verified_posts(self) -> None:
        author = nw.author_view_from_export(self._export(), self.SIGNER)
        ids = [p.entry_id for p in author.posts]
        self.assertEqual(ids, ["f" * 64])  # not the "0"-signed wire post, not the invalid one
        page = nw.render_author(author)
        self.assertIn("Harbor Desk", page)
        self.assertIn("recognized organizer", page)
        self.assertIn(f'href="/post/{"f" * 64}/"', page)


if __name__ == "__main__":
    unittest.main()
