"""Stateless renderer for the one public Riot conference export.

This module deliberately has no network client, persistence layer, signing
code, or mutation API.  It accepts one narrow, versioned public document and
renders the fixed incident-board profile from it.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
from html import escape
import json
from pathlib import Path
import re
from typing import Mapping
from urllib.parse import unquote, urlsplit


EXPORT_SCHEMA = "riot-public-gateway-export/1"
EXPORT_REVISION = "conference-gateway-export-v1"
RENDERER_PROFILE = "incident-board/1"
VERIFICATION_STATUS = "fixture_unverified"
PINNED_EXPORT_SHA256 = "22dce3552b1cea9162a50c448b951fd4d1ac10bb7e0ba3c8deec0284c5c58172"
PINNED_QR_SVG_SHA256 = "e4f1489d8023f5913645b1c8119047b4197ee41ddec1ad07749ff2893fb71e0e"
PUBLIC_NAMESPACE = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
INCIDENT_TITLE = "Harbor District Evacuation"
SOURCE_FIXTURE = "fixtures/conference/incident-space-v1.json"
SOURCE_FIXTURE_SHA256 = "b91350ff9b7cf05acbb895de5e7bf4b9aba26a63acaed075ec29f5b72bccbd64"
SOURCE_MANIFEST = "fixtures/conference/package-manifest-v1.json"
SOURCE_MANIFEST_SHA256 = "2863f1676ced91a2c90eb003662a532b0eefa38a0aabe747c3084b3e873d1fab"
ALLOWED_KINDS = frozenset({"alert", "observation", "resource", "request", "offer"})
SITE_ROUTES = frozenset({"/site/", "/site/incident-board", "/site/incident-board/alerts"})

REPO_ROOT = Path(__file__).resolve().parents[2]
GATEWAY_FIXTURE_DIR = REPO_ROOT / "fixtures" / "conference" / "gateway-space"
DEFAULT_EXPORT_PATH = GATEWAY_FIXTURE_DIR / "public-export-v1.json"
QR_SVG_PATH = GATEWAY_FIXTURE_DIR / "open-in-riot-v1.svg"

_ID = re.compile(r"^[0-9a-f]{64}$")
_REMOTE_URL = re.compile(
    r"(?:https?|wss?|ftp|file|data|javascript|ipfs|magnet):", re.IGNORECASE
)
_FORBIDDEN_FIELD_PARTS = (
    "private",
    "group",
    "capability",
    "receipt",
    "secret",
    "password",
    "token",
    "nsec",
    "encrypted",
    "encryption",
    "javascript",
    "script",
    "executable",
    "wasm",
    "module",
    "command",
    "handler",
    "onload",
    "onclick",
    "remote",
    "url",
    "uri",
    "href",
    "src",
)
_TOP_LEVEL_FIELDS = frozenset(
    {
        "schema",
        "export_revision",
        "renderer_profile",
        "verification_status",
        "source_fixture",
        "source_fixture_sha256",
        "source_manifest",
        "source_manifest_sha256",
        "namespace",
        "visibility",
        "title",
        "generated_at",
        "entries",
    }
)
_ENTRY_FIELDS = frozenset(
    {"kind", "entry_id", "signer", "title", "body", "freshness", "ai_assisted"}
)
_RENDER_AUTHORITY = object()


class GatewayError(ValueError):
    """The supplied value is outside the public gateway's fixed boundary."""


@dataclass(frozen=True)
class PublicEntry:
    kind: str
    entry_id: str
    signer: str
    title: str
    body: str
    freshness: str
    ai_assisted: bool


@dataclass(frozen=True, init=False)
class PublicGateway:
    namespace: str
    title: str
    verification_status: str
    entries: tuple[PublicEntry, ...]
    _verified_export_sha256: str = field(repr=False)
    _render_authority: object = field(repr=False)

    def __new__(cls) -> "PublicGateway":
        raise TypeError("PublicGateway must be constructed with from_file()")

    @classmethod
    def from_file(cls, export_path: Path) -> "PublicGateway":
        """Load a checked-in local export; URLs and other sources are not inputs."""
        if not isinstance(export_path, Path):
            raise GatewayError("only a local export path is permitted")
        try:
            raw_export = export_path.read_bytes()
            export_sha256 = hashlib.sha256(raw_export).hexdigest()
            if export_sha256 != PINNED_EXPORT_SHA256:
                raise GatewayError("public export SHA-256 does not match the pinned fixture")
            document = json.loads(raw_export)
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
            raise GatewayError("local public export could not be read") from error
        entries = _validate_document(document)
        gateway = object.__new__(cls)
        object.__setattr__(gateway, "namespace", PUBLIC_NAMESPACE)
        object.__setattr__(gateway, "title", INCIDENT_TITLE)
        object.__setattr__(gateway, "verification_status", VERIFICATION_STATUS)
        object.__setattr__(gateway, "entries", entries)
        object.__setattr__(gateway, "_verified_export_sha256", export_sha256)
        object.__setattr__(gateway, "_render_authority", _RENDER_AUTHORITY)
        return gateway

    @staticmethod
    def validate_document(document: object) -> None:
        """Validate boundary rejection without creating a renderable gateway."""
        _validate_document(document)

    def render(self, route: str) -> str:
        if (
            self._render_authority is not _RENDER_AUTHORITY
            or self._verified_export_sha256 != PINNED_EXPORT_SHA256
        ):
            raise GatewayError("public export SHA-256 must be verified before rendering")
        parsed = urlsplit(route)
        if (
            parsed.scheme
            or parsed.netloc
            or parsed.query
            or parsed.fragment
            or parsed.path not in SITE_ROUTES
        ):
            raise GatewayError("unknown public route")
        entries = self.entries
        if parsed.path == "/site/incident-board/alerts":
            entries = tuple(entry for entry in entries if entry.kind == "alert")
        return _render_page(
            self.title,
            self.namespace,
            self.verification_status,
            _load_qr_svg(),
            entries,
        )


def _validate_document(document: object) -> tuple[PublicEntry, ...]:
    _reject_forbidden_content(document)
    if not isinstance(document, Mapping):
        raise GatewayError("public export must be an object")
    if document.get("verification_status") != VERIFICATION_STATUS:
        raise GatewayError("fixture verification status is not permitted")
    if set(document) != _TOP_LEVEL_FIELDS:
        raise GatewayError("public export fields are not permitted")
    if document.get("schema") != EXPORT_SCHEMA or document.get("export_revision") != EXPORT_REVISION:
        raise GatewayError("public export version is not permitted")
    if document.get("renderer_profile") != RENDERER_PROFILE:
        raise GatewayError("renderer profile is not permitted")
    if (
        document.get("source_fixture") != SOURCE_FIXTURE
        or document.get("source_fixture_sha256") != SOURCE_FIXTURE_SHA256
        or document.get("source_manifest") != SOURCE_MANIFEST
        or document.get("source_manifest_sha256") != SOURCE_MANIFEST_SHA256
    ):
        raise GatewayError("conference fixture boundary is not permitted")
    if document.get("visibility") != "public":
        raise GatewayError("only public exports are permitted")
    if document.get("namespace") != PUBLIC_NAMESPACE or document.get("title") != INCIDENT_TITLE:
        raise GatewayError("only the fixed conference export is permitted")
    _require_timestamp(document.get("generated_at"), "generated_at")

    raw_entries = document.get("entries")
    if not isinstance(raw_entries, list) or not raw_entries:
        raise GatewayError("public export must contain public entries")
    return tuple(_parse_entry(entry) for entry in raw_entries)


def _reject_forbidden_content(value: object) -> None:
    if isinstance(value, Mapping):
        for key, nested in value.items():
            if not isinstance(key, str):
                raise GatewayError("public export fields are not permitted")
            normalized = key.lower().replace("-", "_")
            if any(part in normalized for part in _FORBIDDEN_FIELD_PARTS):
                raise GatewayError(f"field {key!r} is not permitted")
            _reject_forbidden_content(nested)
    elif isinstance(value, list):
        for nested in value:
            _reject_forbidden_content(nested)
    elif isinstance(value, str):
        normalized = value
        for _ in range(3):
            decoded = unquote(normalized)
            if decoded == normalized:
                break
            normalized = decoded
        slash_normalized = normalized.replace("\\", "/")
        if _REMOTE_URL.search(normalized) or "//" in slash_normalized:
            raise GatewayError("remote URLs or references are not permitted")


def _parse_entry(value: object) -> PublicEntry:
    if not isinstance(value, Mapping) or set(value) != _ENTRY_FIELDS:
        raise GatewayError("entry fields are not permitted")
    kind = value.get("kind")
    if kind not in ALLOWED_KINDS:
        raise GatewayError("entry kind is not permitted")
    entry_id = _require_id(value.get("entry_id"), "entry_id")
    signer = _require_id(value.get("signer"), "signer")
    title = _require_text(value.get("title"), "title")
    body = _require_text(value.get("body"), "body")
    freshness = _require_timestamp(value.get("freshness"), "freshness")
    ai_assisted = value.get("ai_assisted")
    if not isinstance(ai_assisted, bool):
        raise GatewayError("ai_assisted must be a boolean")
    return PublicEntry(kind, entry_id, signer, title, body, freshness, ai_assisted)


def _require_id(value: object, field: str) -> str:
    if not isinstance(value, str) or not _ID.fullmatch(value):
        raise GatewayError(f"{field} must be a full public identifier")
    return value


def _require_text(value: object, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise GatewayError(f"{field} must be non-empty text")
    return value


def _require_timestamp(value: object, field: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value):
        raise GatewayError(f"{field} must be an RFC3339 UTC timestamp")
    return value


def _render_page(
    title: str,
    namespace: str,
    verification_status: str,
    qr_svg: str,
    entries: tuple[PublicEntry, ...],
) -> str:
    escaped_title = escape(title)
    cards = "".join(_render_entry(entry) for entry in entries)
    namespace_uri = f"riot://open?namespace={namespace}"
    return f"""<!doctype html>
<html lang=\"en\">
<head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{escaped_title} · Riot</title></head>
<body>
<main>
  <p>Public Riot export · renderer profile: {RENDERER_PROFILE}</p>
  <p>Fixture verification: {escape(verification_status.replace("_", " "))}</p>
  <h1>{escaped_title}</h1>
  <p>Available offline from this local public export.</p>
  <p>Public namespace: <code>{namespace}</code></p>
  <div class=\"qr-code\">{qr_svg}</div>
  <p><a href=\"{namespace_uri}\">Open in Riot</a></p>
  <section aria-label=\"Incident entries\">{cards}</section>
</main>
</body>
</html>"""


def _render_entry(entry: PublicEntry) -> str:
    assisted = "AI-assisted draft" if entry.ai_assisted else "Human-authored draft"
    return f"""
<article>
  <p>{escape(entry.kind.title())}</p>
  <h2>{escape(entry.title)}</h2>
  <p>{escape(entry.body)}</p>
  <p>Claimed author (unverified fixture): <code>{entry.signer}</code></p>
  <p>Freshness: <time datetime=\"{entry.freshness}\">{entry.freshness}</time></p>
  <p>{assisted}</p>
</article>"""


def _load_qr_svg() -> str:
    try:
        raw_svg = QR_SVG_PATH.read_bytes()
    except OSError as error:
        raise GatewayError("local QR fixture could not be read") from error
    if hashlib.sha256(raw_svg).hexdigest() != PINNED_QR_SVG_SHA256:
        raise GatewayError("local QR SVG SHA-256 does not match the pinned fixture")
    return raw_svg.decode("utf-8").strip()
