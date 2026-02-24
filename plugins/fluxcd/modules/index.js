import { _ResourceBuilder } from "husako/_base";
import { name as _name } from "husako";

// Re-export source types for convenience
export { GitRepository, HelmRepository, OCIRepository } from "fluxcd/source";

// --- HelmRelease (helm.toolkit.fluxcd.io/v2) ---

class _HelmRelease extends _ResourceBuilder {
  constructor() { super("helm.toolkit.fluxcd.io/v2", "HelmRelease"); }
  chart(n, version) { return this._setDeep("chart.spec", { chart: n, version: String(version) }); }
  sourceRef(ref) {
    const resolved = ref && typeof ref._sourceRef === "function" ? ref._sourceRef() : ref;
    return this._setDeep("chart.spec.sourceRef", resolved);
  }
  interval(v) { return this._setSpec("interval", v); }
  values(v) {
    const resolved = v && typeof v._toJSON === "function" ? v._toJSON() : v;
    return this._setSpec("values", resolved);
  }
  valuesFrom(v) { return this._setSpec("valuesFrom", v); }
  dependsOn(v) { return this._setSpec("dependsOn", v); }
}
export function HelmRelease(n) {
  const r = new _HelmRelease();
  return n ? r.metadata(_name(n)) : r;
}

// --- Kustomization (kustomize.toolkit.fluxcd.io/v1) ---

class _Kustomization extends _ResourceBuilder {
  constructor() { super("kustomize.toolkit.fluxcd.io/v1", "Kustomization"); }
  sourceRef(ref) {
    const resolved = ref && typeof ref._sourceRef === "function" ? ref._sourceRef() : ref;
    return this._setSpec("sourceRef", resolved);
  }
  path(v) { return this._setSpec("path", v); }
  interval(v) { return this._setSpec("interval", v); }
  prune(v) { return this._setSpec("prune", v); }
  targetNamespace(v) { return this._setSpec("targetNamespace", v); }
  dependsOn(v) { return this._setSpec("dependsOn", v); }
}
export function Kustomization(n) {
  const r = new _Kustomization();
  return n ? r.metadata(_name(n)) : r;
}
