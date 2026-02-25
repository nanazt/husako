import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const rustToolchain = getAction("dtolnay/rust-toolchain@stable");
const rustCache = getAction("Swatinem/rust-cache@v2");

const bench = new Job("ubuntu-latest", {
  permissions: { contents: "write" },
}).steps((s) =>
  s
    .add(checkout({}))
    .add(rustToolchain({}))
    .add(rustCache({}))
    .add({
      name: "Cache husako bench downloads",
      uses: "actions/cache@v4",
      with: {
        path: "crates/husako-bench/fixtures/.husako/cache",
        key: "${{ runner.os }}-husako-bench-${{ hashFiles('crates/husako-bench/fixtures/husako.toml') }}",
      },
    })
    .add({
      name: "Build husako binary",
      run: "cargo build --release --bin husako",
    })
    .add({
      name: "Generate types for benchmarks",
      run: "cd crates/husako-bench/fixtures && ../../../target/release/husako gen",
    })
    .add({
      name: "Run benchmarks",
      run: "cargo bench -p husako-bench 2>&1 | tee bench-output.txt",
    })
    .add({
      name: "Upload criterion results",
      uses: "actions/upload-artifact@v4",
      with: { name: "criterion", path: "target/criterion" },
    })
    .add({
      name: "Generate bench report",
      run: "cargo run -p husako-bench --bin report -- --output-dir ./bench-results",
    })
    .add({
      name: "Commit results to bench-results branch",
      run: [
        `git config user.name  "github-actions[bot]"`,
        `git config user.email "github-actions[bot]@users.noreply.github.com"`,
        `git fetch origin bench-results:bench-results 2>/dev/null || true`,
        `# On first run, --orphan stages all workspace files; unstage them immediately.`,
        `git checkout bench-results 2>/dev/null || { git checkout --orphan bench-results && git rm --cached -r . --quiet; }`,
        `mkdir -p latest`,
        `cp bench-results/bench-summary.md latest/`,
        `cp bench-results/bench-report.md  latest/`,
        `git add latest/`,
        `if [[ "$GITHUB_REF" == refs/tags/v* ]]; then`,
        `  version="$GITHUB_REF_NAME"`,
        `  mkdir -p "releases/$version"`,
        `  cp bench-results/bench-summary.md "releases/$version/"`,
        `  cp bench-results/bench-report.md  "releases/$version/"`,
        `  git add "releases/$version/"`,
        `fi`,
        `git commit -m "bench: $(date -u +%Y-%m-%dT%H:%M:%SZ) \${GITHUB_REF_NAME:-\${GITHUB_SHA::7}}"`,
        `git push origin bench-results`,
      ].join("\n"),
    }),
);

new Workflow({
  name: "Bench",
  on: { push: { branches: ["master"], tags: ["v*"] } },
})
  .jobs((j) => j.add("bench", bench))
  .build("bench");
