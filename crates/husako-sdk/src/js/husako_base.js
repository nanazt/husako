// husako/_base: base classes for k8s resource builders.
// Used by auto-generated schema modules; not for direct import in user code.

function _resolveFragments(obj) {
  if (obj === null || obj === undefined) return obj;
  if (typeof obj !== "object") return obj;
  if (Array.isArray(obj)) return obj.map(v => _resolveFragments(v));
  if (typeof obj._toJSON === "function") return _resolveFragments(obj._toJSON());
  const result = {};
  for (const key in obj) {
    result[key] = _resolveFragments(obj[key]);
  }
  return result;
}

function _mergeDeep(target, source) {
  if (!target || typeof target !== "object" || Array.isArray(target)) return source;
  if (!source || typeof source !== "object" || Array.isArray(source)) return source;
  const result = Object.assign({}, target);
  for (const key in source) {
    if (typeof source[key] === "object" && source[key] !== null && !Array.isArray(source[key])
        && typeof result[key] === "object" && result[key] !== null && !Array.isArray(result[key])) {
      result[key] = _mergeDeep(result[key], source[key]);
    } else {
      result[key] = source[key];
    }
  }
  return result;
}

// --- Quantity normalization ---

function normalizeCpu(v) {
  if (typeof v === "string") return v;
  if (typeof v === "number") {
    if (Number.isInteger(v)) return String(v);
    const m = Math.round(v * 1000);
    return m + "m";
  }
  return String(v);
}

function normalizeMemory(v) {
  if (typeof v === "string") return v;
  if (typeof v === "number") return v + "Gi";
  return String(v);
}

// --- SpecFragment ---
// Returned by chain starter functions (name(), image(), etc.).
// Mutable accumulator — NOT copy-on-write.
// Compatible with both MetadataChain and ContainerChain contexts.

export function _createSpecFragment(init) {
  const f = {
    _husakoTag: "SpecFragment",
    _name: undefined,
    _namespace: undefined,
    _labels: undefined,
    _annotations: undefined,
    _image: undefined,
    _imagePullPolicy: undefined,
    _resources: undefined,
    _command: undefined,
    _args: undefined,
  };
  if (init) {
    for (const k in init) f[k] = init[k];
  }
  f.name = function(v) { f._name = v; return f; };
  f.namespace = function(v) { f._namespace = v; return f; };
  f.label = function(k, v) {
    if (!f._labels) f._labels = {};
    f._labels[k] = v;
    return f;
  };
  f.annotation = function(k, v) {
    if (!f._annotations) f._annotations = {};
    f._annotations[k] = v;
    return f;
  };
  f.image = function(v) { f._image = v; return f; };
  f.imagePullPolicy = function(v) { f._imagePullPolicy = v; return f; };
  f.resources = function(r) { f._resources = r; return f; };
  f.command = function(v) { f._command = v; return f; };
  f.args = function(v) { f._args = v; return f; };
  f._toMetadata = function() {
    const obj = {};
    if (f._name !== undefined) obj.name = f._name;
    if (f._namespace !== undefined) obj.namespace = f._namespace;
    if (f._labels && Object.keys(f._labels).length > 0) obj.labels = Object.assign({}, f._labels);
    if (f._annotations && Object.keys(f._annotations).length > 0) obj.annotations = Object.assign({}, f._annotations);
    return obj;
  };
  f._toContainer = function() {
    const obj = {};
    if (f._name !== undefined) obj.name = f._name;
    if (f._image !== undefined) obj.image = f._image;
    if (f._imagePullPolicy !== undefined) obj.imagePullPolicy = f._imagePullPolicy;
    if (f._resources !== undefined) obj.resources = _resolveFragments(f._resources);
    if (f._command !== undefined) obj.command = f._command;
    if (f._args !== undefined) obj.args = f._args;
    return obj;
  };
  return f;
}

// --- ResourceChain ---
// Returned by cpu() and memory() chain starters from k8s/core/v1.
// Bare resource list accumulator — no requests/limits wrapper.
// Pass to requests() to create a ResourceRequirementsChain.

export function _createResourceChain(init) {
  const r = {
    _husakoTag: "ResourceChain",
    _cpu: undefined,
    _memory: undefined,
  };
  if (init) {
    for (const k in init) r[k] = init[k];
  }
  r.cpu = function(v) { r._cpu = normalizeCpu(v); return r; };
  r.memory = function(v) { r._memory = normalizeMemory(v); return r; };
  r._toJSON = function() {
    const obj = {};
    if (r._cpu !== undefined) obj.cpu = r._cpu;
    if (r._memory !== undefined) obj.memory = r._memory;
    return obj;  // bare { cpu?, memory? } — no requests wrapper
  };
  return r;
}

// --- ResourceRequirementsChain ---
// Returned by requests() from k8s/core/v1.
// Carries both requests and optional limits. Mutable accumulator.

export function _createResourceRequirementsChain(reqList) {
  const r = {
    _husakoTag: "ResourceRequirementsChain",
    _requests: reqList,
    _limits: undefined,
  };
  r.limits = function(chain) {
    r._limits = chain && typeof chain._toJSON === "function" ? chain._toJSON() : chain;
    return r;
  };
  r._toJSON = function() {
    const obj = {};
    if (r._requests && Object.keys(r._requests).length > 0) obj.requests = r._requests;
    if (r._limits && Object.keys(r._limits).length > 0) obj.limits = r._limits;
    return obj;
  };
  return r;
}

export class _SchemaBuilder {
  constructor(init) {
    this._props = init ? Object.assign({}, init) : {};
  }

  _copy() {
    const n = new this.constructor();
    n._props = Object.assign({}, this._props);
    return n;
  }

  _set(key, value) {
    const n = this._copy();
    n._props[key] = value;
    return n;
  }

  _toJSON() {
    return _resolveFragments(this._props);
  }

  _render() {
    return this._toJSON();
  }
}

export class _ResourceBuilder {
  constructor(apiVersion, kind) {
    this._apiVersion = apiVersion;
    this._kind = kind;
    this._metadata = null;
    this._resources = null;
    this._spec = null;
    this._specParts = null;
    this._extra = null;
  }

  _copy() {
    const next = new this.constructor(this._apiVersion, this._kind);
    next._metadata = this._metadata;
    next._resources = this._resources;
    next._spec = this._spec;
    next._specParts = this._specParts;
    next._extra = this._extra;
    return next;
  }

  metadata(chain) {
    if (chain && chain._husakoTag === "SpecFragment") {
      const next = this._copy();
      next._metadata = chain._toMetadata();
      return next;
    }
    throw new Error(
      "metadata() requires a MetadataChain — use name(), namespace(), label() from \"k8s/meta/v1\"."
    );
  }

  spec(value) {
    const next = this._copy();
    next._spec = value;
    next._specParts = null;
    return next;
  }

  _setSpec(key, value) {
    const next = this._copy();
    next._spec = null;
    next._specParts = Object.assign({}, next._specParts || {});
    next._specParts[key] = value;
    return next;
  }

  _setDeep(path, value) {
    const parts = path.split(".");
    let nested = value;
    for (let i = parts.length - 1; i >= 0; i--) {
      const wrapper = {};
      wrapper[parts[i]] = nested;
      nested = wrapper;
    }
    const next = this._copy();
    next._spec = null;
    next._specParts = _mergeDeep(next._specParts || {}, nested);
    return next;
  }

  set(key, value) {
    const next = this._copy();
    if (!next._extra) next._extra = {};
    next._extra = Object.assign({}, next._extra);
    next._extra[key] = value;
    return next;
  }

  resources(chain) {
    // Kept for backward compatibility. Accepts a ResourceChain, SpecFragment, or plain object.
    const next = this._copy();
    if (chain && typeof chain._toJSON === "function") {
      next._resources = chain._toJSON();
    } else if (chain !== null && chain !== undefined) {
      next._resources = chain;
    }
    return next;
  }

  _render() {
    const obj = {
      apiVersion: this._apiVersion,
      kind: this._kind,
    };

    if (this._metadata) {
      obj.metadata = this._metadata;
    }

    if (this._spec) {
      obj.spec = _resolveFragments(this._spec);
    } else if (this._specParts) {
      obj.spec = _resolveFragments(this._specParts);
      if (this._resources) {
        const containers = obj.spec
          && obj.spec.template
          && obj.spec.template.spec
          && obj.spec.template.spec.containers;
        if (containers && containers.length > 0) {
          containers[0].resources = Object.assign(containers[0].resources || {}, this._resources);
        } else {
          obj.spec = _mergeDeep(obj.spec, {
            template: { spec: { containers: [{ resources: this._resources }] } },
          });
        }
      }
    } else if (this._resources) {
      obj.spec = {
        template: {
          spec: {
            containers: [
              { resources: this._resources },
            ],
          },
        },
      };
    }

    if (this._extra) {
      for (const k in this._extra) {
        obj[k] = _resolveFragments(this._extra[k]);
      }
    }

    return obj;
  }
}
