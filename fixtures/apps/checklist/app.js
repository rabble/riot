"use strict";

const list = document.getElementById("items");
const empty = document.getElementById("empty");
const form = document.getElementById("add-form");
const input = document.getElementById("new-item");
const error = document.getElementById("error");

let me = { displayName: "" };
riot.whoami().then((who) => { me = who; }).catch(() => {});

function newID() {
  if (crypto.randomUUID) { return crypto.randomUUID(); }
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
}

function showError(message) {
  error.textContent = message;
  error.hidden = false;
}

function stamp() {
  return { updated_by: me.displayName, updated_at: Date.now() };
}

function render(rows) {
  error.hidden = true;
  rows.sort((a, b) => (a.value.updated_at || 0) - (b.value.updated_at || 0));
  empty.hidden = rows.length > 0;
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
    meta.textContent = row.value.updated_by || "";
    li.append(box, label, meta);
    return li;
  }));
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
