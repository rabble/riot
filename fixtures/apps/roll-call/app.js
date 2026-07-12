"use strict";

const createButton = document.getElementById("create");
const form = document.getElementById("event-form");
const saveButton = document.getElementById("save");
const titleInput = document.getElementById("title");
const startsInput = document.getElementById("starts-at");
const placeInput = document.getElementById("place");
const list = document.getElementById("events");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const DEFAULT_PLACE = "Place to be decided";
const ID_PATTERN = /^[0-9a-f]{64}$/;
const EVENT_KEY = /^events\/[a-z0-9-]{1,256}$/;
const RSVP_KEY = /^rsvps\/([a-z0-9-]{1,256})\/([0-9a-f]{64})$/;
const DEMO_IDS = { alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" };
let me = null;
let eventRows = [];
let rsvpRows = [];
let ready = false;
let saving = false;
const rsvpLocks = new Set();
const names = new Map();
const inflightProfiles = new Set();

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join(""); }
function validIdentity(value) { return value && ID_PATTERN.test(value.id || ""); }
function validEvent(row) { const value = row && row.value; return Boolean(row && EVENT_KEY.test(row.key) && value && typeof value === "object" && typeof value.title === "string" && value.title.trim() && value.title.length <= 120 && typeof value.starts_at === "string" && Number.isFinite(Date.parse(value.starts_at)) && typeof value.place === "string" && value.place.length <= 100 && ID_PATTERN.test(value.created_by_id || "")); }
function validRsvp(row) { const match = row && typeof row.key === "string" ? row.key.match(RSVP_KEY) : null; const value = row && row.value; return Boolean(match && value && typeof value === "object" && typeof value.attending === "boolean" && Number.isFinite(value.at) && value.at >= 0); }
function showError(message) { error.textContent = message; error.hidden = false; status.textContent = message; }
function formValid() { return titleInput.value.trim().length > 0 && Number.isFinite(new Date(startsInput.value).getTime()); }
function person(id) { return me && id === me.id ? "You" : names.get(id) || "A neighbor"; }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!ID_PATTERN.test(id || "") || inflightProfiles.has(id)) return; inflightProfiles.add(id); riot.profile(id).then((profile) => { inflightProfiles.delete(id); const label = profile.displayName + " · " + profile.tag; if (names.get(id) !== label) { names.set(id, label); paint(); } }).catch(() => inflightProfiles.delete(id)); }); }

async function ensureSeeded() {
  const existing = await riot.list("events"); const marker = await riot.get(SEED_MARKER);
  if (existing.length && (!marker || marker.status !== "seeding")) { if (!marker || marker.status !== "ready") await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; }
  if (marker && marker.status !== "seeding") return;
  if (!marker) { await riot.put(SEED_MARKER, { version: 1, status: "seeding" }); if ((await riot.list("events")).length) { await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; } }
  const base = new Date(); base.setHours(18, 0, 0, 0); const future = (days, hour) => { const date = new Date(base); date.setDate(date.getDate() + days); date.setHours(hour, 0, 0, 0); return date.toISOString(); };
  const seeds = [
    ["events/community-garden-workday", { title: "Community garden workday", starts_at: future(2, 10), place: "North garden gate", created_by_id: DEMO_IDS.alex }],
    ["events/courtyard-supper", { title: "Courtyard supper", starts_at: future(6, 18), place: "Main courtyard", created_by_id: DEMO_IDS.sam }],
    ["events/repair-cafe", { title: "Repair café", starts_at: future(11, 14), place: "Workshop room", created_by_id: DEMO_IDS.alex }],
  ];
  // Without compare-and-set, reserved keys plus re-reads are the strongest
  // available protection. Visible synced records are never overwritten here.
  for (const [key, value] of seeds) if ((await riot.get(key)) === null) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1, status: "ready", initialized_at: Date.now() });
}
function rsvpsFor(eventID) { return rsvpRows.filter(validRsvp).filter((row) => row.key.startsWith(`rsvps/${eventID}/`) && row.value.attending); }
async function toggleRsvp(eventID) {
  const key = `rsvps/${eventID}/${me.id}`; if (!ready || rsvpLocks.has(key)) return; rsvpLocks.add(key); paint();
  try { const latest = await riot.get(key); await riot.put(key, { attending: !(latest && latest.attending === true), at: Date.now() }); }
  catch { showError("Couldn't update your RSVP. Try again."); }
  finally { rsvpLocks.delete(key); paint(); }
}
function paint() {
  const valid = eventRows.filter(validEvent).sort((a, b) => Date.parse(a.value.starts_at) - Date.parse(b.value.starts_at)); createButton.disabled = !ready; saveButton.disabled = !ready || saving || !formValid(); document.getElementById("empty").hidden = valid.length > 0;
  if (ready) status.textContent = valid.length ? `${valid.length} upcoming events` : "No events are scheduled";
  list.replaceChildren(...valid.map((row) => {
    const value = row.value; const id = row.key.split("/")[1]; const attending = rsvpsFor(id); const key = `rsvps/${id}/${me ? me.id : ""}`; const mine = Boolean(me) && attending.some((rsvp) => rsvp.key === key);
    const li = document.createElement("li"); li.className = "event"; const date = new Date(value.starts_at); const dateBlock = document.createElement("div"); dateBlock.className = "date"; const month = document.createElement("span"); month.className = "month"; month.textContent = date.toLocaleDateString(undefined, { month: "short" }); const day = document.createElement("span"); day.className = "day"; day.textContent = date.toLocaleDateString(undefined, { day: "2-digit" }); dateBlock.append(month, day);
    const copy = document.createElement("div"); copy.className = "event-copy"; const heading = document.createElement("h2"); heading.textContent = value.title; const details = document.createElement("p"); details.className = "details"; details.textContent = `${date.toLocaleString(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" })} · ${value.place || DEFAULT_PLACE}`; const meta = document.createElement("p"); meta.className = "rsvp-meta"; meta.textContent = attending.length ? `${attending.length} going · hosted by ${person(value.created_by_id)}` : `Be the first to RSVP · hosted by ${person(value.created_by_id)}`; copy.append(heading, details, meta);
    const button = document.createElement("button"); button.type = "button"; button.className = "rsvp" + (mine ? " going" : ""); button.disabled = !ready || rsvpLocks.has(key); button.textContent = mine ? "Going ✓" : "I’m going"; button.setAttribute("aria-pressed", String(mine)); button.setAttribute("aria-label", `${mine ? "Cancel RSVP for" : "RSVP to"} ${value.title}`); button.addEventListener("click", () => toggleRsvp(id)); li.append(dateBlock, copy, button); return li;
  }));
  resolveProfiles(valid.map((row) => row.value.created_by_id));
}
function openForm() { if (!ready) { showError("Wait for your identity before creating an event."); return; } form.hidden = false; createButton.hidden = true; if (!startsInput.value) { const tomorrow = new Date(Date.now() + 86400000); tomorrow.setHours(18, 0, 0, 0); startsInput.value = localInputValue(tomorrow); } titleInput.focus(); }
function localInputValue(date) { const shifted = new Date(date.getTime() - date.getTimezoneOffset() * 60000); return shifted.toISOString().slice(0, 16); }
function closeForm(returnFocus) { form.hidden = true; createButton.hidden = false; if (returnFocus) createButton.focus(); }
createButton.addEventListener("click", openForm); document.getElementById("cancel").addEventListener("click", () => closeForm(true));
form.addEventListener("input", paint); form.addEventListener("change", paint);
form.addEventListener("submit", async (event) => {
  event.preventDefault(); const title = titleInput.value.trim(); const place = placeInput.value.trim() || DEFAULT_PLACE; const starts = new Date(startsInput.value); if (!ready || !title || Number.isNaN(starts.getTime())) { if (!ready) showError("Wait for your identity before saving."); return; } const draft = { title: titleInput.value, starts: startsInput.value, place: placeInput.value }; saving = true; paint();
  try { await riot.put("events/" + newID(), { title, starts_at: starts.toISOString(), place, created_by_id: me.id }); form.reset(); closeForm(false); }
  catch { titleInput.value = draft.title; startsInput.value = draft.starts; placeInput.value = draft.place; showError("Couldn't save the event. Your draft is safe; try again."); }
  finally { saving = false; paint(); }
});
async function init() {
  riot.watch("events", (next) => { eventRows = next; paint(); }); riot.watch("rsvps", (next) => { rsvpRows = next; paint(); });
  try { const identity = await riot.whoami(); if (!validIdentity(identity)) throw new Error("invalid identity"); me = identity; await ensureSeeded(); ready = true; paint(); }
  catch { ready = false; paint(); showError("Your identity couldn't be verified. Events remain read-only."); }
}
init();
