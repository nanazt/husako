import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const setupNode = getAction("actions/setup-node@v4");

const deploy = new Job("ubuntu-latest", {
  permissions: { contents: "read", pages: "write", "id-token": "write" },
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
    .add({ name: "Build", run: "npm run build", "working-directory": "docs" })
    .add({ uses: "actions/configure-pages@v5" })
    .add({
      uses: "actions/upload-pages-artifact@v3",
      with: { path: "docs/.vitepress/dist" },
    })
    .add({
      name: "Deploy to GitHub Pages",
      id: "deployment",
      uses: "actions/deploy-pages@v4",
    }),
);

new Workflow({
  name: "Deploy Docs",
  on: {
    push: { branches: ["master"], paths: ["docs/**"] },
    workflow_dispatch: {},
  },
  concurrency: {
    group: "github-pages",
    "cancel-in-progress": false,
  },
})
  .jobs((j) => j.add("deploy", deploy))
  .build("docs-deploy");
