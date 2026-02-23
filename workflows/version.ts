import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const rustToolchain = getAction("dtolnay/rust-toolchain@stable");
const releasePlz = getAction("release-plz/action@v0.5");

const release = new Job("ubuntu-latest", {
  permissions: { contents: "write", "id-token": "write" },
}).steps((s) =>
  s
    .add(
      checkout({
        with: { "fetch-depth": 0, "persist-credentials": false },
      }),
    )
    .add(rustToolchain({}))
    .add(
      releasePlz({
        with: { command: "release" },
        env: { GITHUB_TOKEN: "${{ secrets.PAT }}" },
      }),
    ),
);

const releasePr = new Job("ubuntu-latest", {
  permissions: { contents: "write", "pull-requests": "write" },
}).steps((s) =>
  s
    .add(
      checkout({
        with: { "fetch-depth": 0, "persist-credentials": false },
      }),
    )
    .add(rustToolchain({}))
    .add(
      releasePlz({
        with: { command: "release-pr" },
        env: { GITHUB_TOKEN: "${{ secrets.PAT }}" },
      }),
    ),
);

new Workflow({
  name: "Version",
  on: { push: { branches: ["master"] } },
  concurrency: {
    group: "release-plz-${{ github.ref }}",
    "cancel-in-progress": false,
  },
})
  .jobs((j) => j.add("release", release).add("release-pr", releasePr))
  .build("version");
