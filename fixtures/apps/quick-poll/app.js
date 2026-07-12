"use strict";

const poll = document.getElementById("poll");
const questionElement = document.getElementById("question");
const askedBy = document.getElementById("asked-by");
const optionsElement = document.getElementById("options");
const tally = document.getElementById("tally");
const replaceButton = document.getElementById("replace");
const form = document.getElementById("ask-form");
const postButton = document.getElementById("post-question");
const questionInput = document.getElementById("question-input");
const optionInputs = [1, 2, 3, 4].map((n) => document.getElementById("option-" + n));
const error = document.getElementById("error");
const PROPOSAL_KEY = "proposals/current";
const SEED_MARKER = "meta/seeded";
const ID_PATTERN = /^[0-9a-f]{64}$/;
const COMPONENT = /^[a-z0-9-]{1,256}$/;
const DEMO_ID = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
let me = null;
let proposalRows = [];
let voteRows = [];
let ready = false;
let posting = false;
const voteLocks = new Set();
const profiles = new Map();
const inflightProfiles = new Set();

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join(""); }
function validIdentity(value) { return value && ID_PATTERN.test(value.id || ""); }
function validProposal(row) { const value = row && row.value; return Boolean(row && row.key === PROPOSAL_KEY && value && typeof value === "object" && COMPONENT.test(value.id || "") && typeof value.text === "string" && value.text.trim() && value.text.length <= 180 && Array.isArray(value.options) && value.options.length >= 2 && value.options.length <= 4 && value.options.every((option) => typeof option === "string" && option.trim() && option.length <= 100) && ID_PATTERN.test(value.asked_by_id || "") && Number.isFinite(value.at) && value.at >= 0); }
function validVote(row) { const value = row && row.value; const parts = row && typeof row.key === "string" ? row.key.split("/") : []; return Boolean(parts.length === 3 && parts[0] === "votes" && COMPONENT.test(parts[1]) && ID_PATTERN.test(parts[2]) && value && typeof value === "object" && Number.isInteger(value.choice) && value.choice >= 0 && value.choice < 4 && Number.isFinite(value.at) && value.at >= 0); }
function showError(message) { error.textContent = message; error.hidden = false; }
function currentProposal() { return proposalRows.find(validProposal)?.value || null; }
function profileLabel(id) { if (me && id === me.id) return "You"; const profile = profiles.get(id); return profile ? profile.displayName + " · " + profile.tag : "A neighbor"; }
function resolveProfile(id) { if (!ID_PATTERN.test(id || "") || inflightProfiles.has(id) || profiles.has(id)) return; inflightProfiles.add(id); riot.profile(id).then((profile) => { inflightProfiles.delete(id); profiles.set(id, profile); paint(); }).catch(() => inflightProfiles.delete(id)); }

async function ensureSeeded() {
  const existing = await riot.list("proposals"); const marker = await riot.get(SEED_MARKER);
  if (existing.length && (!marker || marker.status !== "seeding")) { if (!marker || marker.status !== "ready") await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; }
  if (marker && marker.status !== "seeding") return;
  if (!marker) { await riot.put(SEED_MARKER, { version: 1, status: "seeding" }); if ((await riot.list("proposals")).length) { await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; } }
  // The storage API has no compare-and-set. The reserved singleton key and a
  // final re-read avoid overwriting any proposal already visible to this app.
  if ((await riot.get(PROPOSAL_KEY)) === null) await riot.put(PROPOSAL_KEY, { id: "safer-school-crossing", text: "How should we make the school crossing safer?", options: ["Add a crossing guard", "Paint a brighter crosswalk", "Add flashing warning lights"], asked_by_id: DEMO_ID, at: 1 });
  await riot.put(SEED_MARKER, { version: 1, status: "ready" });
}
function voteKey(proposalID) { return `votes/${proposalID}/${me.id}`; }
async function vote(proposal, choice) {
  const key = voteKey(proposal.id); if (!ready || voteLocks.has(key)) return; voteLocks.add(key); paint();
  try { await riot.put(key, { choice, at: Date.now() }); }
  catch { showError("Couldn't record your vote. Your previous vote is unchanged; try again."); }
  finally { voteLocks.delete(key); paint(); }
}
function paint() {
  replaceButton.disabled = !ready; postButton.disabled = !ready || posting; const proposal = currentProposal();
  if (!proposal) { questionElement.textContent = "No decision is open yet"; askedBy.textContent = "Ready when you are"; optionsElement.replaceChildren(); tally.textContent = "No votes yet"; poll.hidden = false; return; }
  resolveProfile(proposal.asked_by_id); questionElement.textContent = proposal.text; askedBy.textContent = `Asked by ${profileLabel(proposal.asked_by_id)}`;
  const relevant = voteRows.filter(validVote).filter((row) => row.key.startsWith(`votes/${proposal.id}/`) && row.value.choice < proposal.options.length); const counts = proposal.options.map(() => 0); let mine = -1;
  relevant.forEach((row) => { counts[row.value.choice] += 1; if (row.key === voteKey(proposal.id)) mine = row.value.choice; }); const total = counts.reduce((sum, count) => sum + count, 0); const locked = voteLocks.has(voteKey(proposal.id));
  optionsElement.replaceChildren(...proposal.options.map((label, index) => {
    const li = document.createElement("li"); const button = document.createElement("button"); button.type = "button"; button.className = "option" + (mine === index ? " mine" : ""); button.disabled = !ready || locked; button.setAttribute("aria-pressed", String(mine === index)); button.setAttribute("aria-label", label);
    const bar = document.createElement("span"); bar.className = "bar"; bar.style.width = `${total ? counts[index] / total * 100 : 0}%`; bar.setAttribute("aria-hidden", "true"); const content = document.createElement("span"); content.className = "option-content"; const text = document.createElement("span"); text.className = "label"; text.textContent = label; const count = document.createElement("span"); count.className = "votes"; count.textContent = counts[index] === 1 ? "1 vote" : `${counts[index]} votes`; content.append(text, count); button.append(bar, content); button.addEventListener("click", () => vote(proposal, index)); li.append(button); return li;
  })); tally.textContent = total === 1 ? "1 vote" : `${total} votes`; poll.hidden = false;
}
function openForm() { if (!ready) { showError("Wait for your identity before asking a question."); return; } poll.hidden = true; form.hidden = false; questionInput.focus(); }
function closeForm(returnFocus) { form.hidden = true; poll.hidden = false; if (returnFocus) replaceButton.focus(); }
replaceButton.addEventListener("click", openForm); document.getElementById("cancel").addEventListener("click", () => closeForm(true));
form.addEventListener("submit", async (event) => {
  event.preventDefault(); const text = questionInput.value.trim(); const choices = optionInputs.map((input) => input.value.trim()).filter(Boolean); if (!ready || !text || choices.length < 2) { if (!ready) showError("Wait for your identity before posting."); return; } const drafts = [questionInput.value, ...optionInputs.map((input) => input.value)]; posting = true; paint();
  try { await riot.put(PROPOSAL_KEY, { id: newID(), text, options: choices, asked_by_id: me.id, at: Date.now() }); form.reset(); closeForm(false); }
  catch { questionInput.value = drafts[0]; optionInputs.forEach((input, index) => { input.value = drafts[index + 1]; }); showError("Couldn't post the question. Your draft is safe; try again."); }
  finally { posting = false; paint(); }
});
async function init() {
  try { const identity = await riot.whoami(); if (!validIdentity(identity)) throw new Error("invalid identity"); me = identity; await ensureSeeded(); ready = true; paint(); riot.watch("proposals", (next) => { proposalRows = next; paint(); }); riot.watch("votes", (next) => { voteRows = next; paint(); }); }
  catch { ready = false; paint(); showError("Your identity couldn't be verified. Decisions remain read-only."); }
}
init();
