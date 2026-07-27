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

function escapeConfXml(text) {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

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
  <dir>${escapeConfXml(fontsDirectory)}</dir>
  <cachedir>${escapeConfXml(cacheDirectory)}</cachedir>
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

// Process-global font state: librsvg reads these variables at first
// rasterization, so callers apply them before loading sharp and restore
// them once rendering completes. Returns a restore closure.
export function applyRenderEnvironment(env) {
  const previous = Object.fromEntries(
    Object.keys(env).map((key) => [key, process.env[key]]),
  );
  for (const [key, value] of Object.entries(env)) {
    process.env[key] = value;
  }
  return () => {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  };
}
