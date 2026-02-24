import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const setupNode = getAction("actions/setup-node@v4");

const release = new Job("ubuntu-latest", {
  permissions: { contents: "write" },
}).steps((s) =>
  s
    // Checkout master (not the tag commit) so we can push the update
    .add(checkout({ with: { ref: "master" } }))
    .add(
      setupNode({
        with: { "node-version": "22" },
      }),
    )
    .add({
      name: "Update versions.json",
      run: "node docs/scripts/update-versions.mjs ${{ github.ref_name }}",
    })
    .add({
      name: "Commit and push",
      run: [
        'git config user.name "github-actions[bot]"',
        'git config user.email "github-actions[bot]@users.noreply.github.com"',
        "git add docs/versions.json",
        'git commit -m "docs: update versions.json for ${{ github.ref_name }}"',
        "git push",
      ].join("\n"),
    }),
);

new Workflow({
  name: "Docs Release",
  on: {
    push: {
      tags: ["v*"],
    },
  },
})
  .jobs((j) => j.add("update-versions", release))
  .build("docs-release");
