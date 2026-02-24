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

const e2e = new Job("ubuntu-latest").steps((s) =>
  s
    .add(checkout({}))
    .add(rustToolchain({}))
    .add(rustCache({}))
    .add({
      name: "Cache husako downloads (Scenario A)",
      uses: "actions/cache@v4",
      with: {
        path: "test/e2e/.husako/cache",
        key: "${{ runner.os }}-husako-e2e-${{ hashFiles('test/e2e/husako.toml') }}",
      },
    })
    .add({
      name: "Install kubeconform",
      run: [
        'VER=$(curl -sL "https://api.github.com/repos/yannh/kubeconform/releases/latest" | grep \'"tag_name"\' | cut -d\'"\' -f4)',
        'curl -sL "https://github.com/yannh/kubeconform/releases/download/${VER}/kubeconform-linux-amd64.tar.gz" | tar xz',
        "chmod +x kubeconform && sudo mv kubeconform /usr/local/bin/kubeconform",
      ].join("\n"),
    })
    .add({
      name: "Build husako binary",
      run: "cargo build --release --bin husako",
    })
    .add({
      name: "E2E tests",
      run: "bash scripts/e2e.sh",
    }),
);

new Workflow({
  name: "Check",
  on: {
    push: { branches: ["master"] },
    pull_request: { branches: ["master"] },
  },
})
  .jobs((j) => j.add("lint", lint).add("test", test).add("e2e", e2e))
  .build("check");
