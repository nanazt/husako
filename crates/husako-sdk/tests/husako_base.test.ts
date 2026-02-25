import { describe, test, expect } from "husako/test";
import { _ResourceBuilder, _SchemaBuilder } from "husako/_base";
import { metadata, requests, limits, cpu, memory } from "husako";

describe("_ResourceBuilder", () => {
  test("apiVersion and kind in render", () => {
    const obj = new _ResourceBuilder("v1", "ConfigMap")._render();
    expect(obj.apiVersion).toBe("v1");
    expect(obj.kind).toBe("ConfigMap");
  });
  test("metadata() sets metadata", () => {
    const obj = new _ResourceBuilder("v1", "ConfigMap")
      .metadata(metadata().name("cm").namespace("default"))._render();
    expect(obj.metadata.name).toBe("cm");
    expect(obj.metadata.namespace).toBe("default");
  });
  test("spec() sets entire spec", () => {
    expect(new _ResourceBuilder("apps/v1", "Deployment").spec({ replicas: 3 })._render().spec.replicas).toBe(3);
  });
  test("set() adds top-level field", () => {
    expect(new _ResourceBuilder("v1", "ConfigMap").set("data", { key: "val" })._render().data).toEqual({ key: "val" });
  });
  test("_setSpec() adds spec property", () => {
    expect(new _ResourceBuilder("apps/v1", "Deployment")._setSpec("replicas", 2)._render().spec.replicas).toBe(2);
  });
  test("multiple _setSpec() calls merge", () => {
    const obj = new _ResourceBuilder("apps/v1", "Deployment")
      ._setSpec("replicas", 1)._setSpec("paused", true)._render();
    expect(obj.spec.replicas).toBe(1);
    expect(obj.spec.paused).toBe(true);
  });
  test("_setDeep() sets nested spec path", () => {
    const obj = new _ResourceBuilder("apps/v1", "Deployment")
      ._setDeep("template.spec.containers", [{ name: "app" }])._render();
    expect(obj.spec.template.spec.containers[0].name).toBe("app");
  });
  test("_setDeep() merges with existing _setSpec()", () => {
    const obj = new _ResourceBuilder("apps/v1", "Deployment")
      ._setSpec("replicas", 3)
      ._setDeep("template.spec.serviceAccountName", "sa")._render();
    expect(obj.spec.replicas).toBe(3);
    expect(obj.spec.template.spec.serviceAccountName).toBe("sa");
  });
  test("resources() populates spec.template.spec.containers[0].resources", () => {
    const obj = new _ResourceBuilder("apps/v1", "Deployment")
      .resources(requests(cpu("100m").memory("128Mi")), limits(cpu("500m")))._render();
    expect(obj.spec.template.spec.containers[0].resources.requests.cpu).toBe("100m");
    expect(obj.spec.template.spec.containers[0].resources.requests.memory).toBe("128Mi");
    expect(obj.spec.template.spec.containers[0].resources.limits.cpu).toBe("500m");
  });
});

describe("_SchemaBuilder", () => {
  test("_set() returns new instance (copy-on-write)", () => {
    const b = new _SchemaBuilder();
    const b2 = b._set("foo", "bar");
    expect(b._toJSON()).not.toEqual(b2._toJSON());
  });
  test("_toJSON() returns accumulated props", () => {
    expect(new _SchemaBuilder()._set("x", 1)._set("y", 2)._toJSON()).toEqual({ x: 1, y: 2 });
  });
});
