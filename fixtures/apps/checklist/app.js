"use strict";

const list = document.getElementById("items");
const empty = document.getElementById("empty");
const form = document.getElementById("add-form");
const input = document.getElementById("new-task");
const addButton = document.getElementById("add");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const ID_PATTERN = /^[0-9a-f]{64}$/;
const TASK_KEY = /^tasks\/[a-z0-9-]{1,256}$/;
const DEMO_IDS = { alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" };
let me = null;
let rows = [];
let ready = false;
let adding = false;
const locks = new Set();
const names = new Map();
const inflightProfiles = new Set();

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (byte) => byte.toString(16).padStart(2, "0")).join(""); }
function validIdentity(value) { return value && ID_PATTERN.test(value.id || ""); }
function validTask(row) { const value = row && row.value; return Boolean(row && TASK_KEY.test(row.key) && value && typeof value === "object" && typeof value.text === "string" && value.text.trim() && value.text.length <= 180 && Number.isFinite(value.created_at) && value.created_at >= 0 && ID_PATTERN.test(value.added_by_id || "") && (value.assigned_to_id === "" || ID_PATTERN.test(value.assigned_to_id || "")) && typeof value.completed === "boolean"); }
function showError(message) { error.textContent = message; error.hidden = false; status.textContent = message; }
function formValid() { return input.value.trim().length > 0; }
function person(id) { return me && id === me.id ? "You" : names.get(id) || "A neighbor"; }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!ID_PATTERN.test(id || "") || inflightProfiles.has(id)) return; inflightProfiles.add(id); riot.profile(id).then((profile) => { inflightProfiles.delete(id); const label = profile.displayName + " · " + profile.tag; if (names.get(id) !== label) { names.set(id, label); paint(); } }).catch(() => inflightProfiles.delete(id)); }); }

async function ensureSeeded() {
  const existing = await riot.list("tasks");
  const marker = await riot.get(SEED_MARKER);
  if (existing.length && (!marker || marker.status !== "seeding")) { if (!marker || marker.status !== "ready") await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; }
  if (marker && marker.status !== "seeding") return;
  if (!marker) { await riot.put(SEED_MARKER, { version: 1, status: "seeding" }); if ((await riot.list("tasks")).length) { await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; } }
  const seeds = [
    ["tasks/bring-extension-cord", { text: "Bring the long extension cord", created_at: 1, added_by_id: DEMO_IDS.alex, assigned_to_id: "", completed: false }],
    ["tasks/check-garden-gate", { text: "Check the garden gate latch", created_at: 2, added_by_id: DEMO_IDS.sam, assigned_to_id: DEMO_IDS.alex, completed: false }],
    ["tasks/print-sign-in-sheet", { text: "Print the sign-in sheet", created_at: 3, added_by_id: DEMO_IDS.alex, assigned_to_id: DEMO_IDS.sam, completed: true }],
  ];
  // The bridge has no compare-and-set. Stable reserved keys plus a fresh read
  // prevent this client from overwriting synced data; simultaneous first opens
  // may still write the same seed value, which is harmless under record LWW.
  for (const [key, value] of seeds) if ((await riot.get(key)) === null) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1, status: "ready" });
}

async function mutateTask(row, change) {
  if (!ready || locks.has(row.key)) return;
  locks.add(row.key); paint();
  try {
    const latest = { key: row.key, value: await riot.get(row.key) };
    if (!validTask(latest)) throw new Error("task changed shape");
    // Local mutations serialize per record and re-read the latest LWW value.
    // Cross-device concurrent writes remain record-level last-writer-wins.
    await riot.put(row.key, { ...latest.value, ...change(latest.value) });
  } catch { showError("Couldn't update that task. Try again."); }
  finally { locks.delete(row.key); paint(); }
}

function paint() {
  const valid = rows.filter(validTask).sort((a, b) => Number(a.value.completed) - Number(b.value.completed) || a.value.created_at - b.value.created_at);
  empty.hidden = valid.length > 0;
  addButton.disabled = !ready || adding || !formValid();
  if (ready) status.textContent = valid.length ? `${valid.filter((row) => !row.value.completed).length} open · ${valid.filter((row) => row.value.completed).length} done` : "No tasks yet";
  list.replaceChildren(...valid.map((row) => {
    const value = row.value; const locked = locks.has(row.key);
    const li = document.createElement("li"); li.className = "task" + (value.completed ? " done" : "");
    const toggle = document.createElement("button"); toggle.type = "button"; toggle.className = "toggle"; toggle.textContent = value.completed ? "✓" : "○"; toggle.disabled = !ready || locked; toggle.setAttribute("aria-label", `${value.completed ? "Reopen" : "Complete"} ${value.text}`); toggle.setAttribute("aria-pressed", String(value.completed)); toggle.addEventListener("click", () => mutateTask(row, (latest) => ({ completed: !latest.completed })));
    const copy = document.createElement("div"); copy.className = "task-copy"; const text = document.createElement("span"); text.className = "task-text"; text.textContent = value.text; const meta = document.createElement("span"); meta.className = "meta"; meta.textContent = value.assigned_to_id ? `Taken by ${person(value.assigned_to_id)}` : `Added by ${person(value.added_by_id)}`; copy.append(text, meta);
    const assign = document.createElement("button"); assign.type = "button"; assign.className = "assign"; assign.disabled = !ready || locked; const mine = me && value.assigned_to_id === me.id; assign.textContent = mine ? "Unassign" : value.assigned_to_id ? "Take over" : "Take this"; assign.setAttribute("aria-label", `${assign.textContent}: ${value.text}`); assign.addEventListener("click", () => mutateTask(row, (latest) => ({ assigned_to_id: latest.assigned_to_id === me.id ? "" : me.id })));
    li.append(toggle, copy, assign); return li;
  }));
  resolveProfiles(valid.flatMap((row) => [row.value.added_by_id, row.value.assigned_to_id]));
}

form.addEventListener("submit", async (event) => {
  event.preventDefault(); const text = input.value.trim(); if (!ready || !text) { if (!ready) showError("Wait for your identity before adding a task."); return; }
  const draft = input.value; adding = true; input.value = ""; paint();
  try { await riot.put("tasks/" + newID(), { text, created_at: Date.now(), added_by_id: me.id, assigned_to_id: "", completed: false }); }
  catch { input.value = draft; input.focus(); showError("Couldn't add that task. Your draft is safe; try again."); }
  finally { adding = false; paint(); }
});
input.addEventListener("input", paint);

async function init() {
  riot.watch("tasks", (next) => { rows = next; paint(); });
  try {
    const identity = await riot.whoami(); if (!validIdentity(identity)) throw new Error("invalid identity"); me = identity;
    await ensureSeeded(); ready = true; paint();
  } catch { ready = false; paint(); showError("Your identity couldn't be verified. Tasks remain read-only."); }
}
init();
