"use strict";

const form = document.getElementById("add-form");
const input = document.getElementById("new-item");
const needs = document.getElementById("needs");
const offers = document.getElementById("offers");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const DEMO_IDS = { alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" };
let me = { id: "", displayName: "member", tag: "" };
let rows = [];
const names = new Map();
const inflight = new Set();

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join(""); }
function showError(message) { error.textContent = message; error.hidden = false; }
function person(id) { return id === me.id ? "You" : names.get(id) || "A neighbor"; }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!/^[0-9a-f]{64}$/.test(id || "") || inflight.has(id)) return; inflight.add(id); riot.profile(id).then((profile) => { inflight.delete(id); const label = profile.displayName + " · " + profile.tag; if (names.get(id) !== label) { names.set(id, label); paint(); } }).catch(() => inflight.delete(id)); }); }

async function ensureSeeded() {
  if (await riot.get(SEED_MARKER)) return;
  const seeds = [
    ["items/folding-chairs", { kind: "need", text: "Six folding chairs", created_at: 1, added_by_id: DEMO_IDS.alex, resolved_by_id: "" }],
    ["items/coffee-urn", { kind: "offer", text: "Large coffee urn", created_at: 2, added_by_id: DEMO_IDS.sam, resolved_by_id: "" }],
    ["items/name-tags", { kind: "offer", text: "Blank name tags and markers", created_at: 3, added_by_id: DEMO_IDS.alex, resolved_by_id: DEMO_IDS.sam }],
  ];
  for (const [key, value] of seeds) if (!(await riot.get(key))) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1 });
}

function card(row) {
  const value = row.value || {}; const li = document.createElement("li"); li.className = "card" + (value.resolved_by_id ? " resolved" : "");
  const text = document.createElement("p"); text.className = "item-text"; text.textContent = String(value.text || "Untitled item");
  const meta = document.createElement("p"); meta.className = "meta"; meta.textContent = value.resolved_by_id ? `Sorted by ${person(value.resolved_by_id)}` : `Posted by ${person(value.added_by_id)}`;
  const button = document.createElement("button"); button.type = "button"; button.className = "resolve"; button.textContent = value.resolved_by_id ? "Reopen" : "Mark resolved"; button.setAttribute("aria-label", `${button.textContent}: ${value.text}`);
  button.addEventListener("click", () => { button.disabled = true; riot.put(row.key, { ...value, resolved_by_id: value.resolved_by_id ? "" : me.id }).catch(() => { button.disabled = false; showError("Couldn't update that item. Try again."); }); });
  li.append(text, meta, button); return li;
}
function paint() {
  error.hidden = true; rows.sort((a, b) => (a.value.resolved_by_id ? 1 : 0) - (b.value.resolved_by_id ? 1 : 0) || (a.value.created_at || 0) - (b.value.created_at || 0));
  const needRows = rows.filter((row) => row.value.kind === "need"); const offerRows = rows.filter((row) => row.value.kind === "offer");
  needs.replaceChildren(...needRows.map(card)); offers.replaceChildren(...offerRows.map(card));
  document.getElementById("needs-empty").hidden = needRows.length > 0; document.getElementById("offers-empty").hidden = offerRows.length > 0;
  document.getElementById("need-count").textContent = String(needRows.filter((row) => !row.value.resolved_by_id).length); document.getElementById("offer-count").textContent = String(offerRows.filter((row) => !row.value.resolved_by_id).length);
  resolveProfiles(rows.flatMap((row) => [row.value.added_by_id, row.value.resolved_by_id]));
}
form.addEventListener("submit", (event) => {
  event.preventDefault(); const text = input.value.trim(); if (!text || !me.id) return; const draft = input.value; const kind = document.querySelector('input[name="kind"]:checked').value; input.value = "";
  riot.put("items/" + newID(), { kind, text, created_at: Date.now(), added_by_id: me.id, resolved_by_id: "" }).then(() => { status.textContent = "Item posted"; }).catch(() => { input.value = draft; input.focus(); showError("Couldn't post that item. Your draft is safe; try again."); });
});
riot.watch("items", (next) => { rows = next; paint(); });
riot.whoami().then((who) => { me = who; return ensureSeeded(); }).catch(() => showError("The board couldn't open shared storage. Try reopening the app."));
