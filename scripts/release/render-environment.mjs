import { join } from "node:path";

// Hermetic fontconfig environment for SVG text rendering through sharp's
// bundled libvips (librsvg resolves fonts via fontconfig only; @font-face is
// unsupported). This is the only module that shapes the render process
// environment; env mutation happens in the CLI before the render module is
// dynamically imported, and tests assert through spawned processes.

export const RENDER_ENVIRONMENT = Object.freeze({
  FONTCONFIG_PATH: "FONTCONFIG_PATH",
  XDG_CACHE_HOME: "XDG_CACHE_HOME",
  PANGOCAIRO_BACKEND: "PANGOCAIRO_BACKEND",
});

export async function buildFontsConf({ fontsDirectory, workDirectory, fs }) {
  if (typeof fs?.writeFile !== "function" || typeof fs?.mkdir !== "function") {
    throw new TypeError("an fs adapter with mkdir/writeFile is required");
  }
  const confDirectory = join(workDirectory, "fontconfig");
  const cacheDirectory = join(workDirectory, "fontconfig-cache");
  await fs.mkdir(confDirectory, { recursive: true });
  await fs.mkdir(cacheDirectory, { recursive: true });
  const confPath = join(confDirectory, "fonts.conf");
  const conf = `<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig>
  <dir>${fontsDirectory}</dir>
  <cachedir>${cacheDirectory}</cachedir>
</fontconfig>
`;
  await fs.writeFile(confPath, conf);
  return Object.freeze({
    confPath,
    confDirectory,
    env: Object.freeze({
      [RENDER_ENVIRONMENT.FONTCONFIG_PATH]: confDirectory,
      [RENDER_ENVIRONMENT.XDG_CACHE_HOME]: cacheDirectory,
      [RENDER_ENVIRONMENT.PANGOCAIRO_BACKEND]: "fontconfig",
    }),
  });
}
