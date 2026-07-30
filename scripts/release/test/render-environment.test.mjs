import { test } from "node:test";
import assert from "node:assert/strict";
import * as realFs from "node:fs/promises";
import { join } from "node:path";
import { spawn } from "node:child_process";

import { tempReleaseRoot } from "./helpers.mjs";
import { buildFontsConf, RENDER_ENVIRONMENT } from "../render-environment.mjs";

const repositoryRoot = new URL("../../..", import.meta.url).pathname;

test("buildFontsConf writes a hermetic fontconfig pointing at the checked-in fonts", async () => {
  const root = await tempReleaseRoot();
  const fontsDirectory = join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts");
  const result = await buildFontsConf({ fontsDirectory, workDirectory: root, fs: realFs });
  const conf = await realFs.readFile(result.confPath, "utf8");
  assert.match(conf, /<dir>.*Resources\/Fonts<\/dir>/);
  assert.match(conf, /<\?xml/);
  assert.deepEqual(Object.keys(result.env).sort(), [...Object.keys(RENDER_ENVIRONMENT)].sort());
  assert.equal(result.env.FONTCONFIG_PATH, result.confDirectory);
});

test("render environment variables reach a spawned process before sharp loads", async () => {
  const root = await tempReleaseRoot();
  const fontsDirectory = join(repositoryRoot, "apps", "ios", "Riot", "Resources", "Fonts");
  const { env } = await buildFontsConf({ fontsDirectory, workDirectory: root, fs: realFs });
  const child = spawn(
    process.execPath,
    ["-e", "console.log(JSON.stringify({fc: process.env.FONTCONFIG_PATH, xdg: process.env.XDG_CACHE_HOME}))"],
    { env: { ...process.env, ...env }, stdio: ["ignore", "pipe", "pipe"] },
  );
  let stdout = "";
  child.stdout.on("data", (chunk) => { stdout += chunk; });
  const code = await new Promise((resolve) => child.on("close", resolve));
  assert.equal(code, 0);
  const parsed = JSON.parse(stdout);
  assert.equal(parsed.fc, env.FONTCONFIG_PATH);
  assert.equal(parsed.xdg, env.XDG_CACHE_HOME);
});

test("buildFontsConf requires an fs adapter", async () => {
  await assert.rejects(
    buildFontsConf({ fontsDirectory: "x", workDirectory: "y" }),
    /fs adapter/,
  );
});

test("applyRenderEnvironment restores pre-existing values on cleanup", async () => {
  const { applyRenderEnvironment } = await import("../render-environment.mjs");
  process.env.FONTCONFIG_PATH = "/preexisting";
  const restore = applyRenderEnvironment({ FONTCONFIG_PATH: "/render" });
  assert.equal(process.env.FONTCONFIG_PATH, "/render");
  restore();
  assert.equal(process.env.FONTCONFIG_PATH, "/preexisting");
  delete process.env.FONTCONFIG_PATH;
});
