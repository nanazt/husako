#!/usr/bin/env node

const { execFileSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const PLATFORMS = {
  "linux-x64": { pkg: "@husako/linux-x64", bin: "husako" },
  "linux-arm64": { pkg: "@husako/linux-arm64", bin: "husako" },
  "darwin-x64": { pkg: "@husako/darwin-x64", bin: "husako" },
  "darwin-arm64": { pkg: "@husako/darwin-arm64", bin: "husako" },
  "win32-x64": { pkg: "@husako/win32-x64", bin: "husako.exe" },
};

function getPlatformKey() {
  const platform = process.platform;
  const arch = process.arch;

  const platformMap = { linux: "linux", darwin: "darwin", win32: "win32" };
  const archMap = { x64: "x64", arm64: "arm64" };

  const p = platformMap[platform];
  const a = archMap[arch];

  if (!p || !a) {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    process.exit(1);
  }

  return `${p}-${a}`;
}

function findBinary() {
  const key = getPlatformKey();
  const info = PLATFORMS[key];

  if (!info) {
    console.error(`No binary available for ${key}`);
    process.exit(1);
  }

  try {
    const pkgDir = path.dirname(require.resolve(`${info.pkg}/package.json`));
    const binPath = path.join(pkgDir, "bin", info.bin);

    if (fs.existsSync(binPath)) {
      return binPath;
    }
  } catch (_) {
    // Package not installed
  }

  console.error(
    `Could not find husako binary for ${key}.\n` +
      `Try reinstalling: npm install -g husako`,
  );
  process.exit(1);
}

const binary = findBinary();

try {
  const result = execFileSync(binary, process.argv.slice(2), {
    stdio: "inherit",
  });
} catch (e) {
  if (e.status !== null) {
    process.exit(e.status);
  }
  throw e;
}
