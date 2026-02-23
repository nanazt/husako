import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");

const sync = new Job("ubuntu-latest").steps((s) =>
  s
    .add(
      checkout({
        with: { token: "${{ secrets.PAT }}" },
      }),
    )
    .add({
      name: "Install gaji",
      run: "npm install -g gaji",
    })
    .add({ name: "Generate types", run: "gaji dev" })
    .add({ name: "Build workflows", run: "gaji build" })
    .add({
      name: "Commit and push if changed",
      run: [
        'git config user.name "github-actions[bot]"',
        'git config user.email "github-actions[bot]@users.noreply.github.com"',
        "git add .github/workflows/",
        'git diff --staged --quiet || (git commit -m "chore: sync workflow YAML" && git push)',
      ].join("\n"),
    }),
);

new Workflow({
  name: "Sync Workflows",
  on: {
    push: {
      branches: ["master"],
      paths: ["workflows/**"],
    },
  },
})
  .jobs((j) => j.add("sync", sync))
  .build("sync-workflows");
