"use strict";

const form = document.getElementById("composer");
const input = document.getElementById("message");
const sendButton = document.getElementById("send");
const list = document.getElementById("messages");
const empty = document.getElementById("empty");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const ID_PATTERN = /^[0-9a-f]{64}$/;
const MESSAGE_KEY = /^messages\/([0-9]{1,16})-([a-z0-9-]{1,80})$/;
const DEMO_IDS = {
  alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  jo: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
};
let me = null;
let rows = [];
let ready = false;
let sending = false;
const names = new Map();
const inflightProfiles = new Set();

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (byte) => byte.toString(16).padStart(2, "0")).join(""); }
function validIdentity(value) { return Boolean(value && ID_PATTERN.test(value.id || "")); }
function validMessage(row) { const match = row && typeof row.key === "string" ? row.key.match(MESSAGE_KEY) : null; const value = row && row.value; return Boolean(match && value && typeof value === "object" && typeof value.text === "string" && value.text.trim() && value.text.length <= 500 && Number.isFinite(value.created_at) && value.created_at >= 0 && ID_PATTERN.test(value.author_id || "")); }
function showError(message) { error.textContent = message; error.hidden = false; status.textContent = message; }
function person(id) { if (me && id === me.id) return "You"; const profile = names.get(id); return profile ? `${profile.displayName} · ${profile.tag}` : "A neighbor"; }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!ID_PATTERN.test(id || "") || inflightProfiles.has(id) || names.has(id)) return; inflightProfiles.add(id); riot.profile(id).then((profile) => { inflightProfiles.delete(id); if (profile && typeof profile.displayName === "string" && typeof profile.tag === "string") names.set(id, profile); paint(); }).catch(() => inflightProfiles.delete(id)); }); }
function nearBottom() { return document.documentElement.scrollHeight - window.scrollY - window.innerHeight < 120; }

async function ensureSeeded() {
  const existing = await riot.list("messages"); const marker = await riot.get(SEED_MARKER);
  if (existing.length && (!marker || marker.status !== "seeding")) { if (!marker || marker.status !== "ready") await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; }
  if (marker && marker.status !== "seeding") return;
  if (!marker) { await riot.put(SEED_MARKER, { version: 1, status: "seeding" }); if ((await riot.list("messages")).length) { await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; } }
  const seeds = [
    ["messages/1-checking-in", { text: "Is anyone heading past the community room?", created_at: 1, author_id: DEMO_IDS.alex }],
    ["messages/2-tea", { text: "I can bring extra tea.", created_at: 2, author_id: DEMO_IDS.sam }],
    ["messages/3-cups", { text: "Great — I’ll leave clean cups by the door.", created_at: 3, author_id: DEMO_IDS.jo }],
  ];
  for (const [key, value] of seeds) if ((await riot.get(key)) === null) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1, status: "ready" });
}

function paint() {
  const shouldScroll = nearBottom();
  const valid = rows.filter(validMessage).sort((left, right) => left.value.created_at - right.value.created_at || left.key.localeCompare(right.key));
  sendButton.disabled = !ready || sending || !input.value.trim();
  empty.hidden = valid.length > 0;
  if (ready) status.textContent = valid.length ? `${valid.length} messages` : "No messages yet";
  list.replaceChildren(...valid.map((row, index) => {
    const value = row.value; const previous = valid[index - 1]; const grouped = Boolean(previous && previous.value.author_id === value.author_id);
    const item = document.createElement("li"); item.className = `message${value.author_id === (me && me.id) ? " mine" : ""}${grouped ? " grouped" : " group-start"}`;
    const text = document.createElement("p"); text.textContent = value.text;
    const meta = document.createElement("p"); meta.className = "meta"; meta.textContent = `${person(value.author_id)} · ${new Date(value.created_at).toLocaleString()}`;
    item.append(text, meta); return item;
  }));
  resolveProfiles(valid.map((row) => row.value.author_id));
  if (shouldScroll) requestAnimationFrame(() => window.scrollTo({ top: document.documentElement.scrollHeight, behavior: "smooth" }));
}

form.addEventListener("submit", async (event) => {
  event.preventDefault(); const text = input.value.trim(); if (!ready || !text) { if (!ready) showError("Wait for your identity before sending."); return; }
  const draft = input.value; const createdAt = Date.now(); input.value = ""; sending = true; paint();
  try { await riot.put(`messages/${createdAt}-${newID()}`, { text, created_at: createdAt, author_id: me.id }); }
  catch { input.value = draft; input.focus(); showError("Couldn't send that message. Your draft is safe; try again."); }
  finally { sending = false; paint(); }
});
input.addEventListener("input", paint);
async function init() {
  riot.watch("messages", (next) => { rows = Array.isArray(next) ? next : []; paint(); });
  try { const identity = await riot.whoami(); if (!validIdentity(identity)) throw new Error("invalid identity"); me = identity; await ensureSeeded(); ready = true; paint(); }
  catch { ready = false; paint(); showError("Your identity couldn't be verified. Chat remains read-only."); }
}
init();
