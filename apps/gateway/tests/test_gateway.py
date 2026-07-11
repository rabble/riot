"""Public-boundary contract for the Riot conference gateway."""

from __future__ import annotations

import hashlib
import json
from http.server import ThreadingHTTPServer
from pathlib import Path
import re
import sys
import tempfile
from threading import Thread
import unittest
from urllib.error import HTTPError
from urllib.request import Request
from urllib.request import urlopen
from xml.etree import ElementTree

ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ROOT.parents[1]
GATEWAY_FIXTURE = REPO_ROOT / "fixtures" / "conference" / "gateway-space"
EXPORT = GATEWAY_FIXTURE / "public-export-v1.json"
QR_SVG = GATEWAY_FIXTURE / "open-in-riot-v1.svg"
SOURCE_FIXTURE = REPO_ROOT / "fixtures" / "conference" / "incident-space-v1.json"
SOURCE_MANIFEST = REPO_ROOT / "fixtures" / "conference" / "package-manifest-v1.json"
QR_SVG_SHA256 = "e4f1489d8023f5913645b1c8119047b4197ee41ddec1ad07749ff2893fb71e0e"
sys.path.insert(0, str(ROOT))

try:
    import riot_gateway as gateway_module
    from riot_gateway import GatewayError, PublicGateway
    from server import make_handler
except ModuleNotFoundError:
    gateway_module = None
    GatewayError = RuntimeError
    PublicGateway = None
    make_handler = None


class PublicGatewayTest(unittest.TestCase):
    def setUp(self) -> None:
        self.assertIsNotNone(PublicGateway, "the public reader does not exist")
        self.assertTrue(EXPORT.is_file(), "the fixture-bound gateway export does not exist")
        self.gateway = PublicGateway.from_file(EXPORT)

    def test_gateway_uses_one_fixture_bound_export_with_a_pinned_hash(self) -> None:
        document = json.loads(EXPORT.read_text())
        export_hash = hashlib.sha256(EXPORT.read_bytes()).hexdigest()

        self.assertFalse((ROOT / "public-export-v1.json").exists())
        self.assertEqual(export_hash, getattr(gateway_module, "PINNED_EXPORT_SHA256", None))
        self.assertEqual(document["source_fixture"], "fixtures/conference/incident-space-v1.json")
        self.assertEqual(document["source_fixture_sha256"], hashlib.sha256(SOURCE_FIXTURE.read_bytes()).hexdigest())
        self.assertEqual(document["source_manifest"], "fixtures/conference/package-manifest-v1.json")
        self.assertEqual(document["source_manifest_sha256"], hashlib.sha256(SOURCE_MANIFEST.read_bytes()).hexdigest())

    def test_modified_export_bytes_are_rejected_before_render(self) -> None:
        mutated = EXPORT.read_bytes().replace(b"Ferry terminal", b"Mutant terminal", 1)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "public-export-v1.json"
            path.write_bytes(mutated)
            with self.assertRaisesRegex(GatewayError, "SHA-256"):
                PublicGateway.from_file(path)

    def test_public_hash_constant_cannot_make_a_mutated_document_renderable(self) -> None:
        document = json.loads(EXPORT.read_text())
        document["entries"][0]["title"] = "Forged incident title"

        self.assertIsNone(PublicGateway.validate_document(document))
        with self.assertRaisesRegex(TypeError, "from_file"):
            PublicGateway()
        with self.assertRaises((AttributeError, TypeError, GatewayError)):
            forged = PublicGateway.from_document(
                document,
                _verified_export_sha256=gateway_module.PINNED_EXPORT_SHA256,
            )
            forged.render("/site/")

    def test_renders_the_fixed_public_incident_board_at_site_routes(self) -> None:
        home = self.gateway.render("/site/")
        alerts = self.gateway.render("/site/incident-board/alerts")

        self.assertIn("Harbor District Evacuation", home)
        self.assertIn("Ferry terminal access restricted", alerts)
        self.assertIn("incident-board/1", home)

    def test_renders_unverified_fixture_provenance_freshness_ai_offline_and_open_in_riot(self) -> None:
        page = self.gateway.render("/site/incident-board")

        self.assertIn("Claimed author (unverified fixture):", page)
        self.assertIn("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", page)
        self.assertIn("Freshness:", page)
        self.assertIn("2026-07-11T09:30:00Z", page)
        self.assertIn("AI-assisted draft", page)
        self.assertIn("Available offline from this local public export", page)
        self.assertIn("Open in Riot", page)
        self.assertIn("riot://open?namespace=3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c", page)
        self.assertIn("<svg", page)
        self.assertIn('data-qr-value="riot://open?namespace=3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"', page)

    def test_every_page_embeds_the_checked_in_qr_matrix_and_full_value(self) -> None:
        svg = ElementTree.parse(QR_SVG).getroot()
        namespace = "{http://www.w3.org/2000/svg}"
        encoded_value = f"riot://open?namespace={self.gateway.namespace}"

        self.assertEqual(svg.tag, f"{namespace}svg")
        self.assertEqual(svg.attrib["data-qr-value"], encoded_value)
        self.assertGreaterEqual(int(svg.attrib["data-module-count"]), 21)
        matrix_path = svg.find(f"{namespace}path")
        self.assertIsNotNone(matrix_path)
        self.assertGreater(matrix_path.attrib["d"].count("m"), 100)
        for route in gateway_module.SITE_ROUTES:
            with self.subTest(route=route):
                page = self.gateway.render(route)
                self.assertIn("<svg", page)
                self.assertIn(f'data-qr-value="{encoded_value}"', page)

    def test_checked_svg_bytes_and_matrix_decode_to_the_exact_open_value(self) -> None:
        encoded_value = f"riot://open?namespace={self.gateway.namespace}"

        self.assertEqual(hashlib.sha256(QR_SVG.read_bytes()).hexdigest(), QR_SVG_SHA256)
        self.assertEqual(_decode_version_6_m_qr(QR_SVG), encoded_value)

    def test_rejects_private_and_unsafe_fields_before_rendering(self) -> None:
        document = json.loads(EXPORT.read_text())

        for field, value in (
            ("private_group", {"id": "private-group-id"}),
            ("capability", "capability-token"),
            ("receipt", {"private_id": "receipt-id"}),
            ("private_id", "private-record-id"),
            ("secret", "not-a-secret"),
            ("javascript", "alert('no')"),
            ("remote_url", "https://example.invalid/export.json"),
        ):
            with self.subTest(field=field):
                candidate = dict(document)
                candidate[field] = value
                with self.assertRaisesRegex(GatewayError, "not permitted"):
                    PublicGateway.validate_document(candidate)

    def test_rejects_missing_or_unknown_fixture_verification_status(self) -> None:
        document = json.loads(EXPORT.read_text())
        document["verification_status"] = "fixture_unverified"

        missing_status = dict(document)
        del missing_status["verification_status"]
        with self.assertRaisesRegex(GatewayError, "verification status"):
            PublicGateway.validate_document(missing_status)

        unknown_status = dict(document)
        unknown_status["verification_status"] = "signature_verified"
        with self.assertRaisesRegex(GatewayError, "verification status"):
            PublicGateway.validate_document(unknown_status)

    def test_rejects_arbitrary_profiles_and_external_urls_before_rendering(self) -> None:
        document = json.loads(EXPORT.read_text())

        arbitrary_profile = dict(document)
        arbitrary_profile["renderer_profile"] = "anything/99"
        with self.assertRaisesRegex(GatewayError, "renderer profile"):
            PublicGateway.validate_document(arbitrary_profile)

        remote_link = json.loads(EXPORT.read_text())
        remote_link["entries"][0]["body"] = "See https://example.invalid/for-more"
        with self.assertRaisesRegex(GatewayError, "remote URLs"):
            PublicGateway.validate_document(remote_link)

    def test_rejects_external_and_executable_schemes_including_mixed_case(self) -> None:
        document = json.loads(EXPORT.read_text())

        for scheme in ("file:", "data:", "JaVaScRiPt:", "IPFS:", "magnet:"):
            with self.subTest(scheme=scheme):
                candidate = json.loads(EXPORT.read_text())
                candidate["entries"][0]["body"] = f"{scheme}unsafe-payload"
                with self.assertRaisesRegex(GatewayError, "remote URLs"):
                    PublicGateway.validate_document(candidate)

    def test_rejects_protocol_relative_backslash_and_encoded_remote_references(self) -> None:
        for reference in (
            "//example.invalid/path",
            r"\\example.invalid\path",
            "%2f%2fexample.invalid/path",
            "%5C%5Cexample.invalid%5Cpath",
            "%252F%252Fexample.invalid/path",
        ):
            with self.subTest(reference=reference):
                candidate = json.loads(EXPORT.read_text())
                candidate["entries"][0]["body"] = reference
                with self.assertRaisesRegex(GatewayError, "remote"):
                    PublicGateway.validate_document(candidate)

    def test_rejects_non_site_routes(self) -> None:
        with self.assertRaisesRegex(GatewayError, "unknown public route"):
            self.gateway.render("/private/incident-board")

    def test_rejects_network_and_absolute_form_request_targets(self) -> None:
        for route in (
            "//attacker.example/site/",
            "https://attacker.example/site/",
            "http://attacker.example/site/incident-board",
        ):
            with self.subTest(route=route):
                with self.assertRaisesRegex(GatewayError, "unknown public route"):
                    self.gateway.render(route)


class ServerHeadersTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.assertIsNotNone(make_handler, "the HTTP reader does not exist")
        gateway = PublicGateway.from_file(EXPORT)
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(gateway))
        cls.thread = Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()

    def test_successful_responses_forbid_scripts_network_and_referrers(self) -> None:
        port = self.server.server_address[1]
        with urlopen(f"http://127.0.0.1:{port}/site/", timeout=2) as response:
            self.assertEqual(response.headers["Content-Security-Policy"], "default-src 'none'; script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'")
            self.assertEqual(response.headers["X-Content-Type-Options"], "nosniff")
            self.assertEqual(response.headers["Referrer-Policy"], "no-referrer")

    def test_error_responses_forbid_scripts_network_and_referrers(self) -> None:
        port = self.server.server_address[1]
        for request in (
            Request(f"http://127.0.0.1:{port}/private/incident-board"),
            Request(f"http://127.0.0.1:{port}/site/", method="POST", data=b"{}"),
        ):
            with self.subTest(method=request.get_method(), path=request.full_url):
                with self.assertRaises(HTTPError) as result:
                    urlopen(request, timeout=2)
                error = result.exception
                try:
                    headers = error.headers
                    self.assertEqual(headers["Content-Security-Policy"], "default-src 'none'; script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'")
                    self.assertEqual(headers["X-Content-Type-Options"], "nosniff")
                    self.assertEqual(headers["Referrer-Policy"], "no-referrer")
                finally:
                    error.close()


class TestModuleLayoutTest(unittest.TestCase):
    def test_unittest_entrypoint_follows_all_test_classes(self) -> None:
        source = Path(__file__).read_text()
        self.assertGreater(source.rfind('if __name__ == "__main__":'), source.rfind("class "))


def _decode_version_6_m_qr(svg_path: Path) -> str:
    root = ElementTree.parse(svg_path).getroot()
    namespace = "{http://www.w3.org/2000/svg}"
    size = int(root.attrib["data-module-count"])
    path = root.find(f"{namespace}path")
    matrix = _matrix_from_segno_path(path.attrib["d"], size, border=4)
    mask = _format_mask(matrix)
    function = _version_6_function_modules(size)
    bits: list[int] = []

    right = size - 1
    while right >= 1:
        if right == 6:
            right = 5
        upward = ((right + 1) & 2) == 0
        for vertical in range(size):
            y = size - 1 - vertical if upward else vertical
            for x in (right, right - 1):
                if not function[y][x]:
                    bit = matrix[y][x]
                    if _mask_bit(mask, x, y):
                        bit ^= 1
                    bits.append(bit)
        right -= 2

    interleaved = [_read_bits(bits, offset, 8) for offset in range(0, 172 * 8, 8)]
    blocks = [bytearray() for _ in range(4)]
    for index, byte in enumerate(interleaved[:108]):
        blocks[index % 4].append(byte)
    data = b"".join(blocks)
    data_bits = [(byte >> shift) & 1 for byte in data for shift in range(7, -1, -1)]
    if _read_bits(data_bits, 0, 4) != 0b0100:
        raise AssertionError("checked QR is not byte mode")
    length = _read_bits(data_bits, 4, 8)
    payload = bytes(
        _read_bits(data_bits, 12 + index * 8, 8) for index in range(length)
    )
    return payload.decode("utf-8")


def _matrix_from_segno_path(path_data: str, size: int, border: int) -> list[list[int]]:
    tokens = re.findall(r"[MmhH]|-?(?:\d+(?:\.\d*)?|\.\d+)", path_data)
    matrix = [[0] * size for _ in range(size)]
    x = 0.0
    y = 0.0
    index = 0
    while index < len(tokens):
        command = tokens[index]
        index += 1
        if command in ("M", "m"):
            next_x = float(tokens[index])
            next_y = float(tokens[index + 1])
            index += 2
            if command == "M":
                x, y = next_x, next_y
            else:
                x += next_x
                y += next_y
        elif command in ("h", "H"):
            end_x = float(tokens[index])
            index += 1
            if command == "h":
                end_x += x
            row = round(y - border - 0.5)
            start = round(min(x, end_x) - border)
            stop = round(max(x, end_x) - border)
            for column in range(start, stop):
                matrix[row][column] = 1
            x = end_x
        else:
            raise AssertionError(f"unsupported SVG path command: {command}")
    return matrix


def _format_mask(matrix: list[list[int]]) -> int:
    coordinates = (
        [(8, row) for row in range(6)]
        + [(8, 7), (8, 8)]
        + [(8, len(matrix) - 15 + bit) for bit in range(8, 15)]
    )
    format_bits = sum(matrix[y][x] << bit for bit, (x, y) in enumerate(coordinates))
    unmasked = format_bits ^ 0x5412
    data = unmasked >> 10
    remainder = data
    for _ in range(10):
        remainder = (remainder << 1) ^ ((remainder >> 9) * 0x537)
    if ((data << 10) | remainder) != unmasked:
        raise AssertionError("checked QR format BCH is invalid")
    return data & 0b111


def _version_6_function_modules(size: int) -> list[list[bool]]:
    function = [[False] * size for _ in range(size)]
    for index in range(size):
        function[6][index] = True
        function[index][6] = True
    for center_x, center_y in ((3, 3), (size - 4, 3), (3, size - 4)):
        for delta_y in range(-4, 5):
            for delta_x in range(-4, 5):
                x = center_x + delta_x
                y = center_y + delta_y
                if 0 <= x < size and 0 <= y < size:
                    function[y][x] = True
    for y in range(32, 37):
        for x in range(32, 37):
            function[y][x] = True
    format_coordinates = (
        [(column, 8) for column in range(6)]
        + [(7, 8), (8, 8), (8, 7)]
        + [(8, 14 - bit) for bit in range(9, 15)]
        + [(size - 1 - bit, 8) for bit in range(8)]
        + [(8, size - 15 + bit) for bit in range(8, 15)]
        + [(8, size - 8)]
    )
    for x, y in format_coordinates:
        function[y][x] = True
    return function


def _mask_bit(mask: int, x: int, y: int) -> bool:
    masks = (
        (x + y) % 2,
        y % 2,
        x % 3,
        (x + y) % 3,
        (x // 3 + y // 2) % 2,
        x * y % 2 + x * y % 3,
        (x * y % 2 + x * y % 3) % 2,
        ((x + y) % 2 + x * y % 3) % 2,
    )
    return masks[mask] == 0


def _read_bits(bits: list[int], offset: int, length: int) -> int:
    value = 0
    for bit in bits[offset : offset + length]:
        value = (value << 1) | bit
    return value


if __name__ == "__main__":
    unittest.main()
