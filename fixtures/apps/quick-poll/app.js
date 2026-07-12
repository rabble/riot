"use strict";

const poll = document.getElementById("poll");
const questionElement = document.getElementById("question");
const askedBy = document.getElementById("asked-by");
const optionsElement = document.getElementById("options");
const tally = document.getElementById("tally");
const form = document.getElementById("ask-form");
const questionInput = document.getElementById("question-input");
const optionInputs = [1, 2, 3, 4].map((n) => document.getElementById("option-" + n));
const error = document.getElementById("error");
const PROPOSAL_KEY = "proposals/current";
const SEED_MARKER = "meta/seeded";
const DEMO_ID = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
let me = { id: "", displayName: "member", tag: "" };
let proposal = null;
let votes = [];
let askerName = "A neighbor";

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join(""); }
function showError(message) { error.textContent = message; error.hidden = false; }
async function ensureSeeded() {
  if (await riot.get(SEED_MARKER)) return;
  if (!(await riot.get(PROPOSAL_KEY))) await riot.put(PROPOSAL_KEY, { id: "safer-school-crossing", text: "How should we make the school crossing safer?", options: ["Add a crossing guard", "Paint a brighter crosswalk", "Add flashing warning lights"], asked_by_id: DEMO_ID, at: 1 });
  await riot.put(SEED_MARKER, { version: 1 });
}
function voteKey(proposalID) { return `votes/${proposalID}/${me.id}`; }
function paint() {
  error.hidden = true; if (!proposal || !Array.isArray(proposal.options)) return;
  questionElement.textContent = String(proposal.text || "Untitled question"); askedBy.textContent = `Asked by ${askerName}`;
  const relevant = votes.filter((row) => row.key.startsWith(`votes/${proposal.id}/`) && Number.isInteger(row.value.choice)); const counts = proposal.options.map(() => 0); let mine = -1;
  relevant.forEach((row) => { if (row.value.choice >= 0 && row.value.choice < counts.length) counts[row.value.choice] += 1; if (row.key === voteKey(proposal.id)) mine = row.value.choice; }); const total = counts.reduce((sum, count) => sum + count, 0);
  optionsElement.replaceChildren(...proposal.options.map((label, index) => {
    const li = document.createElement("li"); const button = document.createElement("button"); button.type = "button"; button.className = "option" + (mine === index ? " mine" : ""); button.setAttribute("aria-pressed", String(mine === index)); button.setAttribute("aria-label", String(label)); button.disabled = !me.id;
    const bar = document.createElement("span"); bar.className = "bar"; bar.style.width = `${total ? counts[index] / total * 100 : 0}%`; bar.setAttribute("aria-hidden", "true");
    const content = document.createElement("span"); content.className = "option-content"; const text = document.createElement("span"); text.className = "label"; text.textContent = String(label); const count = document.createElement("span"); count.className = "votes"; count.textContent = counts[index] === 1 ? "1 vote" : `${counts[index]} votes`; content.append(text, count); button.append(bar, content);
    button.addEventListener("click", () => { button.disabled = true; riot.put(voteKey(proposal.id), { choice: index, at: Date.now() }).catch(() => { button.disabled = false; showError("Couldn't record your vote. Your previous vote is unchanged; try again."); }); }); li.append(button); return li;
  })); tally.textContent = total === 1 ? "1 vote" : `${total} votes`; poll.hidden = false;
}
function refreshAsker() { if (!proposal || !/^[0-9a-f]{64}$/.test(proposal.asked_by_id || "")) return; riot.profile(proposal.asked_by_id).then((profile) => { askerName = proposal.asked_by_id === me.id ? "You" : profile.displayName + " · " + profile.tag; paint(); }).catch(() => {}); }
document.getElementById("replace").addEventListener("click", () => { poll.hidden = true; form.hidden = false; questionInput.focus(); });
document.getElementById("cancel").addEventListener("click", () => { form.hidden = true; poll.hidden = false; });
form.addEventListener("submit", (event) => { event.preventDefault(); const text = questionInput.value.trim(); const choices = optionInputs.map((input) => input.value.trim()).filter(Boolean); if (!text || choices.length < 2 || !me.id) return; const drafts = [questionInput.value, ...optionInputs.map((input) => input.value)]; riot.put(PROPOSAL_KEY, { id: newID(), text, options: choices, asked_by_id: me.id, at: Date.now() }).then(() => { form.reset(); form.hidden = true; poll.hidden = false; }).catch(() => { questionInput.value = drafts[0]; optionInputs.forEach((input, index) => { input.value = drafts[index + 1]; }); showError("Couldn't post the question. Your draft is safe; try again."); }); });
riot.watch("proposals", (rows) => { const current = rows.find((row) => row.key === PROPOSAL_KEY); proposal = current ? current.value : null; refreshAsker(); paint(); });
riot.watch("votes", (rows) => { votes = rows; paint(); });
riot.whoami().then((who) => { me = who; return ensureSeeded(); }).catch(() => showError("Decisions couldn't open shared storage. Try reopening the app."));
