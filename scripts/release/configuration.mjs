const SNAPSHOT_SOURCE = "release/source/configuration-snapshot.json";

function gate(id, state, pointer, observed, expected, recovery) {
  return { id, state, sourceFile: SNAPSHOT_SOURCE, pointer, observed, expected, recovery };
}

function check(id, pointer, ok, observed, expected, recovery) {
  return gate(id, ok ? "PASS" : "BLOCKED", pointer, observed, expected, ok ? "No action required." : recovery);
}

function versionGate(platform, config) {
  const isAndroid = platform === "android";
  const version = isAndroid ? config.versionName : config.marketingVersion;
  const ready = isAndroid
    ? version === "1.0" && Number.isInteger(config.versionCode) && config.versionCode > 0
    : version === "1.0" && config.buildInjection === "explicit";
  const observed = isAndroid
    ? `${version} (versionCode ${config.versionCode})`
    : `${version} (${config.buildInjection} build injection)`;
  return check(
    `config.${platform}.version`,
    isAndroid ? "/android/versionName" : `/${platform}/marketingVersion`,
    ready,
    observed,
    isAndroid
      ? "versionName 1.0 with an explicit positive versionCode"
      : "marketing version 1.0 with explicit (non-commit-count) build injection",
    isAndroid
      ? "Set versionName 1.0 and an explicit positive versionCode in WU-006."
      : "Set marketing version 1.0 with explicit build injection in WU-005.",
  );
}

export function evaluateConfiguration(snapshot) {
  return [
    versionGate("ios", snapshot.ios),
    check(
      "config.ios.bundleId",
      "/ios/bundleId",
      snapshot.ios.bundleId === "net.protest.riot",
      snapshot.ios.bundleId,
      "net.protest.riot",
      "Restore the canonical Apple bundle identifier.",
    ),
    check(
      "config.ios.icon",
      "/ios/icon",
      snapshot.ios.icon === "present",
      snapshot.ios.icon,
      "a 1024px app icon",
      "Provide the iOS app icon set.",
    ),
    check(
      "config.ios.privacyManifest",
      "/ios/privacyManifest",
      snapshot.ios.privacyManifest === true,
      snapshot.ios.privacyManifest,
      "a checked-in PrivacyInfo.xcprivacy",
      "Add the iOS privacy manifest in WU-005.",
    ),
    versionGate("macos", snapshot.macos),
    check(
      "config.macos.bundleId",
      "/macos/bundleId",
      snapshot.macos.bundleId === "net.protest.riot",
      snapshot.macos.bundleId,
      "net.protest.riot",
      "Restore the canonical Apple bundle identifier.",
    ),
    check(
      "config.macos.icon",
      "/macos/icon",
      snapshot.macos.icon === "present",
      snapshot.macos.icon,
      "a complete macOS app-icon set",
      "Add the macOS app-icon asset set in WU-005.",
    ),
    check(
      "config.macos.privacyManifest",
      "/macos/privacyManifest",
      snapshot.macos.privacyManifest === true,
      snapshot.macos.privacyManifest,
      "a checked-in PrivacyInfo.xcprivacy",
      "Add the macOS privacy manifest in WU-005.",
    ),
    check(
      "config.macos.signing",
      "/macos/signing",
      snapshot.macos.signing === "distribution",
      snapshot.macos.signing,
      "distribution signing (never ad-hoc)",
      "Configure macOS distribution signing in WU-005.",
    ),
    check(
      "config.macos.architecture",
      "/macos/architecture",
      snapshot.macos.architecture === "apple-silicon",
      snapshot.macos.architecture,
      "Apple silicon only (no Intel claim)",
      "Remove any Intel architecture claim; Riot ships Apple silicon only.",
    ),
    check(
      "config.android.applicationId",
      "/android/applicationId",
      snapshot.android.applicationId === "net.protest.riot",
      snapshot.android.applicationId,
      "net.protest.riot",
      "Change the public applicationId to net.protest.riot in WU-006 (namespace stays internal).",
    ),
    versionGate("android", snapshot.android),
    check(
      "config.android.icons",
      "/android/icons",
      snapshot.android.icons === "present",
      snapshot.android.icons,
      "adaptive and legacy launcher icons",
      "Add the Android launcher icon tree in WU-006.",
    ),
    check(
      "config.android.signing",
      "/android/signing",
      snapshot.android.signing === "release-external",
      snapshot.android.signing,
      "external upload-key signing with non-argument secrets",
      "Configure external Android upload-key signing in WU-006; debug keys and daemon/argument secrets fail closed.",
    ),
  ];
}

export async function evaluateSnapshotFreshness({ snapshot, repositoryRoot, fs, sha256 }) {
  if (typeof sha256 !== "function") {
    throw new TypeError("a sha256 dependency is required");
  }
  const drifted = [];
  for (const [path, expected] of Object.entries(snapshot.fileHashes)) {
    // Defense in depth: the schema constrains fileHashes keys, but this gate
    // also runs against unchecked snapshots. Never let a snapshot key read
    // outside the repository root — a `../../x` key would hash an arbitrary
    // file and leak its digest into the gate's observed output.
    if (path.split("/").includes("..") || path.startsWith("/") || path.includes("\\")) {
      drifted.push(`${path} (unsafe snapshot path)`);
      continue;
    }
    let observed;
    try {
      observed = sha256(await fs.readFile(`${repositoryRoot}/${path}`));
    } catch {
      observed = "missing";
    }
    if (observed !== expected) drifted.push(`${path} (${observed})`);
  }
  const id = "config.snapshot-freshness";
  if (drifted.length > 0) {
    return gate(
      id,
      "BLOCKED",
      "/fileHashes",
      drifted.join(", "),
      "native files identical to the checked-in snapshot hashes",
      "Native configuration changed out-of-band; regenerate the configuration snapshot and re-evaluate the gates.",
    );
  }
  return gate(
    id,
    "PASS",
    "/fileHashes",
    "fresh",
    "native files identical to the checked-in snapshot hashes",
    "No action required.",
  );
}
