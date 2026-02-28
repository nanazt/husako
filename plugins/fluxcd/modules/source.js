import { _ResourceBuilder, _createSpecFragment } from "husako/_base";

// --- GitRepository (source.toolkit.fluxcd.io/v1) ---

class _GitRepository extends _ResourceBuilder {
  constructor() { super("source.toolkit.fluxcd.io/v1", "GitRepository"); }
  url(v) { return this._setSpec("url", v); }
  ref(v) { return this._setSpec("ref", v); }
  interval(v) { return this._setSpec("interval", v); }
  secretRef(n) { return this._setSpec("secretRef", { name: n }); }
  _sourceRef() {
    const m = this._metadata || {};
    return { kind: "GitRepository", name: m.name, namespace: m.namespace };
  }
}
export function GitRepository(n) {
  const r = new _GitRepository();
  return n ? r.metadata(_createSpecFragment({ _name: n })) : r;
}

// --- HelmRepository (source.toolkit.fluxcd.io/v1) ---

class _HelmRepository extends _ResourceBuilder {
  constructor() { super("source.toolkit.fluxcd.io/v1", "HelmRepository"); }
  url(v) { return this._setSpec("url", v); }
  type(v) { return this._setSpec("type", v); }
  interval(v) { return this._setSpec("interval", v); }
  secretRef(n) { return this._setSpec("secretRef", { name: n }); }
  _sourceRef() {
    const m = this._metadata || {};
    return { kind: "HelmRepository", name: m.name, namespace: m.namespace };
  }
}
export function HelmRepository(n) {
  const r = new _HelmRepository();
  return n ? r.metadata(_createSpecFragment({ _name: n })) : r;
}

// --- OCIRepository (source.toolkit.fluxcd.io/v1beta2) ---

class _OCIRepository extends _ResourceBuilder {
  constructor() { super("source.toolkit.fluxcd.io/v1beta2", "OCIRepository"); }
  url(v) { return this._setSpec("url", v); }
  ref(v) { return this._setSpec("ref", v); }
  interval(v) { return this._setSpec("interval", v); }
  secretRef(n) { return this._setSpec("secretRef", { name: n }); }
  _sourceRef() {
    const m = this._metadata || {};
    return { kind: "OCIRepository", name: m.name, namespace: m.namespace };
  }
}
export function OCIRepository(n) {
  const r = new _OCIRepository();
  return n ? r.metadata(_createSpecFragment({ _name: n })) : r;
}
