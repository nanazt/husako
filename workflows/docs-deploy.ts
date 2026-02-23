import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const setupNode = getAction("actions/setup-node@v4");
const ghPages = getAction("peaceiris/actions-gh-pages@v4");

const deploy = new Job("ubuntu-latest", {
  permissions: { contents: "write" },
}).steps((s) =>
  s
    .add(checkout({ with: { "fetch-depth": 0 } }))
    .add(
      setupNode({
        with: {
          "node-version": "22",
          cache: "npm",
          "cache-dependency-path": "docs/package-lock.json",
        },
      }),
    )
    .add({ name: "Install", run: "npm ci", "working-directory": "docs" })
    // Build for latest (/husako/)
    .add({
      name: "Build (latest)",
      run: "npm run build",
      "working-directory": "docs",
    })
    // Deploy latest to root of gh-pages, keeping existing versioned subdirectories
    .add(
      ghPages({
        name: "Deploy latest",
        with: {
          github_token: "${{ secrets.GITHUB_TOKEN }}",
          publish_dir: "docs/.vitepress/dist",
          keep_files: true,
          destination_dir: ".",
          commit_message: "docs: deploy latest (master)",
        },
      }),
    )
    // On v* tag: rebuild with versioned base (/husako/vX.Y.Z/) and archive
    .add({
      name: "Build (versioned)",
      "if": "${{ startsWith(github.ref, 'refs/tags/v') }}",
      run: "npm run build",
      "working-directory": "docs",
      env: { VITEPRESS_BASE: "/husako/${{ github.ref_name }}/" },
    })
    .add(
      ghPages({
        name: "Archive version",
        "if": "${{ startsWith(github.ref, 'refs/tags/v') }}",
        with: {
          github_token: "${{ secrets.GITHUB_TOKEN }}",
          publish_dir: "docs/.vitepress/dist",
          keep_files: true,
          destination_dir: "${{ github.ref_name }}",
          commit_message: "docs: archive ${{ github.ref_name }}",
        },
      }),
    ),
);

new Workflow({
  name: "Deploy Docs",
  on: {
    push: {
      branches: ["master"],
      paths: ["docs/**"],
      tags: ["v*"],
    },
    workflow_dispatch: {},
  },
  concurrency: {
    group: "github-pages",
    "cancel-in-progress": false,
  },
})
  .jobs((j) => j.add("deploy", deploy))
  .build("docs-deploy");
