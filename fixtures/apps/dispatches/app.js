"use strict";

const writeButton = document.getElementById("write");
const indexView = document.getElementById("index-view");
const list = document.getElementById("posts");
const empty = document.getElementById("empty");
const detailView = document.getElementById("detail-view");
const form = document.getElementById("dispatch-form");
const titleInput = document.getElementById("title");
const bodyInput = document.getElementById("body");
const publishButton = document.getElementById("publish");
const cancelButton = document.getElementById("cancel");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const ID_PATTERN = /^[0-9a-f]{64}$/;
const POST_KEY = /^posts\/([0-9]{1,16})-([a-z0-9-]{1,80})$/;
const DEMO_IDS = {
  alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  jo: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
};
let me = null;
let rows = [];
let ready = false;
let publishing = false;
let view = "index";
let selectedKey = "";
const names = new Map();
const inflightProfiles = new Set();
const profileRevisions = new Map();
let sharedDataRevision = 0;

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (byte) => byte.toString(16).padStart(2, "0")).join(""); }
function validIdentity(value) { return Boolean(value && ID_PATTERN.test(value.id || "")); }
function validPost(row) { const match = row && typeof row.key === "string" ? row.key.match(POST_KEY) : null; const value = row && row.value; return Boolean(match && value && typeof value === "object" && typeof value.title === "string" && value.title.trim() && value.title.length <= 120 && typeof value.body === "string" && value.body.trim() && value.body.length <= 4000 && typeof value.summary === "string" && value.summary.length <= 180 && Number.isFinite(value.created_at) && value.created_at >= 0 && ID_PATTERN.test(value.author_id || "")); }
function summaryFor(body) { const clean = body.trim().replace(/\s+/g, " "); return clean.length <= 180 ? clean : clean.slice(0, 179).trimEnd() + "…"; }
function showError(message) { error.textContent = message; error.hidden = false; status.textContent = message; }
function clearError() { error.textContent = ""; error.hidden = true; if (ready) { const count = rows.filter(validPost).length; status.textContent = count ? `${count} dispatches` : "No dispatches yet"; } }
function person(id) { if (me && id === me.id) return "You"; const profile = names.get(id); return profile ? `${profile.displayName} · ${profile.tag}` : "A neighbor"; }
function displayWhen(timestamp, detailed = false) { if (timestamp > 0 && timestamp < 10000) return "Shared here"; const date = new Date(timestamp); return detailed ? date.toLocaleString() : date.toLocaleDateString(); }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!ID_PATTERN.test(id || "") || inflightProfiles.has(id) || profileRevisions.get(id) === sharedDataRevision) return; const revision = sharedDataRevision; inflightProfiles.add(id); riot.profile(id).then((profile) => { inflightProfiles.delete(id); if (profile && typeof profile.displayName === "string" && typeof profile.tag === "string") { names.set(id, profile); profileRevisions.set(id, revision); } paint(); }).catch(() => inflightProfiles.delete(id)); }); }

async function ensureSeeded() {
  const existing = await riot.list("posts"); const marker = await riot.get(SEED_MARKER);
  if (existing.length && (!marker || marker.status !== "seeding")) { if (!marker || marker.status !== "ready") await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; }
  if (marker && marker.status !== "seeding") return;
  if (!marker) { await riot.put(SEED_MARKER, { version: 1, status: "seeding" }); if ((await riot.list("posts")).length) { await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; } }
  const seeds = [
    ["posts/1-garden-gate", { title: "The garden gate is open again", body: "The east entrance has been repaired and unlocked. Please close it gently after dark so the latch catches.", summary: "The east entrance has been repaired and unlocked.", created_at: 1, author_id: DEMO_IDS.alex }],
    ["posts/2-water-table", { title: "Water table moved to the library steps", body: "The refill table is now under the library awning. There are clean bottles, cups, and a small supply of electrolyte packets.", summary: "The refill table is now under the library awning.", created_at: 2, author_id: DEMO_IDS.sam }],
    ["posts/3-evening-walk", { title: "Notes from the evening route", body: "The south footpath is clear, but the corner by Linden Street is still poorly lit. Walk in pairs after sunset if you can.", summary: "The south footpath is clear; Linden Street is still poorly lit.", created_at: 3, author_id: DEMO_IDS.jo }],
  ];
  for (const [key, value] of seeds) if ((await riot.get(key)) === null) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1, status: "ready" });
}

function openIndex(returnFocus) { view = "index"; selectedKey = ""; paint(); if (returnFocus) writeButton.focus(); }
function openDetail(key, focus) { selectedKey = key; view = "detail"; paint(); if (focus) detailView.focus(); }
function openForm() { if (!ready) { showError("Wait for your identity before writing."); return; } view = "form"; paint(); titleInput.focus(); }
function closeForm() { if (publishing) return; clearError(); form.reset(); openIndex(true); }

function paint() {
  const valid = rows.filter(validPost).sort((left, right) => right.value.created_at - left.value.created_at || right.key.localeCompare(left.key));
  const selected = valid.find((row) => row.key === selectedKey);
  if (view === "detail" && !selected) { view = "index"; selectedKey = ""; requestAnimationFrame(() => (writeButton.disabled ? indexView : writeButton).focus()); }
  writeButton.disabled = !ready || publishing; titleInput.disabled = publishing; bodyInput.disabled = publishing; cancelButton.disabled = publishing; publishButton.disabled = !ready || publishing || !titleInput.value.trim() || !bodyInput.value.trim();
  indexView.hidden = view !== "index"; detailView.hidden = view !== "detail"; form.hidden = view !== "form";
  empty.hidden = valid.length > 0;
  if (ready) status.textContent = valid.length ? `${valid.length} dispatches` : "No dispatches yet";
  list.replaceChildren(...(view === "index" ? valid.map((row) => {
    const value = row.value; const item = document.createElement("li"); item.className = "post"; const button = document.createElement("button"); button.type = "button";
    const heading = document.createElement("h2"); heading.textContent = value.title; const summary = document.createElement("p"); summary.className = "summary"; summary.textContent = value.summary; const meta = document.createElement("p"); meta.className = "meta"; meta.textContent = `${person(value.author_id)} · ${displayWhen(value.created_at)}`;
    button.append(heading, summary, meta); button.addEventListener("click", () => openDetail(row.key, true)); item.append(button); return item;
  }) : []));
  if (selected) { document.getElementById("detail-title").textContent = selected.value.title; document.getElementById("detail-body").textContent = selected.value.body; document.getElementById("detail-meta").textContent = `${person(selected.value.author_id)} · ${displayWhen(selected.value.created_at, true)}`; }
  resolveProfiles(valid.map((row) => row.value.author_id));
}

writeButton.addEventListener("click", openForm);
document.getElementById("back").addEventListener("click", () => openIndex(true));
cancelButton.addEventListener("click", closeForm);
form.addEventListener("input", paint);
form.addEventListener("submit", async (event) => {
  event.preventDefault(); const title = titleInput.value.trim(); const body = bodyInput.value.trim(); if (!ready || !title || !body) { if (!ready) showError("Wait for your identity before publishing."); return; }
  const drafts = { title: titleInput.value, body: bodyInput.value }; const createdAt = Date.now(); const key = `posts/${createdAt}-${newID()}`; const post = { title, body, summary: summaryFor(body), created_at: createdAt, author_id: me.id }; let failed = false; clearError(); publishing = true; paint();
  try { await riot.put(key, post); clearError(); if (!rows.some((row) => row.key === key)) rows = [...rows, { key, value: post }]; form.reset(); selectedKey = key; view = "detail"; paint(); detailView.focus(); }
  catch { failed = true; titleInput.value = drafts.title; bodyInput.value = drafts.body; showError("Couldn't publish that dispatch. Your drafts are safe; try again."); }
  finally { publishing = false; paint(); if (failed) bodyInput.focus(); }
});
async function init() {
  riot.watch("posts", (next) => { rows = Array.isArray(next) ? next : []; sharedDataRevision += 1; paint(); });
  try { const identity = await riot.whoami(); if (!validIdentity(identity)) throw new Error("invalid identity"); me = identity; await ensureSeeded(); ready = true; paint(); }
  catch { ready = false; paint(); showError("Your identity couldn't be verified. Dispatches remain read-only."); }
}
init();
