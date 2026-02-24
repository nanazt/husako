import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const setupNode = getAction("actions/setup-node@v4");
const configurePages = getAction("actions/configure-pages@v5");
const uploadPagesArtifact = getAction("actions/upload-pages-artifact@v3");
const deployPages = getAction("actions/deploy-pages@v4");

const deploy = new Job("ubuntu-latest", {
  permissions: {
    contents: "read",
    pages: "write",
    "id-token": "write",
  },
  environment: {
    name: "github-pages",
    url: "${{ steps.deployment.outputs.page_url }}",
  },
}).steps((s) =>
  s
    .add(checkout({}))
    .add(
      setupNode({
        with: {
          "node-version": "22",
          cache: "npm",
          "cache-dependency-path": "docs/package-lock.json",
        },
      }),
    )
    .add(configurePages({ name: "Setup Pages" }))
    .add({ name: "Install", run: "npm ci", "working-directory": "docs" })
    .add({ name: "Build", run: "npm run build", "working-directory": "docs" })
    .add(
      uploadPagesArtifact({
        name: "Upload artifact",
        with: { path: "docs/.vitepress/dist" },
      }),
    )
    .add(
      deployPages({
        id: "deployment",
        name: "Deploy to GitHub Pages",
      }),
    ),
);

new Workflow({
  name: "Deploy Docs",
  on: {
    push: {
      branches: ["master"],
      paths: ["docs/**"],
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
