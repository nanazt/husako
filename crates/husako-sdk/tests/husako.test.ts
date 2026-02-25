import { describe, test, expect } from "husako/test";
import { metadata, name, namespace, label, annotation,
         cpu, memory, requests, limits, merge, build } from "husako";

describe("metadata fragment", () => {
  test("name()", () => {
    expect(metadata().name("app")._toJSON().name).toBe("app");
  });
  test("namespace()", () => {
    expect(metadata().namespace("ns")._toJSON().namespace).toBe("ns");
  });
  test("label() accumulates", () => {
    const m = metadata().label("env", "prod").label("tier", "web");
    expect(m._toJSON().labels).toEqual({ env: "prod", tier: "web" });
  });
  test("annotation()", () => {
    expect(metadata().annotation("k", "v")._toJSON().annotations).toEqual({ k: "v" });
  });
  test("copy-on-write: original unchanged", () => {
    const orig = metadata().name("orig");
    orig.name("mutated");
    expect(orig._toJSON().name).toBe("orig");
  });
  test("shorthand name()", () => { expect(name("x")._toJSON().name).toBe("x"); });
  test("shorthand namespace()", () => { expect(namespace("ns")._toJSON().namespace).toBe("ns"); });
  test("shorthand label()", () => { expect(label("k","v")._toJSON().labels).toEqual({ k: "v" }); });
  test("shorthand annotation()", () => { expect(annotation("k","v")._toJSON().annotations).toEqual({ k: "v" }); });
});

describe("quantity normalization", () => {
  test("cpu int -> string", () => { expect(cpu(1)._toJSON().cpu).toBe("1"); });
  test("cpu float -> millicores", () => { expect(cpu(0.5)._toJSON().cpu).toBe("500m"); });
  test("cpu string passthrough", () => { expect(cpu("250m")._toJSON().cpu).toBe("250m"); });
  test("memory int -> Gi suffix", () => { expect(memory(2)._toJSON().memory).toBe("2Gi"); });
  test("memory string passthrough", () => { expect(memory("512Mi")._toJSON().memory).toBe("512Mi"); });
});

describe("requests / limits", () => {
  test("requests wraps resource list with cpu+memory", () => {
    const r = requests(cpu("100m").memory("128Mi"));
    expect(r._toJSON().requests.cpu).toBe("100m");
    expect(r._toJSON().requests.memory).toBe("128Mi");
  });
  test("limits wraps resource list", () => {
    expect(limits(cpu("200m"))._toJSON().limits.cpu).toBe("200m");
  });
});

describe("build()", () => {
  // build() calls __husako_build which is not set in test mode.
  // We can only test the TypeError path that fires before __husako_build is reached.
  test("throws TypeError for non-builder input", () => {
    expect(() => build([{ not: "a builder" }])).toThrow("build(): item at index 0");
  });
});

describe("merge()", () => {
  test("metadata: last-wins for name", () => {
    const m = merge([metadata().name("old"), metadata().name("new")]);
    expect(m._toJSON().name).toBe("new");
  });
  test("metadata: deep-merge for labels", () => {
    const m = merge([metadata().label("a", "1"), metadata().label("b", "2")]);
    expect(m._toJSON().labels).toEqual({ a: "1", b: "2" });
  });
  test("resource_list: last-wins for cpu", () => {
    const r = merge([cpu("100m"), cpu("200m")]);
    expect(r._toJSON().cpu).toBe("200m");
  });
  test("non-fragment array: returns last item", () => {
    expect(merge([{ x: 1 }, { x: 2 }])).toEqual({ x: 2 });
  });
});
