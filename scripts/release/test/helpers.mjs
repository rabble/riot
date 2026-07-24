import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

export async function tempReleaseRoot() {
  return mkdtemp(join(tmpdir(), "riot-release-test-"));
}

export async function readUtf8(path) {
  return readFile(path, "utf8");
}

export const fixedClock = () => new Date("2026-07-24T00:00:00.000Z");
