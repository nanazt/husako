const fs = require("fs");
const path = require("path");

const PLATFORMS = {
  "linux-x64": { pkg: "@husako/linux-x64", bin: "husako" },
  "linux-arm64": { pkg: "@husako/linux-arm64", bin: "husako" },
  "darwin-x64": { pkg: "@husako/darwin-x64", bin: "husako" },
  "darwin-arm64": { pkg: "@husako/darwin-arm64", bin: "husako" },
  "win32-x64": { pkg: "@husako/win32-x64", bin: "husako.exe" },
};

function getPlatformKey() {
  const platformMap = { linux: "linux", darwin: "darwin", win32: "win32" };
  const archMap = { x64: "x64", arm64: "arm64" };
  const p = platformMap[process.platform];
  const a = archMap[process.arch];
  return p && a ? `${p}-${a}` : null;
}

const key = getPlatformKey();

if (!key) {
  console.warn(
    `husako: unsupported platform ${process.platform}-${process.arch}`,
  );
  process.exit(0);
}

const info = PLATFORMS[key];

if (!info) {
  console.warn(`husako: no binary package for ${key}`);
  process.exit(0);
}

try {
  const pkgDir = path.dirname(require.resolve(`${info.pkg}/package.json`));
  const binPath = path.join(pkgDir, "bin", info.bin);

  if (!fs.existsSync(binPath)) {
    console.warn(
      `husako: binary not found at ${binPath}\n` +
        `Try reinstalling: npm install -g husako`,
    );
  }
} catch (_) {
  console.warn(
    `husako: platform package ${info.pkg} not installed.\n` +
      `This may happen if your platform is not supported or npm failed to install optional dependencies.\n` +
      `Try reinstalling: npm install -g husako`,
  );
}
