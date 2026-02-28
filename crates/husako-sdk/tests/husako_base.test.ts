import { describe, test, expect } from "husako/test";
import { _ResourceBuilder, _SchemaBuilder, _createSpecFragment, _createResourceChain, _createResourceRequirementsChain } from "husako/_base";

describe("_createSpecFragment", () => {
  test("name populates _toMetadata()", () => {
    const f = _createSpecFragment({ _name: "app" });
    expect(f._toMetadata().name).toBe("app");
  });
  test("name() chain method", () => {
    const f = _createSpecFragment({}).name("app");
    expect(f._toMetadata().name).toBe("app");
  });
  test("namespace() chain method", () => {
    const f = _createSpecFragment({}).namespace("default");
    expect(f._toMetadata().namespace).toBe("default");
  });
  test("label() accumulates", () => {
    const f = _createSpecFragment({}).label("a", "1").label("b", "2");
    expect(f._toMetadata().labels).toEqual({ a: "1", b: "2" });
  });
  test("annotation() accumulates", () => {
    const f = _createSpecFragment({}).annotation("k", "v");
    expect(f._toMetadata().annotations).toEqual({ k: "v" });
  });
  test("image() in _toContainer()", () => {
    const f = _createSpecFragment({}).name("c").image("nginx:1.25");
    expect(f._toContainer().name).toBe("c");
    expect(f._toContainer().image).toBe("nginx:1.25");
  });
  test("imagePullPolicy() in _toContainer()", () => {
    const f = _createSpecFragment({}).image("nginx").imagePullPolicy("Always");
    expect(f._toContainer().imagePullPolicy).toBe("Always");
  });
  test("resources() resolves ResourceRequirementsChain in _toContainer()", () => {
    const list = _createResourceChain({}).cpu("100m").memory("128Mi");
    const req = _createResourceRequirementsChain(list._toJSON());
    const f = _createSpecFragment({}).name("c").resources(req);
    expect(f._toContainer().resources).toEqual({ requests: { cpu: "100m", memory: "128Mi" } });
  });
  test("_toMetadata() omits undefined fields", () => {
    const f = _createSpecFragment({ _name: "x" });
    const meta = f._toMetadata();
    expect(Object.keys(meta)).toEqual(["name"]);
  });
  test("_toContainer() omits undefined fields", () => {
    const f = _createSpecFragment({}).name("c").image("nginx");
    const container = f._toContainer();
    expect(Object.keys(container).sort()).toEqual(["image", "name"]);
  });
});

describe("_createResourceChain", () => {
  test("cpu(string) passthrough", () => {
    expect(_createResourceChain({}).cpu("250m")._toJSON()).toEqual({ cpu: "250m" });
  });
  test("cpu(int) -> string", () => {
    expect(_createResourceChain({}).cpu(1)._toJSON()).toEqual({ cpu: "1" });
  });
  test("cpu(float) -> millicores", () => {
    expect(_createResourceChain({}).cpu(0.5)._toJSON()).toEqual({ cpu: "500m" });
  });
  test("memory(string) passthrough", () => {
    expect(_createResourceChain({}).memory("512Mi")._toJSON()).toEqual({ memory: "512Mi" });
  });
  test("memory(number) -> Gi", () => {
    expect(_createResourceChain({}).memory(2)._toJSON()).toEqual({ memory: "2Gi" });
  });
  test("cpu+memory combined", () => {
    const r = _createResourceChain({}).cpu("100m").memory("128Mi");
    expect(r._toJSON()).toEqual({ cpu: "100m", memory: "128Mi" });
  });
  test("empty chain returns empty object", () => {
    expect(_createResourceChain({})._toJSON()).toEqual({});
  });
});

describe("_createResourceRequirementsChain", () => {
  test("requests only", () => {
    const list = _createResourceChain({}).cpu("250m").memory("128Mi");
    const req = _createResourceRequirementsChain(list._toJSON());
    expect(req._toJSON()).toEqual({ requests: { cpu: "250m", memory: "128Mi" } });
  });
  test("requests + limits", () => {
    const req = _createResourceRequirementsChain(
      _createResourceChain({}).cpu("250m").memory("128Mi")._toJSON()
    ).limits(_createResourceChain({}).cpu("500m").memory("256Mi"));
    expect(req._toJSON()).toEqual({
      requests: { cpu: "250m", memory: "128Mi" },
      limits: { cpu: "500m", memory: "256Mi" },
    });
  });
  test("empty requests omitted", () => {
    const req = _createResourceRequirementsChain({});
    expect(req._toJSON()).toEqual({});
  });
});

describe("_ResourceBuilder", () => {
  test("apiVersion and kind in render", () => {
    const obj = new _ResourceBuilder("v1", "ConfigMap")._render();
    expect(obj.apiVersion).toBe("v1");
    expect(obj.kind).toBe("ConfigMap");
  });
  test("metadata() accepts SpecFragment", () => {
    const frag = _createSpecFragment({ _name: "cm", _namespace: "default" });
    const obj = new _ResourceBuilder("v1", "ConfigMap")
      .metadata(frag)._render();
    expect(obj.metadata.name).toBe("cm");
    expect(obj.metadata.namespace).toBe("default");
  });
  test("metadata() with chained SpecFragment", () => {
    const frag = _createSpecFragment({})
      .name("app")
      .namespace("prod")
      .label("tier", "web");
    const obj = new _ResourceBuilder("apps/v1", "Deployment")
      .metadata(frag)._render();
    expect(obj.metadata.name).toBe("app");
    expect(obj.metadata.namespace).toBe("prod");
    expect(obj.metadata.labels).toEqual({ tier: "web" });
  });
  test("metadata() throws for non-SpecFragment", () => {
    expect(() =>
      new _ResourceBuilder("v1", "ConfigMap").metadata({ _toJSON() { return {}; } } as any)
    ).toThrow("metadata()");
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
