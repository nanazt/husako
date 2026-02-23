import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const rustToolchain = getAction("dtolnay/rust-toolchain@stable");
const uploadArtifact = getAction("actions/upload-artifact@v4");
const downloadArtifact = getAction("actions/download-artifact@v4");
const ghRelease = getAction("softprops/action-gh-release@v2");
const setupNode = getAction("actions/setup-node@v4");

const build = new Job("${{ matrix.target.runner }}", {
  strategy: {
    "fail-fast": false,
    matrix: {
      target: [
        {
          platform: "linux-x64",
          runner: "ubuntu-latest",
          rust_target: "x86_64-unknown-linux-gnu",
          binary: "husako",
          archive: "tar.gz",
        },
        {
          platform: "linux-arm64",
          runner: "ubuntu-latest",
          rust_target: "aarch64-unknown-linux-gnu",
          binary: "husako",
          archive: "tar.gz",
        },
        {
          platform: "darwin-x64",
          runner: "macos-latest",
          rust_target: "x86_64-apple-darwin",
          binary: "husako",
          archive: "tar.gz",
        },
        {
          platform: "darwin-arm64",
          runner: "macos-latest",
          rust_target: "aarch64-apple-darwin",
          binary: "husako",
          archive: "tar.gz",
        },
        {
          platform: "win32-x64",
          runner: "windows-latest",
          rust_target: "x86_64-pc-windows-msvc",
          binary: "husako.exe",
          archive: "zip",
        },
      ],
    },
  },
}).steps((s) =>
  s
    .add(checkout({}))
    .add(
      rustToolchain({
        with: { targets: "${{ matrix.target.rust_target }}" },
      }),
    )
    .add({
      name: "Install cross-compilation tools",
      if: "matrix.target.rust_target == 'aarch64-unknown-linux-gnu'",
      run: "sudo apt-get update && sudo apt-get install -y gcc-aarch64-linux-gnu",
    })
    .add({
      name: "Build",
      run: "cargo build --release --target ${{ matrix.target.rust_target }}",
      env: {
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER:
          "aarch64-linux-gnu-gcc",
        CC_aarch64_unknown_linux_gnu: "aarch64-linux-gnu-gcc",
      },
    })
    .add({
      name: "Archive (unix)",
      if: "matrix.target.archive == 'tar.gz'",
      run: [
        "cd target/${{ matrix.target.rust_target }}/release",
        "tar czf ../../../husako-${{ matrix.target.platform }}.tar.gz ${{ matrix.target.binary }}",
      ].join("\n"),
    })
    .add({
      name: "Archive (windows)",
      if: "matrix.target.archive == 'zip'",
      shell: "pwsh",
      run: 'Compress-Archive -Path "target/${{ matrix.target.rust_target }}/release/${{ matrix.target.binary }}" -DestinationPath "husako-${{ matrix.target.platform }}.zip"',
    })
    .add(
      uploadArtifact({
        with: {
          name: "binary-${{ matrix.target.platform }}",
          path: "husako-${{ matrix.target.platform }}.*",
        },
      }),
    )
    .add(
      uploadArtifact({
        with: {
          name: "npm-${{ matrix.target.platform }}",
          path: "target/${{ matrix.target.rust_target }}/release/${{ matrix.target.binary }}",
        },
      }),
    ),
);

const githubRelease = new Job("ubuntu-latest", {
  needs: ["build"],
  permissions: { contents: "write" },
}).steps((s) =>
  s
    .add(
      downloadArtifact({
        with: { pattern: "binary-*", path: "artifacts", "merge-multiple": true },
      }),
    )
    .add({
      name: "Generate checksums",
      run: [
        "cd artifacts",
        "sha256sum husako-* > checksums-sha256.txt",
      ].join("\n"),
    })
    .add(
      ghRelease({
        with: {
          files: ["artifacts/husako-*", "artifacts/checksums-sha256.txt"].join(
            "\n",
          ),
        },
      }),
    ),
);

const publishNpm = new Job("ubuntu-latest", {
  needs: ["build"],
  permissions: { "id-token": "write" },
}).steps((s) =>
  s
    .add(checkout({}))
    .add(setupNode({ with: { "node-version": "22", "registry-url": "https://registry.npmjs.org" } }))
    .add(
      downloadArtifact({
        with: { pattern: "npm-*", path: "npm-artifacts", "merge-multiple": false },
      }),
    )
    .add({
      name: "Prepare platform packages",
      run: [
        "for platform in linux-x64 linux-arm64 darwin-x64 darwin-arm64 win32-x64; do",
        '  mkdir -p "npm/platform-${platform}/bin"',
        '  if [ -f "npm-artifacts/npm-${platform}/husako" ]; then',
        '    cp "npm-artifacts/npm-${platform}/husako" "npm/platform-${platform}/bin/"',
        '    chmod +x "npm/platform-${platform}/bin/husako"',
        '  elif [ -f "npm-artifacts/npm-${platform}/husako.exe" ]; then',
        '    cp "npm-artifacts/npm-${platform}/husako.exe" "npm/platform-${platform}/bin/"',
        "  fi",
        "done",
      ].join("\n"),
    })
    .add({
      name: "Sync versions",
      run: "bash scripts/sync-versions.sh",
    })
    .add({
      name: "Copy README",
      run: "cp README.md npm/husako/",
    })
    .add({
      name: "Publish platform packages",
      run: [
        "for dir in npm/platform-*; do",
        '  if [ -d "$dir/bin" ] && [ "$(ls -A "$dir/bin")" ]; then',
        '    cd "$dir"',
        "    npm publish --provenance --access public",
        "    cd ../..",
        "  fi",
        "done",
      ].join("\n"),
      env: { NODE_AUTH_TOKEN: "${{ secrets.NPM_TOKEN }}" },
    })
    .add({
      name: "Publish main package",
      run: "cd npm/husako && npm publish --provenance --access public",
      env: { NODE_AUTH_TOKEN: "${{ secrets.NPM_TOKEN }}" },
    }),
);

new Workflow({
  name: "Distribute",
  on: { push: { tags: ["v*"] } },
  permissions: { contents: "read" },
})
  .jobs((j) =>
    j
      .add("build", build)
      .add("github-release", githubRelease)
      .add("publish-npm", publishNpm),
  )
  .build("distribute");
