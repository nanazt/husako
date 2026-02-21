// "husako" module: build, merge, fragment builders

// --- MetadataFragment (copy-on-write) ---

const metaMethods = {
  name(v) {
    return createMeta({ _name: v, _namespace: this._namespace, _labels: Object.assign({}, this._labels), _annotations: Object.assign({}, this._annotations) });
  },
  namespace(v) {
    return createMeta({ _name: this._name, _namespace: v, _labels: Object.assign({}, this._labels), _annotations: Object.assign({}, this._annotations) });
  },
  label(k, v) {
    const labels = Object.assign({}, this._labels);
    labels[k] = v;
    return createMeta({ _name: this._name, _namespace: this._namespace, _labels: labels, _annotations: Object.assign({}, this._annotations) });
  },
  annotation(k, v) {
    const annotations = Object.assign({}, this._annotations);
    annotations[k] = v;
    return createMeta({ _name: this._name, _namespace: this._namespace, _labels: Object.assign({}, this._labels), _annotations: annotations });
  },
  _toJSON() {
    const obj = {};
    if (this._name !== null) obj.name = this._name;
    if (this._namespace !== null) obj.namespace = this._namespace;
    if (Object.keys(this._labels).length > 0) obj.labels = Object.assign({}, this._labels);
    if (Object.keys(this._annotations).length > 0) obj.annotations = Object.assign({}, this._annotations);
    return obj;
  },
};

function createMeta(props) {
  const obj = Object.create(metaMethods);
  obj._type = "metadata";
  obj._name = props._name !== undefined ? props._name : null;
  obj._namespace = props._namespace !== undefined ? props._namespace : null;
  obj._labels = props._labels || {};
  obj._annotations = props._annotations || {};
  return obj;
}

// --- ResourceListFragment (copy-on-write) ---

const rlMethods = {
  cpu(v) {
    return createRL({ _cpu: normalizeCpu(v), _memory: this._memory });
  },
  memory(v) {
    return createRL({ _cpu: this._cpu, _memory: normalizeMemory(v) });
  },
  _toJSON() {
    const obj = {};
    if (this._cpu !== null) obj.cpu = this._cpu;
    if (this._memory !== null) obj.memory = this._memory;
    return obj;
  },
};

function createRL(props) {
  const obj = Object.create(rlMethods);
  obj._type = "resource_list";
  obj._cpu = props._cpu !== undefined ? props._cpu : null;
  obj._memory = props._memory !== undefined ? props._memory : null;
  return obj;
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

// --- ResourceRequirementsFragment (copy-on-write) ---

const rrMethods = {
  requests(rl) { return createRR(rl, this._limits); },
  limits(rl) { return createRR(this._requests, rl); },
};

function createRR(requests, limits) {
  const obj = Object.create(rrMethods);
  obj._type = "resource_requirements";
  obj._requests = requests || null;
  obj._limits = limits || null;
  return obj;
}

// --- Public factory functions ---

export function name(v) {
  return createMeta({ _name: v });
}

export function namespace(v) {
  return createMeta({ _namespace: v });
}

export function label(k, v) {
  const labels = {};
  labels[k] = v;
  return createMeta({ _labels: labels });
}

export function annotation(k, v) {
  const annotations = {};
  annotations[k] = v;
  return createMeta({ _annotations: annotations });
}

export function cpu(v) {
  return createRL({ _cpu: normalizeCpu(v) });
}

export function memory(v) {
  return createRL({ _memory: normalizeMemory(v) });
}

export function requests(rl) {
  return createRR(rl, null);
}

export function limits(rl) {
  return createRR(null, rl);
}

// --- merge ---

export function merge(items) {
  if (!Array.isArray(items) || items.length === 0) return items;

  const type = items[0]._type;

  if (type === "metadata") {
    let merged = createMeta({});
    for (const item of items) {
      if (item._name !== null) merged._name = item._name;
      if (item._namespace !== null) merged._namespace = item._namespace;
      Object.assign(merged._labels, item._labels);
      Object.assign(merged._annotations, item._annotations);
    }
    return merged;
  }

  if (type === "resource_list") {
    let merged = createRL({});
    for (const item of items) {
      if (item._cpu !== null) merged._cpu = item._cpu;
      if (item._memory !== null) merged._memory = item._memory;
    }
    return merged;
  }

  return items[items.length - 1];
}

// --- build ---

export function build(input) {
  let items;
  if (Array.isArray(input)) {
    items = input;
  } else {
    items = [input];
  }

  const rendered = items.map(function(item) {
    if (item && typeof item._render === "function") {
      return item._render();
    }
    return item;
  });

  __husako_build(rendered);
}
