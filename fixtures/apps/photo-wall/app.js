"use strict";

const form = document.getElementById("share-form");
const captionInput = document.getElementById("caption");
const fileInput = document.getElementById("photo");
const shareButton = document.getElementById("share");
const preview = document.getElementById("preview");
const previewImage = document.getElementById("preview-image");
const previewNote = document.getElementById("preview-note");
const list = document.getElementById("photos");
const empty = document.getElementById("empty");
const error = document.getElementById("error");
const status = document.getElementById("status");
const SEED_MARKER = "meta/seeded";
const ID_PATTERN = /^[0-9a-f]{64}$/;
const PHOTO_KEY = /^photos\/([0-9]{1,16})-([a-z0-9-]{1,80})$/;
const IMAGE_DATA = /^data:image\/(?:jpeg|png|webp|gif|svg\+xml)(?:;charset=[a-z0-9-]+)?(?:;base64)?,/i;
const RASTER_IMAGE_DATA = /^data:image\/(?:jpeg|png|webp|gif)(?:;charset=[a-z0-9-]+)?(?:;base64)?,/i;
const MAX_DATA_URL_BYTES = 350 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const MAX_DECODED_PIXELS = 24 * 1024 * 1024;
const MAX_RENDERED_PHOTOS = 200;
const DEMO_IDS = {
  alex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  sam: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  jo: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
};
const SEED_SVGS = {
  courtyard: `<svg xmlns="http://www.w3.org/2000/svg" width="720" height="520" viewBox="0 0 720 520"><rect width="720" height="520" fill="#e5c98f"/><rect y="330" width="720" height="190" fill="#b66f45"/><path d="M0 70h720v260H0z" fill="#f3e4c0"/><path d="M60 105h150v170H60zm250 0h150v170H310zm250 0h100v170H560z" fill="#487b76"/><circle cx="145" cy="390" r="62" fill="#f7d563"/><circle cx="575" cy="405" r="70" fill="#7d9b55"/><path d="M110 390h470M205 350v115M500 350v115" stroke="#5a352a" stroke-width="20" stroke-linecap="round"/><circle cx="360" cy="205" r="52" fill="#bd4778"/><path d="M360 150v110M305 205h110" stroke="#f7ead0" stroke-width="12"/></svg>`,
  tools: `<svg xmlns="http://www.w3.org/2000/svg" width="720" height="500" viewBox="0 0 720 500"><rect width="720" height="500" fill="#d6bd88"/><rect x="55" y="55" width="610" height="390" rx="24" fill="#315f58"/><path d="M105 150h510M105 270h510M105 390h510" stroke="#f0d89e" stroke-width="18"/><path d="M150 110v90m-30-45h60M300 90v130m-35-25 70-70M470 95v120m65-95-65 65" stroke="#e76f51" stroke-width="18" stroke-linecap="round"/><circle cx="170" cy="330" r="38" fill="#e9c84b"/><path d="M270 320h130l30 55H240z" fill="#b6d7cf"/><path d="M520 305l65 65m0-65-65 65" stroke="#f2a7c4" stroke-width="22" stroke-linecap="round"/></svg>`,
  feast: `<svg xmlns="http://www.w3.org/2000/svg" width="720" height="540" viewBox="0 0 720 540"><rect width="720" height="540" fill="#274f61"/><circle cx="610" cy="85" r="45" fill="#f5d66d"/><path d="M0 355h720v185H0z" fill="#c65c49"/><path d="M55 105h610v180H55z" fill="#efe0b9"/><path d="M100 165h520" stroke="#b13f6b" stroke-width="18" stroke-dasharray="35 18"/><path d="M100 350h520l-45 105H145z" fill="#f2c96d"/><circle cx="205" cy="382" r="34" fill="#5e9b64"/><circle cx="360" cy="382" r="34" fill="#d94f66"/><circle cx="515" cy="382" r="34" fill="#6d74bb"/><path d="M170 300v175m380-175v175" stroke="#4b3027" stroke-width="18"/><circle cx="135" cy="265" r="42" fill="#f2a985"/><circle cx="585" cy="265" r="42" fill="#9ec8b7"/></svg>`,
};
const TRUSTED_SEED_DATA_URLS = new Map([
  ["photos/1-courtyard", seedDataURL(SEED_SVGS.courtyard)],
  ["photos/2-tool-library", seedDataURL(SEED_SVGS.tools)],
  ["photos/3-street-feast", seedDataURL(SEED_SVGS.feast)],
]);
let me = null;
let rows = [];
let ready = false;
let processing = false;
let sharing = false;
let preparedDataURL = "";
let selectionRevision = 0;
const names = new Map();
const inflightProfiles = new Set();
const profileRevisions = new Map();
let sharedDataRevision = 0;

function newID() { if (crypto.randomUUID) return crypto.randomUUID().toLowerCase(); return Array.from(crypto.getRandomValues(new Uint8Array(16)), (byte) => byte.toString(16).padStart(2, "0")).join(""); }
function validIdentity(value) { return Boolean(value && ID_PATTERN.test(value.id || "")); }
function byteLength(value) { return new TextEncoder().encode(value).byteLength; }
function validDataURL(value) { return typeof value === "string" && value.length > 0 && byteLength(value) <= MAX_DATA_URL_BYTES && RASTER_IMAGE_DATA.test(value); }
function trustedSeedDataURL(key, value) { return TRUSTED_SEED_DATA_URLS.get(key) === value; }
function validPhoto(row) { const match = row && typeof row.key === "string" ? row.key.match(PHOTO_KEY) : null; const value = row && row.value; return Boolean(match && value && typeof value === "object" && typeof value.caption === "string" && value.caption.trim() && value.caption.length <= 300 && (validDataURL(value.data_url) || trustedSeedDataURL(row.key, value.data_url)) && Number.isFinite(value.created_at) && value.created_at >= 0 && ID_PATTERN.test(value.author_id || "")); }
function person(id) { if (me && id === me.id) return "You"; const profile = names.get(id); return profile ? `${profile.displayName} · ${profile.tag}` : "A neighbor"; }
function showError(message) { error.textContent = message; error.hidden = false; status.textContent = message; }
function photoStatus(count) { return count > MAX_RENDERED_PHOTOS ? `${count} photos · showing newest ${MAX_RENDERED_PHOTOS}` : count ? `${count} photos` : "No photos yet"; }
function clearError() { if (!ready) return; error.textContent = ""; error.hidden = true; status.textContent = photoStatus(rows.filter(validPhoto).length); }
function resolveProfiles(ids) { [...new Set(ids)].forEach((id) => { if (!ID_PATTERN.test(id || "") || inflightProfiles.has(id) || profileRevisions.get(id) === sharedDataRevision) return; const revision = sharedDataRevision; inflightProfiles.add(id); riot.profile(id).then((profile) => { inflightProfiles.delete(id); if (profile && typeof profile.displayName === "string" && typeof profile.tag === "string") { names.set(id, profile); profileRevisions.set(id, revision); } paint(); }).catch(() => inflightProfiles.delete(id)); }); }
function seedDataURL(svg) { return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`; }

async function ensureSeeded() {
  const existing = await riot.list("photos"); const marker = await riot.get(SEED_MARKER);
  if (existing.length && (!marker || marker.status !== "seeding")) { if (!marker || marker.status !== "ready") await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; }
  if (marker && marker.status !== "seeding") return;
  if (!marker) { await riot.put(SEED_MARKER, { version: 1, status: "seeding" }); if ((await riot.list("photos")).length) { await riot.put(SEED_MARKER, { version: 1, status: "ready" }); return; } }
  const seeds = [
    ["photos/1-courtyard", { caption: "Tables coming together in the courtyard", data_url: TRUSTED_SEED_DATA_URLS.get("photos/1-courtyard"), created_at: 1, author_id: DEMO_IDS.alex }],
    ["photos/2-tool-library", { caption: "The tool library is open", data_url: TRUSTED_SEED_DATA_URLS.get("photos/2-tool-library"), created_at: 2, author_id: DEMO_IDS.sam }],
    ["photos/3-street-feast", { caption: "A long table for the street feast", data_url: TRUSTED_SEED_DATA_URLS.get("photos/3-street-feast"), created_at: 3, author_id: DEMO_IDS.jo }],
  ];
  for (const [key, value] of seeds) if ((await riot.get(key)) === null) await riot.put(key, value);
  await riot.put(SEED_MARKER, { version: 1, status: "ready" });
}

function readFile(file) { return new Promise((resolve, reject) => { const reader = new FileReader(); reader.onload = () => typeof reader.result === "string" ? resolve(reader.result) : reject(new Error("unreadable image")); reader.onerror = () => reject(new Error("unreadable image")); reader.readAsDataURL(file); }); }
function loadImage(source) { return new Promise((resolve, reject) => { const image = new Image(); image.onload = () => resolve(image); image.onerror = () => reject(new Error("invalid image")); image.src = source; }); }
async function prepareImage(file) {
  if (!file || !/^image\/(?:jpeg|png|webp|gif|svg\+xml)$/i.test(file.type || "") || file.size > MAX_SOURCE_BYTES) throw new Error("unsupported image");
  const source = await readFile(file);
  if (!IMAGE_DATA.test(source)) throw new Error("unsupported image");
  const image = await loadImage(source);
  const sourceWidth = image.naturalWidth || image.width; const sourceHeight = image.naturalHeight || image.height;
  if (!sourceWidth || !sourceHeight) throw new Error("invalid image");
  if (!Number.isSafeInteger(sourceWidth) || !Number.isSafeInteger(sourceHeight) || sourceWidth * sourceHeight > MAX_DECODED_PIXELS) throw new Error("too many pixels");
  const scale = Math.min(1, 1280 / Math.max(sourceWidth, sourceHeight));
  const canvas = document.createElement("canvas"); canvas.width = Math.max(1, Math.round(sourceWidth * scale)); canvas.height = Math.max(1, Math.round(sourceHeight * scale));
  const context = canvas.getContext("2d"); if (!context) throw new Error("image processing unavailable");
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  for (const quality of [.82, .72, .62, .52, .42, .32, .22, .12]) { const candidate = canvas.toDataURL("image/jpeg", quality); if (validDataURL(candidate)) return candidate; }
  throw new Error("too large");
}

function paintControls() {
  shareButton.disabled = !ready || processing || sharing || !captionInput.value.trim() || !preparedDataURL;
  captionInput.disabled = !ready || sharing;
  fileInput.disabled = !ready || processing || sharing;
  preview.hidden = !preparedDataURL;
  if (preparedDataURL) previewImage.src = preparedDataURL; else previewImage.removeAttribute("src");
  if (preparedDataURL) previewImage.alt = captionInput.value.trim() ? `Preview: ${captionInput.value.trim()}` : "Selected photo preview";
}

function paintGallery() {
  const valid = rows.filter(validPhoto).sort((left, right) => right.value.created_at - left.value.created_at || right.key.localeCompare(left.key));
  empty.hidden = valid.length > 0;
  const rendered = valid.slice(0, MAX_RENDERED_PHOTOS);
  list.replaceChildren(...rendered.map((row) => { const item = document.createElement("li"); item.className = "photo"; const figure = document.createElement("figure"); figure.style.margin = "0"; const image = document.createElement("img"); image.src = row.value.data_url; image.alt = row.value.caption; const details = document.createElement("figcaption"); const caption = document.createElement("p"); caption.className = "caption"; caption.textContent = row.value.caption; const meta = document.createElement("p"); meta.className = "meta"; meta.textContent = `${person(row.value.author_id)} · ${new Date(row.value.created_at).toLocaleDateString()}`; details.append(caption, meta); figure.append(image, details); item.append(figure); return item; }));
  if (ready && error.hidden) status.textContent = photoStatus(valid.length);
  resolveProfiles(rendered.map((row) => row.value.author_id));
}
function paint() { paintControls(); paintGallery(); }

captionInput.addEventListener("input", paintControls);
fileInput.addEventListener("change", async () => {
  if (!ready || processing || sharing) return;
  const revision = ++selectionRevision; const file = fileInput.files && fileInput.files[0]; preparedDataURL = ""; processing = Boolean(file); clearError(); previewNote.textContent = "Preparing a smaller copy…"; paint();
  if (!file) { processing = false; paint(); return; }
  try { const result = await prepareImage(file); if (revision !== selectionRevision) return; preparedDataURL = result; previewNote.textContent = "Ready to share"; }
  catch (failure) { if (revision !== selectionRevision) return; preparedDataURL = ""; showError(failure && failure.message === "too large" ? "That photo is still too large — choose a smaller one." : failure && failure.message === "too many pixels" ? "That image has too many pixels — choose a smaller one." : "That image couldn't be prepared. Choose a JPEG, PNG, WebP, GIF, or SVG."); }
  finally { if (revision === selectionRevision) { processing = false; paint(); if (!preparedDataURL) fileInput.focus(); } }
});
form.addEventListener("submit", async (event) => {
  event.preventDefault(); const caption = captionInput.value.trim(); const draft = captionInput.value; const dataURL = preparedDataURL;
  if (!ready || !caption || !validDataURL(dataURL)) { if (!ready) showError("Wait for your identity before sharing."); return; }
  const createdAt = Date.now(); const key = `photos/${createdAt}-${newID()}`; const photo = { caption, data_url: dataURL, created_at: createdAt, author_id: me.id }; let failed = false; sharing = true; clearError(); paint();
  try { await riot.put(key, photo); if (!rows.some((row) => row.key === key)) rows = [...rows, { key, value: photo }]; form.reset(); preparedDataURL = ""; selectionRevision += 1; clearError(); paint(); captionInput.focus(); }
  catch { failed = true; captionInput.value = draft; preparedDataURL = dataURL; showError("Couldn't share that photo. Your caption and preview are safe; try again."); }
  finally { sharing = false; paint(); if (failed) captionInput.focus(); }
});

async function init() {
  riot.watch("photos", (next) => { rows = Array.isArray(next) ? next : []; sharedDataRevision += 1; paint(); });
  try { const identity = await riot.whoami(); if (!validIdentity(identity)) throw new Error("invalid identity"); me = identity; await ensureSeeded(); ready = true; paint(); }
  catch { ready = false; paint(); showError("Your identity couldn't be verified. Photo Wall remains read-only."); }
}
init();
