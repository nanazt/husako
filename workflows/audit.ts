import { getAction, Job, Workflow } from "../generated/index.js";

const checkout = getAction("actions/checkout@v5");

const audit = new Job("ubuntu-latest").steps((s) =>
  s
    .add(checkout({}))
    .add({
      name: "Install cargo-audit",
      run: "cargo install cargo-audit",
    })
    .add({ name: "Run security audit", run: "cargo audit" }),
);

new Workflow({
  name: "Audit",
  on: { schedule: [{ cron: "0 0 * * 1" }] },
})
  .jobs((j) => j.add("audit", audit))
  .build("audit");
