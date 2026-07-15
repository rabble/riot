"""Stateless renderer for the one public Riot conference export.

This module deliberately has no network client, persistence layer, signing
code, or mutation API.  It accepts one narrow, versioned public document and
renders the fixed incident-board profile from it.
"""

from __future__ import annotations

import base64
from dataclasses import dataclass, field
import hashlib
from html import escape
import json
from pathlib import Path
import re
from typing import Mapping
from urllib.parse import unquote, urlsplit


EXPORT_SCHEMA = "riot-public-gateway-export/2"
EXPORT_REVISION = "conference-gateway-export-v1"
RENDERER_PROFILE = "incident-board/1"
VERIFICATION_STATUS_VALID = "signature_verified"
VERIFICATION_STATUS_INVALID = "signature_invalid"
ALLOWED_VERIFICATION_STATUSES = frozenset(
    {VERIFICATION_STATUS_VALID, VERIFICATION_STATUS_INVALID}
)
PINNED_EXPORT_SHA256 = "d41e95ae50500ff7fe3eecfaa8dd685ae9807be16e27982e40f5735fe9b5ecd1"
PINNED_QR_SVG_SHA256 = "c53738a65751d96cdc855750211898a280d6411cc3dc21db431f6cb820b6a99e"
PUBLIC_NAMESPACE = "ae5f04268d4d2a2f86df7a43e0afe1f26577ac58dcefe95bd9b6e634c5e0155c"
INCIDENT_TITLE = "Harbor District Evacuation"
SOURCE_FIXTURE = "fixtures/conference/incident-space-v1.json"
SOURCE_FIXTURE_SHA256 = "b74f215796d958a8d9bcf554ad483afad6ab27fd194fd52202f38aebce5297de"
SOURCE_MANIFEST = "fixtures/conference/package-manifest-v1.json"
SOURCE_MANIFEST_SHA256 = "2ce667c53501cf3a25a20948f676f899399f0ba0da8558e2ed3283ebfb512060"
ALLOWED_KINDS = frozenset({"alert", "observation", "resource", "request", "offer"})
SITE_ROUTES = frozenset({"/site/", "/site/incident-board", "/site/incident-board/alerts"})

STYLE_CSS = """
:root {
  --paper: #ede7d3;
  --panel: #e3dcc4;
  --ink: #1b2430;
  --ink-muted: #55606e;
  --line: rgba(27, 36, 48, 0.16);
  --hazard: #b23a22;
  --depth: #2e5c7a;
  --anchor: #3d6b4f;
  --flag: #8a5c14;
  color-scheme: light dark;
}
@media (prefers-color-scheme: dark) {
  :root {
    --paper: #14181d;
    --panel: #1c2128;
    --ink: #ece6d3;
    --ink-muted: #9aa6b2;
    --line: rgba(236, 230, 211, 0.16);
    --anchor: #6fae82;
    --flag: #d9a441;
  }
}
* { box-sizing: border-box; }
body {
  margin: 0;
  background: var(--paper);
  color: var(--ink);
  font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, sans-serif;
  line-height: 1.45;
}
.board { max-width: 40rem; margin: 0 auto; padding: 2rem 1.25rem 4rem; }
.eyebrow, .entries-label {
  margin: 0 0 0.75rem;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.72rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--ink-muted);
}
.entries-label { border-top: 1px solid var(--line); padding-top: 1.5rem; margin-top: 0.5rem; }
.fixture-status { margin: 0 0 1.5rem; font-size: 0.85rem; color: var(--ink-muted); }
.fixture-status__tag {
  display: inline-block;
  padding: 0.05rem 0.4rem;
  border: 1px solid var(--flag);
  border-radius: 2px;
  color: var(--flag);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.78rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.headline {
  margin: 0 0 0.5rem;
  font-family: Georgia, "Iowan Old Style", "Times New Roman", serif;
  font-weight: 700;
  font-size: clamp(1.9rem, 5vw, 2.6rem);
  line-height: 1.1;
}
.subhead { margin: 0 0 2rem; color: var(--ink-muted); font-size: 1rem; }
.ticket {
  display: flex;
  align-items: center;
  gap: 1.25rem;
  margin: 0 0 2rem;
  padding: 1.1rem 1.25rem;
  background: var(--panel);
  border: 1px solid var(--line);
  border-left: 3px dashed var(--ink-muted);
  border-radius: 3px;
}
.ticket__main { flex: 1; min-width: 0; }
.ticket__action { margin: 0 0 0.5rem; }
.ticket__link {
  display: inline-block;
  font-weight: 600;
  font-size: 0.95rem;
  color: var(--ink);
  text-decoration: none;
  border-bottom: 2px solid var(--hazard);
  padding-bottom: 0.1rem;
}
.ticket__link:hover, .ticket__link:focus-visible { color: var(--hazard); }
.ticket__namespace {
  margin: 0;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.75rem;
  color: var(--ink-muted);
  word-break: break-all;
}
.ticket__namespace code { font: inherit; color: inherit; }
.ticket__qr { flex: none; line-height: 0; }
.ticket__qr svg { width: 100px; height: 100px; display: block; }
.filter:empty { display: none; }
.filter { margin: 0 0 1.5rem; }
.filter__input {
  width: 100%;
  padding: 0.5rem 0.65rem;
  font: inherit;
  color: var(--ink);
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 3px;
}
.filter__status { margin: 0.4rem 0 0; font-size: 0.8rem; color: var(--ink-muted); }
.entries { display: flex; flex-direction: column; gap: 1.5rem; }
.entry { margin: 0; padding: 0.1rem 0 0.1rem 1rem; border-left: 4px solid var(--ink-muted); }
.entry--alert { border-left-color: var(--hazard); }
.entry--resource { border-left-color: var(--depth); }
.entry--request { border-left-color: var(--flag); }
.entry--offer { border-left-color: var(--anchor); }
.kind {
  display: inline-block;
  margin: 0 0 0.5rem;
  padding: 0.1rem 0.5rem;
  font-weight: 700;
  font-size: 0.7rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  border-radius: 2px;
  border: 1px solid currentColor;
}
.kind--alert { background: var(--hazard); border-color: var(--hazard); color: #fff; }
.kind--resource { background: var(--depth); border-color: var(--depth); color: #fff; }
.kind--request { color: var(--flag); }
.kind--offer { color: var(--anchor); }
.kind--observation { color: var(--ink-muted); }
.verify {
  display: inline-block;
  margin: 0 0 0.5rem 0.4rem;
  padding: 0.1rem 0.5rem;
  font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, sans-serif;
  font-weight: 700;
  font-size: 0.7rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  border-radius: 2px;
  border: 1px solid currentColor;
}
.verify--valid { color: var(--anchor); }
.verify--invalid { background: var(--hazard); border-color: var(--hazard); color: #fff; }
.entry__title {
  margin: 0 0 0.35rem;
  font-family: Georgia, "Iowan Old Style", "Times New Roman", serif;
  font-weight: 700;
  font-size: 1.25rem;
  line-height: 1.25;
}
.entry__body { margin: 0 0 0.6rem; font-size: 1rem; }
.entry__meta {
  margin: 0;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.75rem;
  color: var(--ink-muted);
}
.entry__meta span + span::before { content: " \\00b7 "; }
a { color: inherit; }
:focus-visible { outline: 2px solid var(--hazard); outline-offset: 2px; }
@media (prefers-reduced-motion: reduce) {
  * { transition: none !important; animation: none !important; }
}
""".strip()

# Vendored, self-contained client filter — NO external lib, NO network. Ships
# inline inside every page so a mirror is a complete folder (no CDN choke point,
# no reader-IP leak). connect-src 'none' fences it: it can filter, never fetch.
# Progressive enhancement: builds its own UI into #filter, so no-JS readers see
# no dead controls and every entry stays visible.
SEARCH_JS = """
(function () {
  var entries = Array.prototype.slice.call(document.querySelectorAll('.entry'));
  var mount = document.getElementById('filter');
  if (!entries.length || !mount) { return; }
  var input = document.createElement('input');
  input.type = 'search';
  input.className = 'filter__input';
  input.placeholder = 'Filter entries\\u2026';
  input.setAttribute('aria-label', 'Filter entries');
  var status = document.createElement('p');
  status.className = 'filter__status';
  status.setAttribute('aria-live', 'polite');
  mount.appendChild(input);
  mount.appendChild(status);
  input.addEventListener('input', function () {
    var q = input.value.trim().toLowerCase();
    var shown = 0;
    entries.forEach(function (el) {
      var hit = !q || el.textContent.toLowerCase().indexOf(q) !== -1;
      el.hidden = !hit;
      if (hit) { shown++; }
    });
    status.textContent = q ? shown + ' of ' + entries.length + ' shown' : '';
  });
})();
""".strip()

_STYLE_CSS_HASH = base64.b64encode(hashlib.sha256(STYLE_CSS.encode("utf-8")).digest()).decode("ascii")
_SEARCH_JS_HASH = base64.b64encode(hashlib.sha256(SEARCH_JS.encode("utf-8")).digest()).decode("ascii")
CONTENT_SECURITY_POLICY = (
    "default-src 'none'; "
    f"style-src 'sha256-{_STYLE_CSS_HASH}'; "
    f"script-src 'sha256-{_SEARCH_JS_HASH}'; "
    "connect-src 'none'; base-uri 'none'; form-action 'none'"
)

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
    {"kind", "entry_id", "signer", "title", "body", "freshness", "ai_assisted", "verification_status"}
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
    verification_status: str


@dataclass(frozen=True, init=False)
class PublicGateway:
    namespace: str
    title: str
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
            _load_qr_svg(),
            entries,
        )


def _validate_document(document: object) -> tuple[PublicEntry, ...]:
    _reject_forbidden_content(document)
    if not isinstance(document, Mapping):
        raise GatewayError("public export must be an object")
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
    verification_status = value.get("verification_status")
    if verification_status not in ALLOWED_VERIFICATION_STATUSES:
        raise GatewayError("entry verification status is not permitted")
    return PublicEntry(kind, entry_id, signer, title, body, freshness, ai_assisted, verification_status)


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
    qr_svg: str,
    entries: tuple[PublicEntry, ...],
) -> str:
    escaped_title = escape(title)
    verified_count = sum(1 for entry in entries if entry.verification_status == VERIFICATION_STATUS_VALID)
    cards = "".join(_render_entry(entry) for entry in entries)
    namespace_uri = f"riot://open?namespace={namespace}"
    return f"""<!doctype html>
<html lang=\"en\">
<head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{CONTENT_SECURITY_POLICY}\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{escaped_title} · Riot</title><style>{STYLE_CSS}</style></head>
<body>
<main class=\"board\">
  <p class=\"eyebrow\">Public Riot export · renderer profile: {RENDERER_PROFILE}</p>
  <p class=\"fixture-status\">Signature verification: <span class=\"fixture-status__tag\">{verified_count} of {len(entries)} entries signature-verified</span></p>
  <h1 class=\"headline\">{escaped_title}</h1>
  <p class=\"subhead\">Available offline from this local public export.</p>
  <div class=\"ticket\">
    <div class=\"ticket__main\">
      <p class=\"ticket__action\"><a class=\"ticket__link\" href=\"{namespace_uri}\">Open in Riot</a></p>
      <p class=\"ticket__namespace\">Public namespace: <code>{namespace}</code></p>
    </div>
    <div class=\"ticket__qr\">{qr_svg}</div>
  </div>
  <h2 class=\"entries-label\">Incident entries</h2>
  <div class=\"filter\" id=\"filter\"></div>
  <section aria-label=\"Incident entries\" class=\"entries\">{cards}</section>
</main>
<script>{SEARCH_JS}</script>
</body>
</html>"""


def _render_entry(entry: PublicEntry) -> str:
    assisted = "AI-assisted draft" if entry.ai_assisted else "Human-authored draft"
    if entry.verification_status == VERIFICATION_STATUS_VALID:
        verify_badge = '<span class="verify verify--valid">Signature verified</span>'
    else:
        verify_badge = '<span class="verify verify--invalid">Signature invalid</span>'
    return f"""
<article class=\"entry entry--{entry.kind}\">
  <span class=\"kind kind--{entry.kind}\">{escape(entry.kind.title())}</span>{verify_badge}
  <h2 class=\"entry__title\">{escape(entry.title)}</h2>
  <p class=\"entry__body\">{escape(entry.body)}</p>
  <p class=\"entry__meta\"><span>Claimed author: <code>{entry.signer}</code></span><span>Freshness: <time datetime=\"{entry.freshness}\">{entry.freshness}</time></span><span>{assisted}</span></p>
</article>"""


def _load_qr_svg() -> str:
    try:
        raw_svg = QR_SVG_PATH.read_bytes()
    except OSError as error:
        raise GatewayError("local QR fixture could not be read") from error
    if hashlib.sha256(raw_svg).hexdigest() != PINNED_QR_SVG_SHA256:
        raise GatewayError("local QR SVG SHA-256 does not match the pinned fixture")
    return raw_svg.decode("utf-8").strip()
