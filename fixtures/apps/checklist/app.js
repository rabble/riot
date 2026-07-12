"use strict";

const list = document.getElementById("items");
const empty = document.getElementById("empty");
const form = document.getElementById("add-form");
const input = document.getElementById("new-task");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const DEMO_IDS = { alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" };
let me = { id: "", displayName: "member", tag: "" };
let rows = [];
const names = new Map();
const inflight = new Set();

function newID() {
  if (crypto.randomUUID) return crypto.randomUUID().toLowerCase();
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (byte) => byte.toString(16).padStart(2, "0")).join("");
}
function showError(message) { error.textContent = message; error.hidden = false; }
function person(id) { return id === me.id ? "You" : names.get(id) || "A neighbor"; }
function resolveProfiles(ids) {
  [...new Set(ids)].forEach((id) => {
    if (!/^[0-9a-f]{64}$/.test(id || "") || inflight.has(id)) return;
    inflight.add(id);
    riot.profile(id).then((profile) => {
      inflight.delete(id);
      const label = profile.displayName + " · " + profile.tag;
      if (names.get(id) !== label) { names.set(id, label); paint(); }
    }).catch(() => { inflight.delete(id); });
  });
}

async function ensureSeeded() {
  if (await riot.get(SEED_MARKER)) return;
  const seeds = [
    ["tasks/bring-extension-cord", { text: "Bring the long extension cord", created_at: 1, added_by_id: DEMO_IDS.alex, assigned_to_id: "", completed: false }],
    ["tasks/check-garden-gate", { text: "Check the garden gate latch", created_at: 2, added_by_id: DEMO_IDS.sam, assigned_to_id: DEMO_IDS.alex, completed: false }],
    ["tasks/print-sign-in-sheet", { text: "Print the sign-in sheet", created_at: 3, added_by_id: DEMO_IDS.alex, assigned_to_id: DEMO_IDS.sam, completed: true }],
  ];
  for (const [key, value] of seeds) if (!(await riot.get(key))) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1 });
}

function paint() {
  error.hidden = true;
  rows.sort((a, b) => (a.value.completed - b.value.completed) || ((a.value.created_at || 0) - (b.value.created_at || 0)));
  empty.hidden = rows.length > 0;
  status.textContent = rows.length ? `${rows.filter((row) => !row.value.completed).length} open · ${rows.filter((row) => row.value.completed).length} done` : "";
  list.replaceChildren(...rows.map((row) => {
    const value = row.value || {};
    const li = document.createElement("li"); li.className = "task" + (value.completed ? " done" : "");
    const toggle = document.createElement("button"); toggle.type = "button"; toggle.className = "toggle"; toggle.textContent = value.completed ? "✓" : "○"; toggle.setAttribute("aria-label", `${value.completed ? "Reopen" : "Complete"} ${value.text}`); toggle.setAttribute("aria-pressed", String(Boolean(value.completed)));
    toggle.addEventListener("click", () => { toggle.disabled = true; riot.put(row.key, { ...value, completed: !value.completed }).catch(() => { toggle.disabled = false; showError("Couldn't update that task. Your text is still here; try again."); }); });
    const copy = document.createElement("div"); copy.className = "task-copy";
    const text = document.createElement("span"); text.className = "task-text"; text.textContent = String(value.text || "Untitled task");
    const meta = document.createElement("span"); meta.className = "meta"; meta.textContent = value.assigned_to_id ? `Taken by ${person(value.assigned_to_id)}` : `Added by ${person(value.added_by_id)}`; copy.append(text, meta);
    const assign = document.createElement("button"); assign.type = "button"; assign.className = "assign"; const mine = value.assigned_to_id === me.id; assign.textContent = mine ? "Unassign" : value.assigned_to_id ? "Take over" : "Take this"; assign.setAttribute("aria-label", `${assign.textContent}: ${value.text}`);
    assign.addEventListener("click", () => { assign.disabled = true; riot.put(row.key, { ...value, assigned_to_id: mine ? "" : me.id }).catch(() => { assign.disabled = false; showError("Couldn't change the assignment. Try again."); }); });
    li.append(toggle, copy, assign); return li;
  }));
  resolveProfiles(rows.flatMap((row) => [row.value.added_by_id, row.value.assigned_to_id]));
}

form.addEventListener("submit", (event) => {
  event.preventDefault(); const text = input.value.trim(); if (!text || !me.id) return;
  const draft = input.value; input.value = "";
  riot.put("tasks/" + newID(), { text, created_at: Date.now(), added_by_id: me.id, assigned_to_id: "", completed: false })
    .then(() => { status.textContent = "Task added"; })
    .catch(() => { input.value = draft; input.focus(); showError("Couldn't add that task. Your draft is safe; try again."); });
});

riot.watch("tasks", (next) => { rows = next; paint(); });
riot.whoami().then((who) => { me = who; return ensureSeeded(); }).catch(() => { showError("Tasks couldn't open shared storage. Try reopening the app."); });
