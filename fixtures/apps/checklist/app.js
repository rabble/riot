"use strict";

const list = document.getElementById("items");
const empty = document.getElementById("empty");
const form = document.getElementById("add-form");
const input = document.getElementById("new-item");
const error = document.getElementById("error");

// An item records WHO touched it as an id, never as a name. A name is a claim
// its owner can change; a stored name is a snapshot, and no later rename can
// ever go back and repair it. Store the id, resolve the name at render time,
// and one rename fixes every row that person ever touched.
let me = { id: "", displayName: "member", tag: "" };
riot.whoami().then((who) => { me = who; }).catch(() => {});

// id -> "Name · tag", last known. Purely a paint cache so a re-render doesn't
// flicker: every render still re-resolves each id ONCE (once per unique id, not
// once per row), so a rename — or a peer's profile card that only just synced
// in — shows up on the next change without reopening the app.
const names = new Map();

function newID() {
  if (crypto.randomUUID) { return crypto.randomUUID(); }
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
}

function showError(message) {
  error.textContent = message;
  error.hidden = false;
}

function stamp() {
  return { updated_by_id: me.id, updated_at: Date.now() };
}

// Items written before ids carry `updated_by`: a bare name SNAPSHOT with no id
// behind it. Show it exactly as stored — there is nothing to resolve and nothing
// to repair — and never mistake it for something resolvable.
function legacyAttribution(value) {
  return typeof value.updated_by === "string" ? value.updated_by : "";
}

function render(rows) {
  error.hidden = true;
  rows.sort((a, b) => (a.value.updated_at || 0) - (b.value.updated_at || 0));
  empty.hidden = rows.length > 0;
  // id -> every meta element in this render waiting on that id. Repeated
  // authors collapse into one lookup.
  const pending = new Map();
  list.replaceChildren(...rows.map((row) => {
    const li = document.createElement("li");
    if (row.value.done) { li.className = "done"; }
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = Boolean(row.value.done);
    box.setAttribute("aria-label", row.value.text);
    box.addEventListener("change", () => {
      riot.put(row.key, { ...row.value, done: box.checked, ...stamp() })
        .catch(() => { box.checked = !box.checked; showError("Couldn't save that — try again"); });
    });
    const label = document.createElement("label");
    label.textContent = row.value.text;
    box.id = "box-" + row.key.replaceAll("/", "-");
    label.htmlFor = box.id;
    const meta = document.createElement("span");
    meta.className = "meta";
    const id = row.value.updated_by_id;
    if (typeof id === "string" && id) {
      meta.textContent = names.get(id) || "";
      const waiting = pending.get(id);
      if (waiting) { waiting.push(meta); } else { pending.set(id, [meta]); }
    } else {
      meta.textContent = legacyAttribution(row.value);
    }
    li.append(box, label, meta);
    return li;
  }));
  pending.forEach((elements, id) => {
    riot.profile(id).then((who) => {
      // The host hands the name and the tag over as separate fields precisely so
      // this line can put them back together. The name is already sanitized —
      // it cannot contain "·" — so flattening it here cannot forge a second tag.
      const text = who.displayName + " · " + who.tag;
      names.set(id, text);
      elements.forEach((el) => { el.textContent = text; });
    }).catch(() => {});
  });
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = input.value.trim();
  if (!text) { return; }
  input.value = "";
  riot.put("items/" + newID(), { text, done: false, ...stamp() })
    .catch(() => { input.value = text; showError("Couldn't save that — try again"); });
});

riot.watch("items", render);
