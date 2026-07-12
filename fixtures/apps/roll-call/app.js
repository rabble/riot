"use strict";

const createButton = document.getElementById("create");
const form = document.getElementById("event-form");
const titleInput = document.getElementById("title");
const startsInput = document.getElementById("starts-at");
const placeInput = document.getElementById("place");
const list = document.getElementById("events");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const DEMO_IDS = { alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" };
let me = { id: "", displayName: "member", tag: "" };
let eventRows = [];
let rsvpRows = [];
const names = new Map();
const inflight = new Set();

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join(""); }
function showError(message) { error.textContent = message; error.hidden = false; }
function localInputValue(date) { const shifted = new Date(date.getTime() - date.getTimezoneOffset() * 60000); return shifted.toISOString().slice(0, 16); }
function person(id) { return id === me.id ? "You" : names.get(id) || "A neighbor"; }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!/^[0-9a-f]{64}$/.test(id || "") || inflight.has(id)) return; inflight.add(id); riot.profile(id).then((profile) => { inflight.delete(id); const label = profile.displayName + " · " + profile.tag; if (names.get(id) !== label) { names.set(id, label); paint(); } }).catch(() => inflight.delete(id)); }); }
async function ensureSeeded() {
  if (await riot.get(SEED_MARKER)) return;
  const base = new Date(); base.setHours(18, 0, 0, 0);
  const future = (days, hour) => { const date = new Date(base); date.setDate(date.getDate() + days); date.setHours(hour, 0, 0, 0); return date.toISOString(); };
  const seeds = [
    ["events/community-garden-workday", { title: "Community garden workday", starts_at: future(2, 10), place: "North garden gate", created_by_id: DEMO_IDS.alex }],
    ["events/courtyard-supper", { title: "Courtyard supper", starts_at: future(6, 18), place: "Main courtyard", created_by_id: DEMO_IDS.sam }],
    ["events/repair-cafe", { title: "Repair café", starts_at: future(11, 14), place: "Workshop room", created_by_id: DEMO_IDS.alex }],
  ];
  for (const [key, value] of seeds) if (!(await riot.get(key))) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1, initialized_at: Date.now() });
}
function rsvpsFor(eventID) { return rsvpRows.filter((row) => row.key.startsWith(`rsvps/${eventID}/`) && row.value.attending); }
function paint() {
  error.hidden = true; eventRows.sort((a, b) => new Date(a.value.starts_at) - new Date(b.value.starts_at)); document.getElementById("empty").hidden = eventRows.length > 0;
  list.replaceChildren(...eventRows.map((row) => {
    const value = row.value || {}; const id = row.key.split("/")[1]; const attending = rsvpsFor(id); const mine = attending.some((rsvp) => rsvp.key === `rsvps/${id}/${me.id}`);
    const li = document.createElement("li"); li.className = "event"; const date = new Date(value.starts_at); const dateBlock = document.createElement("div"); dateBlock.className = "date";
    const month = document.createElement("span"); month.className = "month"; month.textContent = date.toLocaleDateString(undefined, { month: "short" }); const day = document.createElement("span"); day.className = "day"; day.textContent = date.toLocaleDateString(undefined, { day: "2-digit" }); dateBlock.append(month, day);
    const copy = document.createElement("div"); copy.className = "event-copy"; const heading = document.createElement("h2"); heading.textContent = String(value.title || "Untitled event"); const details = document.createElement("p"); details.className = "details"; details.textContent = `${date.toLocaleString(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" })} · ${value.place}`; const meta = document.createElement("p"); meta.className = "rsvp-meta"; meta.textContent = attending.length ? `${attending.length} going · hosted by ${person(value.created_by_id)}` : `Be the first to RSVP · hosted by ${person(value.created_by_id)}`; copy.append(heading, details, meta);
    const button = document.createElement("button"); button.type = "button"; button.className = "rsvp" + (mine ? " going" : ""); button.textContent = mine ? "Going ✓" : "I’m going"; button.setAttribute("aria-pressed", String(mine)); button.setAttribute("aria-label", `${mine ? "Cancel RSVP for" : "RSVP to"} ${value.title}`);
    button.addEventListener("click", () => { button.disabled = true; riot.put(`rsvps/${id}/${me.id}`, { attending: !mine, at: Date.now() }).catch(() => { button.disabled = false; showError("Couldn't update your RSVP. Try again."); }); }); li.append(dateBlock, copy, button); return li;
  }));
  resolveProfiles(eventRows.map((row) => row.value.created_by_id));
}
function openForm() { form.hidden = false; createButton.hidden = true; if (!startsInput.value) { const tomorrow = new Date(Date.now() + 86400000); tomorrow.setHours(18, 0, 0, 0); startsInput.value = localInputValue(tomorrow); } titleInput.focus(); }
function closeForm() { form.hidden = true; createButton.hidden = false; }
createButton.addEventListener("click", openForm); document.getElementById("cancel").addEventListener("click", closeForm);
form.addEventListener("submit", (event) => { event.preventDefault(); const title = titleInput.value.trim(); const place = placeInput.value.trim(); const starts = new Date(startsInput.value); if (!title || !place || Number.isNaN(starts.getTime()) || !me.id) return; const draft = { title: titleInput.value, starts: startsInput.value, place: placeInput.value }; riot.put("events/" + newID(), { title, starts_at: starts.toISOString(), place, created_by_id: me.id }).then(() => { form.reset(); closeForm(); status.textContent = "Event saved"; }).catch(() => { titleInput.value = draft.title; startsInput.value = draft.starts; placeInput.value = draft.place; showError("Couldn't save the event. Your draft is safe; try again."); }); });
riot.watch("events", (next) => { eventRows = next; paint(); }); riot.watch("rsvps", (next) => { rsvpRows = next; paint(); });
riot.whoami().then((who) => { me = who; return ensureSeeded(); }).catch(() => showError("Events couldn't open shared storage. Try reopening the app."));
