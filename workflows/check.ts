import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const rustToolchain = getAction("dtolnay/rust-toolchain@stable");
const rustCache = getAction("Swatinem/rust-cache@v2");

const lint = new Job("ubuntu-latest").steps((s) =>
  s
    .add(checkout({}))
    .add(rustToolchain({ with: { components: "clippy,rustfmt" } }))
    .add(rustCache({}))
    .add({ name: "Format check", run: "cargo fmt --all --check" })
    .add({
      name: "Clippy",
      run: "cargo clippy --workspace --all-targets --all-features -- -D warnings",
    }),
);

const test = new Job("ubuntu-latest").steps((s) =>
  s
    .add(checkout({}))
    .add(rustToolchain({}))
    .add(rustCache({}))
    .add({
      name: "Run tests",
      run: "cargo test --workspace --all-features",
    }),
);

new Workflow({
  name: "Check",
  on: {
    push: { branches: ["master"] },
    pull_request: { branches: ["master"] },
  },
})
  .jobs((j) => j.add("lint", lint).add("test", test))
  .build("check");
