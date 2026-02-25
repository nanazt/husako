import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");
const rustToolchain = getAction("dtolnay/rust-toolchain@stable");
const rustCache = getAction("Swatinem/rust-cache@v2");

const bench = new Job("ubuntu-latest").steps((s) =>
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
    }),
);

new Workflow({
  name: "Bench",
  on: { push: { branches: ["master"] } },
})
  .jobs((j) => j.add("bench", bench))
  .build("bench");
