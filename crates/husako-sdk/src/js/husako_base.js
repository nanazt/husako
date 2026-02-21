// _ResourceBuilder and _SchemaBuilder base classes for k8s resource builders.
// Each chainable method returns a NEW instance (copy-on-write).

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

  metadata(fragment) {
    const next = this._copy();
    next._metadata = fragment;
    return next;
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

  resources(...fragments) {
    const next = this._copy();
    let req = {};
    for (const f of fragments) {
      if (f && f._type === "resource_requirements") {
        if (f._requests) {
          req.requests = f._requests._toJSON();
        }
        if (f._limits) {
          req.limits = f._limits._toJSON();
        }
      }
    }
    next._resources = req;
    return next;
  }

  _render() {
    const obj = {
      apiVersion: this._apiVersion,
      kind: this._kind,
    };

    if (this._metadata) {
      if (this._metadata._type === "metadata") {
        obj.metadata = this._metadata._toJSON();
      } else {
        obj.metadata = this._metadata;
      }
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
