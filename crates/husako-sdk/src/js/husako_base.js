// _ResourceBuilder base class for k8s resource builders.
// Each chainable method returns a NEW instance (copy-on-write).

export class _ResourceBuilder {
  constructor(apiVersion, kind) {
    this._apiVersion = apiVersion;
    this._kind = kind;
    this._metadata = null;
    this._resources = null;
    this._spec = null;
    this._extra = null;
  }

  _copy() {
    const next = new this.constructor(this._apiVersion, this._kind);
    next._metadata = this._metadata;
    next._resources = this._resources;
    next._spec = this._spec;
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
    // Merge all resource requirement fragments
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
      obj.spec = this._spec;
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
        obj[k] = this._extra[k];
      }
    }

    return obj;
  }
}
