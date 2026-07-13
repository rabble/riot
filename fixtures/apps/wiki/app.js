"use strict";

const indexView = document.getElementById("page-index");
const pageList = document.getElementById("pages");
const empty = document.getElementById("empty");
const detail = document.getElementById("detail");
const reader = document.getElementById("reader");
const editor = document.getElementById("editor");
const editButton = document.getElementById("edit");
const cancelButton = document.getElementById("cancel");
const saveButton = document.getElementById("save");
const pageText = document.getElementById("page-text");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const ID_PATTERN = /^[0-9a-f]{64}$/;
const PAGE_KEY = /^pages\/([a-z0-9]+(?:-[a-z0-9]+)*)$/;
const DEMO_IDS = {
  alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  jo: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
};
let me = null;
let rows = [];
let ready = false;
let saving = false;
let editing = false;
let selectedKey = "";
const names = new Map();
const inflightProfiles = new Set();
const profileRevisions = new Map();
let sharedDataRevision = 0;

function validIdentity(value) { return Boolean(value && ID_PATTERN.test(value.id || "")); }
function validPage(row) { const match = row && typeof row.key === "string" ? row.key.match(PAGE_KEY) : null; const value = row && row.value; return Boolean(match && match[1].length <= 120 && value && typeof value === "object" && typeof value.title === "string" && value.title.trim() && value.title.length <= 120 && typeof value.body === "string" && value.body.length <= 10000 && Number.isFinite(value.updated_at) && value.updated_at >= 0 && ID_PATTERN.test(value.updated_by_id || "")); }
function person(id) { if (me && id === me.id) return "You"; const profile = names.get(id); return profile ? `${profile.displayName} · ${profile.tag}` : "A neighbor"; }
function showError(message) { error.textContent = message; error.hidden = false; status.textContent = message; }
function clearError() { if (!ready) return; error.textContent = ""; error.hidden = true; const count = rows.filter(validPage).length; status.textContent = count ? `${count} pages` : "No pages yet"; }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!ID_PATTERN.test(id || "") || inflightProfiles.has(id) || profileRevisions.get(id) === sharedDataRevision) return; const revision = sharedDataRevision; inflightProfiles.add(id); riot.profile(id).then((profile) => { inflightProfiles.delete(id); if (profile && typeof profile.displayName === "string" && typeof profile.tag === "string") { names.set(id, profile); profileRevisions.set(id, revision); } paint(); }).catch(() => inflightProfiles.delete(id)); }); }

async function ensureSeeded() {
  const existing = await riot.list("pages"); const marker = await riot.get(SEED_MARKER);
  if (existing.length && (!marker || marker.status !== "seeding")) { if (!marker || marker.status !== "ready") await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; }
  if (marker && marker.status !== "seeding") return;
  if (!marker) { await riot.put(SEED_MARKER, { version: 1, status: "seeding" }); if ((await riot.list("pages")).length) { await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; } }
  const seeds = [
    ["pages/meeting-guide", { title: "Meeting guide", body: "We meet by the library steps. Bring a chair if you need one, leave room for the ramp, and start with names and one useful update from each person.", updated_at: 1, updated_by_id: DEMO_IDS.alex }],
    ["pages/tool-library", { title: "Tool library", body: "Borrowing hours are Tuesday and Saturday afternoons. Write your name on the clipboard and return clean tools to the marked shelf.", updated_at: 2, updated_by_id: DEMO_IDS.sam }],
    ["pages/shared-kitchen", { title: "Shared kitchen", body: "Label ingredients with the date. The left cupboard is for shared staples; the blue shelf is reserved for allergy-safe supplies.", updated_at: 3, updated_by_id: DEMO_IDS.jo }],
    ["pages/welcome-new-neighbors", { title: "Welcome, new neighbors", body: "Start here for local contacts, regular gatherings, and the small ways we look after this place together.", updated_at: 4, updated_by_id: DEMO_IDS.alex }],
  ];
  for (const [key, value] of seeds) if ((await riot.get(key)) === null) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1, status: "ready" });
}

function openPage(key, focus) { selectedKey = key; editing = false; clearError(); paint(); if (focus) detail.focus(); }
function openIndex() { selectedKey = ""; editing = false; clearError(); paint(); indexView.focus(); }
function beginEdit() { if (!ready || !selectedKey) { showError("Wait for your identity before editing."); return; } const selected = rows.filter(validPage).find((row) => row.key === selectedKey); if (!selected) return; editing = true; pageText.value = selected.value.body; clearError(); paint(); pageText.focus(); }
function cancelEdit() { if (saving) return; editing = false; clearError(); paint(); editButton.focus(); }

function paint() {
  const valid = rows.filter(validPage).sort((left, right) => left.value.title.localeCompare(right.value.title));
  const onPhone = window.matchMedia("(max-width: 640px)").matches;
  if (!selectedKey && valid.length && !onPhone) selectedKey = valid[0].key;
  let selected = valid.find((row) => row.key === selectedKey);
  if (selectedKey && !selected) { selectedKey = ""; editing = false; requestAnimationFrame(() => indexView.focus()); }
  selected = valid.find((row) => row.key === selectedKey);
  indexView.hidden = onPhone && Boolean(selected);
  detail.hidden = !selected;
  empty.hidden = valid.length > 0;
  pageList.replaceChildren(...valid.map((row) => { const item = document.createElement("li"); const link = document.createElement("a"); link.className = "page-link"; link.href = `#${row.key.slice(6)}`; link.textContent = row.value.title; if (row.key === selectedKey) link.setAttribute("aria-current", "page"); link.addEventListener("click", (event) => { event.preventDefault(); openPage(row.key, true); }); item.append(link); return item; }));
  editButton.disabled = !ready || saving;
  pageText.disabled = saving;
  cancelButton.disabled = saving;
  saveButton.disabled = !ready || saving || !pageText.value.trim();
  reader.hidden = editing;
  editor.hidden = !editing;
  if (selected) {
    document.getElementById("detail-title").textContent = selected.value.title;
    document.getElementById("detail-body").textContent = selected.value.body;
    document.getElementById("detail-meta").textContent = `Updated by ${person(selected.value.updated_by_id)} · ${new Date(selected.value.updated_at).toLocaleString()}`;
    document.getElementById("editor-title").textContent = `Edit ${selected.value.title}`;
  }
  if (ready) status.textContent = valid.length ? `${valid.length} pages` : "No pages yet";
  resolveProfiles(valid.map((row) => row.value.updated_by_id));
}

document.getElementById("back").addEventListener("click", openIndex);
editButton.addEventListener("click", beginEdit);
cancelButton.addEventListener("click", cancelEdit);
editor.addEventListener("input", paint);
window.addEventListener("resize", paint);
editor.addEventListener("submit", async (event) => {
  event.preventDefault();
  const key = selectedKey; const desiredBody = pageText.value.trim(); const draft = pageText.value;
  if (!ready || !key || !desiredBody) { if (!ready) showError("Wait for your identity before saving."); return; }
  let failed = false; saving = true; clearError(); paint();
  try {
    const latest = await riot.get(key);
    if (!validPage({ key, value: latest })) throw new Error("page unavailable");
    const updated = { ...latest, body: desiredBody, updated_at: Date.now(), updated_by_id: me.id };
    await riot.put(key, updated);
    if (!rows.some((row) => row.key === key && row.value.updated_at === updated.updated_at)) rows = rows.map((row) => row.key === key ? { key, value: updated } : row);
    editing = false; clearError(); paint(); editButton.focus();
  } catch { failed = true; pageText.value = draft; showError("Couldn't save that page. Your draft is safe; try again."); }
  finally { saving = false; paint(); if (failed) pageText.focus(); }
});

async function init() {
  riot.watch("pages", (next) => { rows = Array.isArray(next) ? next : []; sharedDataRevision += 1; paint(); });
  try { const identity = await riot.whoami(); if (!validIdentity(identity)) throw new Error("invalid identity"); me = identity; await ensureSeeded(); ready = true; paint(); }
  catch { ready = false; paint(); showError("Your identity couldn't be verified. Wiki pages remain read-only."); }
}
init();
