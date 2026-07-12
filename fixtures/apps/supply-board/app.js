"use strict";

const form = document.getElementById("add-form");
const input = document.getElementById("new-item");
const postButton = document.getElementById("post");
const needs = document.getElementById("needs");
const offers = document.getElementById("offers");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const ID_PATTERN = /^[0-9a-f]{64}$/;
const ITEM_KEY = /^items\/[a-z0-9-]{1,256}$/;
const DEMO_IDS = { alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" };
let me = null;
let rows = [];
let ready = false;
let posting = false;
const locks = new Set();
const names = new Map();
const inflightProfiles = new Set();

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join(""); }
function validIdentity(value) { return value && ID_PATTERN.test(value.id || ""); }
function validItem(row) { const value = row && row.value; return Boolean(row && ITEM_KEY.test(row.key) && value && typeof value === "object" && ["need", "offer"].includes(value.kind) && typeof value.text === "string" && value.text.trim() && value.text.length <= 180 && Number.isFinite(value.created_at) && value.created_at >= 0 && ID_PATTERN.test(value.added_by_id || "") && (value.resolved_by_id === "" || ID_PATTERN.test(value.resolved_by_id || ""))); }
function showError(message) { error.textContent = message; error.hidden = false; status.textContent = message; }
function formValid() { return input.value.trim().length > 0; }
function person(id) { return me && id === me.id ? "You" : names.get(id) || "A neighbor"; }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!ID_PATTERN.test(id || "") || inflightProfiles.has(id)) return; inflightProfiles.add(id); riot.profile(id).then((profile) => { inflightProfiles.delete(id); const label = profile.displayName + " · " + profile.tag; if (names.get(id) !== label) { names.set(id, label); paint(); } }).catch(() => inflightProfiles.delete(id)); }); }

async function ensureSeeded() {
  const existing = await riot.list("items"); const marker = await riot.get(SEED_MARKER);
  if (existing.length && (!marker || marker.status !== "seeding")) { if (!marker || marker.status !== "ready") await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; }
  if (marker && marker.status !== "seeding") return;
  if (!marker) { await riot.put(SEED_MARKER, { version: 1, status: "seeding" }); if ((await riot.list("items")).length) { await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; } }
  const seeds = [
    ["items/folding-chairs", { kind: "need", text: "Six folding chairs", created_at: 1, added_by_id: DEMO_IDS.alex, resolved_by_id: "" }],
    ["items/coffee-urn", { kind: "offer", text: "Large coffee urn", created_at: 2, added_by_id: DEMO_IDS.sam, resolved_by_id: "" }],
    ["items/name-tags", { kind: "offer", text: "Blank name tags and markers", created_at: 3, added_by_id: DEMO_IDS.alex, resolved_by_id: DEMO_IDS.sam }],
  ];
  // No bridge CAS exists. Reserved keys and fresh reads avoid overwriting data
  // already visible here; simultaneous identical seed writes remain possible.
  for (const [key, value] of seeds) if ((await riot.get(key)) === null) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1, status: "ready" });
}

async function toggleResolved(row, shouldResolve) {
  if (!ready || locks.has(row.key)) return; locks.add(row.key); paint();
  try { const latest = { key: row.key, value: await riot.get(row.key) }; if (!validItem(latest)) throw new Error("item changed shape"); if (Boolean(latest.value.resolved_by_id) === shouldResolve) return; await riot.put(row.key, { ...latest.value, resolved_by_id: shouldResolve ? me.id : "" }); }
  catch { showError("Couldn't update that item. Try again."); }
  finally { locks.delete(row.key); paint(); }
}
function card(row) {
  const value = row.value; const li = document.createElement("li"); li.className = "card" + (value.resolved_by_id ? " resolved" : "");
  const text = document.createElement("p"); text.className = "item-text"; text.textContent = value.text; const meta = document.createElement("p"); meta.className = "meta"; meta.textContent = value.resolved_by_id ? `Sorted by ${person(value.resolved_by_id)}` : `Posted by ${person(value.added_by_id)}`;
  const button = document.createElement("button"); button.type = "button"; button.className = "resolve"; button.disabled = !ready || locks.has(row.key); button.textContent = value.resolved_by_id ? "Reopen" : "Mark resolved"; button.setAttribute("aria-label", `${button.textContent}: ${value.text}`); button.addEventListener("click", () => toggleResolved(row, !Boolean(value.resolved_by_id))); li.append(text, meta, button); return li;
}
function paint() {
  const valid = rows.filter(validItem).sort((a, b) => Number(Boolean(a.value.resolved_by_id)) - Number(Boolean(b.value.resolved_by_id)) || a.value.created_at - b.value.created_at);
  postButton.disabled = !ready || posting || !formValid();
  const needRows = valid.filter((row) => row.value.kind === "need"); const offerRows = valid.filter((row) => row.value.kind === "offer"); needs.replaceChildren(...needRows.map(card)); offers.replaceChildren(...offerRows.map(card));
  document.getElementById("needs-empty").hidden = needRows.length > 0; document.getElementById("offers-empty").hidden = offerRows.length > 0; document.getElementById("need-count").textContent = String(needRows.filter((row) => !row.value.resolved_by_id).length); document.getElementById("offer-count").textContent = String(offerRows.filter((row) => !row.value.resolved_by_id).length);
  if (ready) status.textContent = valid.length ? `${valid.length} items on the board` : "The board is empty";
  resolveProfiles(valid.flatMap((row) => [row.value.added_by_id, row.value.resolved_by_id]));
}
form.addEventListener("submit", async (event) => {
  event.preventDefault(); const text = input.value.trim(); if (!ready || !text) { if (!ready) showError("Wait for your identity before posting."); return; } const draft = input.value; const kind = document.querySelector('input[name="kind"]:checked').value; input.value = ""; posting = true; paint();
  try { await riot.put("items/" + newID(), { kind, text, created_at: Date.now(), added_by_id: me.id, resolved_by_id: "" }); }
  catch { input.value = draft; input.focus(); showError("Couldn't post that item. Your draft is safe; try again."); }
  finally { posting = false; paint(); }
});
input.addEventListener("input", paint);
async function init() {
  riot.watch("items", (next) => { rows = next; paint(); });
  try { const identity = await riot.whoami(); if (!validIdentity(identity)) throw new Error("invalid identity"); me = identity; await ensureSeeded(); ready = true; paint(); }
  catch { ready = false; paint(); showError("Your identity couldn't be verified. The board remains read-only."); }
}
init();
